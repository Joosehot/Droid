//! winit application: window, input mapping, frame pacing. Games are changed with
//! `droid edit <name> "<more words>"`; the window reloads when its files change.

use crate::game::Game;
use crate::render::{Overlay, Renderer};
use crate::sound::Audio;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

pub struct App {
    path: PathBuf,
    game: Option<Game>,
    renderer: Option<Renderer>,
    last_frame: Instant,
    error_sink: Arc<Mutex<Option<String>>>,
    /// AIGE_EXIT_AFTER=<seconds>: quit automatically (for automated testing).
    exit_after: Option<f32>,
    started: Instant,
    factory: Option<crate::game::Factory>,
    audio: Audio,
    /// Some(until) while the "use droid edit" hint is shown (TAB).
    hint_until: Option<Instant>,
}

impl App {
    pub fn new(path: PathBuf, error_sink: Option<Arc<Mutex<Option<String>>>>) -> Self {
        let exit_after = std::env::var("DROID_EXIT_AFTER").ok().and_then(|v| v.parse().ok());
        let error_sink = error_sink.unwrap_or_default();
        App {
            path,
            game: None,
            renderer: None,
            last_frame: Instant::now(),
            error_sink,
            exit_after,
            started: Instant::now(),
            factory: None,
            audio: Audio::start(),
            hint_until: None,
        }
    }

    /// Run Rust game code instead of the data's script.
    pub fn with_game(mut self, factory: crate::game::Factory) -> Self {
        self.factory = Some(factory);
        self
    }

    pub fn run(mut self) -> anyhow::Result<()> {
        let event_loop = EventLoop::new()?;
        event_loop.set_control_flow(ControlFlow::Poll);
        event_loop.run_app(&mut self)?;
        Ok(())
    }

    fn overlay(&self, w: u32, h: u32) -> Overlay {
        let mut o = Overlay::default();
        if self.hint_until.is_some_and(|t| Instant::now() < t) {
            let (w, h) = (w as f32, h as f32);
            o.rects.push((0.0, h - 34.0, w, 26.0, [0.05, 0.05, 0.08, 0.75]));
            o.texts.push(("change this game with: droid edit <name> \"<more words>\"".into(), 12.0, h - 30.0, 14.0, [0.7, 0.9, 0.7]));
        }
        o
    }
}

fn key_name(code: KeyCode) -> Option<&'static str> {
    use KeyCode::*;
    Some(match code {
        KeyA => "a", KeyB => "b", KeyC => "c", KeyD => "d", KeyE => "e", KeyF => "f", KeyG => "g",
        KeyH => "h", KeyI => "i", KeyJ => "j", KeyK => "k", KeyL => "l", KeyM => "m", KeyN => "n",
        KeyO => "o", KeyP => "p", KeyQ => "q", KeyR => "r", KeyS => "s", KeyT => "t", KeyU => "u",
        KeyV => "v", KeyW => "w", KeyX => "x", KeyY => "y", KeyZ => "z",
        Digit0 => "0", Digit1 => "1", Digit2 => "2", Digit3 => "3", Digit4 => "4",
        Digit5 => "5", Digit6 => "6", Digit7 => "7", Digit8 => "8", Digit9 => "9",
        ArrowLeft => "left", ArrowRight => "right", ArrowUp => "up", ArrowDown => "down",
        Space => "space", Enter => "enter", Escape => "escape", Tab => "tab",
        ShiftLeft | ShiftRight => "shift", ControlLeft | ControlRight => "ctrl",
        AltLeft | AltRight => "alt", Backspace => "backspace",
        _ => return None,
    })
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.renderer.is_some() {
            return;
        }
        let game = match Game::load_with(&self.path, self.factory.clone()) {
            Ok(mut g) => {
                g.error_sink = Some(self.error_sink.clone());
                g
            }
            Err(e) => {
                eprintln!("failed to load {}: {e}", self.path.display());
                event_loop.exit();
                return;
            }
        };
        let attrs = Window::default_attributes()
            .with_title(format!("Droid - {}", game.spec.title))
            .with_inner_size(winit::dpi::LogicalSize::new(1280, 720));
        let window = Arc::new(event_loop.create_window(attrs).expect("window"));
        let renderer = Renderer::new(window).expect("renderer");
        game.world.borrow_mut().aspect = renderer.aspect();
        self.renderer = Some(renderer);
        self.game = Some(game);
        self.last_frame = Instant::now();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if self.game.is_none() || self.renderer.is_none() {
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                let renderer = self.renderer.as_mut().unwrap();
                renderer.resize(size.width, size.height);
                self.game.as_ref().unwrap().world.borrow_mut().aspect = renderer.aspect();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else { return };
                let pressed = event.state == ElementState::Pressed;

                let game = self.game.as_mut().unwrap();
                if code == KeyCode::Tab && pressed && !event.repeat {
                    self.hint_until = Some(Instant::now() + std::time::Duration::from_secs(4));
                    return;
                }
                if code == KeyCode::F5 && pressed {
                    game.restart();
                    return;
                }
                if let Some(name) = key_name(code) {
                    let mut w = game.world.borrow_mut();
                    match event.state {
                        ElementState::Pressed => {
                            if !event.repeat && w.input.down.insert(name.to_string()) {
                                w.input.pressed.insert(name.to_string());
                            }
                        }
                        ElementState::Released => {
                            w.input.down.remove(name);
                            w.input.released.insert(name.to_string());
                        }
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let size = self.renderer.as_ref().unwrap().window.inner_size();
                let game = self.game.as_ref().unwrap();
                let mut w = game.world.borrow_mut();
                w.input.mouse_ndc = [
                    (position.x / size.width.max(1) as f64 * 2.0 - 1.0) as f32,
                    (1.0 - position.y / size.height.max(1) as f64 * 2.0) as f32,
                ];
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                let game = self.game.as_ref().unwrap();
                let mut w = game.world.borrow_mut();
                let down = state == ElementState::Pressed;
                if down && !w.input.mouse_down {
                    w.input.mouse_pressed = true;
                }
                w.input.mouse_down = down;
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = (now - self.last_frame).as_secs_f32();
                self.last_frame = now;
                {
                    let game = self.game.as_mut().unwrap();
                    let renderer = self.renderer.as_mut().unwrap();
                    if game.poll_reload() {
                        renderer.window.set_title(&format!("Droid - {}", game.spec.title));
                        game.world.borrow_mut().aspect = renderer.aspect();
                    }
                    game.step(dt);
                    for cmd in game.world.borrow_mut().sounds.drain(..) {
                        self.audio.play(cmd);
                    }
                }
                let size = self.renderer.as_ref().unwrap().window.inner_size();
                let overlay = self.overlay(size.width, size.height);
                let game = self.game.as_ref().unwrap();
                self.renderer.as_mut().unwrap().render(&game.world.borrow(), &overlay);
                if let Some(limit) = self.exit_after {
                    if self.started.elapsed().as_secs_f32() > limit {
                        let w = game.world.borrow();
                        for t in &w.texts {
                            eprintln!("[droid] text {} = {:?}", t.id, t.text);
                        }
                        if let Some(e) = &w.error {
                            eprintln!("[droid] ERROR: {e}");
                        }
                        event_loop.exit();
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(r) = &self.renderer {
            r.window.request_redraw();
        }
    }
}

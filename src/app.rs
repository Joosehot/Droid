//! winit application: window, input mapping, frame pacing, in-window AI chat.

use crate::ai;
use crate::game::Game;
use crate::render::{Overlay, Renderer};
use crate::sound::Audio;
use crate::project;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

/// Background AI worker: receives change requests, patches the game file, reports status.
struct AiWorker {
    tx: mpsc::Sender<String>,
    status: Arc<Mutex<String>>,
    busy: Arc<Mutex<bool>>,
}

impl AiWorker {
    fn start(path: PathBuf, error_sink: Arc<Mutex<Option<String>>>) -> Option<AiWorker> {
        let client = match ai::Client::from_env() {
            Ok(c) => c,
            Err(e) => {
                log::warn!("in-window chat disabled: {e}");
                return None;
            }
        };
        let (tx, rx) = mpsc::channel::<String>();
        let status = Arc::new(Mutex::new(format!("AI ready ({}). Press TAB to chat.", client.describe())));
        let busy = Arc::new(Mutex::new(false));
        let (st, bz) = (status.clone(), busy.clone());
        std::thread::spawn(move || {
            for req in rx {
                *bz.lock().unwrap() = true;
                *st.lock().unwrap() = format!("AI working on: {req}");
                let started = Instant::now();
                let err = error_sink.lock().unwrap().clone();
                let request = if req == "fix" { "Fix the reported error without changing the design.".to_string() } else { req.clone() };
                let result = (|| -> anyhow::Result<crate::spec::GameSpec> {
                    let current = if path.is_dir() { project::read_dir(&path)? } else { project::to_files(&project::load(&path)?)? };
                    let (spec, files) = client.edit_game(&current, &request, err.as_deref())?;
                    if path.is_dir() { project::write_dir(&path, &files)?; } else { project::save(&path, &spec)?; }
                    Ok(spec)
                })();
                let secs = started.elapsed().as_secs();
                *st.lock().unwrap() = match result {
                    Ok(spec) => format!("AI done in {secs}s: {}. TAB to chat again.", spec.title),
                    Err(e) => format!("AI error: {e}"),
                };
                *bz.lock().unwrap() = false;
            }
        });
        Some(AiWorker { tx, status, busy })
    }
}

pub struct App {
    path: PathBuf,
    game: Option<Game>,
    renderer: Option<Renderer>,
    last_frame: Instant,
    error_sink: Arc<Mutex<Option<String>>>,
    /// AIGE_EXIT_AFTER=<seconds>: quit automatically (for automated testing).
    exit_after: Option<f32>,
    started: Instant,
    audio: Audio,
    ai: Option<AiWorker>,
    /// Some(buffer) while the chat box is open.
    chat: Option<String>,
    status_shown_until: Option<Instant>,
}

impl App {
    pub fn new(path: PathBuf, error_sink: Option<Arc<Mutex<Option<String>>>>) -> Self {
        let exit_after = std::env::var("AIGE_EXIT_AFTER").ok().and_then(|v| v.parse().ok());
        let error_sink = error_sink.unwrap_or_default();
        let ai = AiWorker::start(path.clone(), error_sink.clone());
        App {
            path,
            game: None,
            renderer: None,
            last_frame: Instant::now(),
            error_sink,
            exit_after,
            started: Instant::now(),
            audio: Audio::start(),
            ai,
            chat: None,
            status_shown_until: Some(Instant::now() + std::time::Duration::from_secs(6)),
        }
    }

    pub fn run(mut self) -> anyhow::Result<()> {
        let event_loop = EventLoop::new()?;
        event_loop.set_control_flow(ControlFlow::Poll);
        event_loop.run_app(&mut self)?;
        Ok(())
    }

    fn send_chat(&mut self, text: String) {
        let Some(ai) = &self.ai else { return };
        if *ai.busy.lock().unwrap() {
            *ai.status.lock().unwrap() = "AI is still working; wait for it to finish.".into();
        } else {
            let _ = ai.tx.send(text);
        }
        self.status_shown_until = None; // keep status visible while working
    }

    fn overlay(&self, w: u32, h: u32) -> Overlay {
        let mut o = Overlay::default();
        let (w, h) = (w as f32, h as f32);
        if let Some(buf) = &self.chat {
            let bar_h = 44.0;
            o.rects.push((0.0, h - bar_h, w, bar_h, [0.05, 0.05, 0.08, 0.92]));
            let caret = if (self.started.elapsed().as_secs_f32() * 2.0) as i32 % 2 == 0 { "_" } else { " " };
            o.texts.push((format!("> {buf}{caret}"), 12.0, h - bar_h + 12.0, 20.0, [1.0, 1.0, 1.0]));
            o.texts.push(("ENTER send   ESC close".into(), w - 300.0, h - bar_h + 14.0, 14.0, [0.6, 0.6, 0.7]));
        }
        if let Some(ai) = &self.ai {
            let busy = *ai.busy.lock().unwrap();
            let timed = self.status_shown_until.map_or(true, |t| Instant::now() < t);
            if busy || timed || self.chat.is_some() {
                let status = ai.status.lock().unwrap().clone();
                let y = if self.chat.is_some() { h - 44.0 - 26.0 } else { h - 30.0 };
                o.rects.push((0.0, y - 4.0, w, 26.0, [0.05, 0.05, 0.08, 0.75]));
                let col = if busy { [1.0, 0.85, 0.3] } else if status.starts_with("AI error") { [1.0, 0.4, 0.4] } else { [0.7, 0.9, 0.7] };
                o.texts.push((status, 12.0, y, 14.0, col));
            }
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
        let game = match Game::load(&self.path) {
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
            .with_title(format!("AIGE - {}", game.spec.title))
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

                // Chat box open: capture typing.
                if let Some(buf) = self.chat.as_mut() {
                    if !pressed {
                        return;
                    }
                    match code {
                        KeyCode::Escape | KeyCode::Tab => self.chat = None,
                        KeyCode::Enter => {
                            let text = buf.trim().to_string();
                            self.chat = None;
                            if !text.is_empty() {
                                self.send_chat(text);
                            }
                        }
                        KeyCode::Backspace => {
                            buf.pop();
                        }
                        _ => {
                            if let Some(t) = &event.text {
                                for c in t.chars().filter(|c| !c.is_control()) {
                                    buf.push(c);
                                }
                            }
                        }
                    }
                    return;
                }

                let game = self.game.as_mut().unwrap();
                if code == KeyCode::Tab && pressed && !event.repeat && self.ai.is_some() {
                    self.chat = Some(String::new());
                    game.world.borrow_mut().input.down.clear();
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
                        renderer.window.set_title(&format!("AIGE - {}", game.spec.title));
                        game.world.borrow_mut().aspect = renderer.aspect();
                        self.status_shown_until = Some(Instant::now() + std::time::Duration::from_secs(8));
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
                            eprintln!("[aige] text {} = {:?}", t.id, t.text);
                        }
                        if let Some(e) = &w.error {
                            eprintln!("[aige] ERROR: {e}");
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

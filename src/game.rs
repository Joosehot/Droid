//! Game runtime: owns the world + script host, steps a frame, hot-reloads `game.json`.

use crate::physics;
use crate::project;
use crate::script::{ScriptHost, Shared};
use crate::spec::GameSpec;
use crate::world::World;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime};

pub struct Game {
    pub path: PathBuf,
    pub spec: GameSpec,
    pub world: Shared,
    host: ScriptHost,
    stamp: (Option<SystemTime>, usize),
    last_check: Instant,
    /// Shared with the chat thread so the AI can see runtime errors.
    pub error_sink: Option<std::sync::Arc<std::sync::Mutex<Option<String>>>>,
    /// Game code written in Rust (a fresh instance is made on every restart).
    native: Option<(Box<dyn crate::api::Game>, Factory)>,
}

/// Makes a fresh game-code instance.
pub type Factory = std::rc::Rc<dyn Fn() -> Box<dyn crate::api::Game>>;

impl Game {
    pub fn load(path: &Path) -> anyhow::Result<Game> {
        Self::load_with(path, None)
    }

    /// Load the game data at `path`; with a factory, the game logic is that Rust code
    /// instead of the data's script.
    pub fn load_with(path: &Path, factory: Option<Factory>) -> anyhow::Result<Game> {
        let spec = project::load(path)?;
        let stamp = Self::stamp_of(path);
        let world: Shared = Rc::new(RefCell::new(World::from_spec(&spec)));
        let host = ScriptHost::new(world.clone());
        let native = factory.map(|f| (f(), f));
        let mut g = Game { path: path.to_path_buf(), spec, world, host, stamp, last_check: Instant::now(), error_sink: None, native };
        g.start_logic();
        Ok(g)
    }

    /// The game code's status (script games are always "playing").
    pub fn status(&self) -> crate::api::Status {
        self.native.as_ref().map(|(g, _)| g.status()).unwrap_or(crate::api::Status::Playing)
    }

    fn start_logic(&mut self) {
        if let Some((game, _)) = self.native.as_mut() {
            let mut w = self.world.borrow_mut();
            game.init(&mut crate::api::Ctx::new(&mut w));
        } else {
            let r = self.host.load(&self.spec.script.clone());
            self.report(r);
        }
    }

    fn stamp_of(path: &Path) -> (Option<SystemTime>, usize) {
        if path.is_dir() {
            project::stamp(path)
        } else {
            (std::fs::metadata(path).and_then(|m| m.modified()).ok(), 1)
        }
    }

    fn report(&mut self, r: Result<(), String>) {
        if let Err(e) = r {
            log::error!("{e}");
            self.world.borrow_mut().error = Some(e.clone());
            if let Some(s) = &self.error_sink {
                *s.lock().unwrap() = Some(e);
            }
        }
    }

    /// Rebuild world + script from the current spec (restart).
    pub fn restart(&mut self) {
        let aspect = self.world.borrow().aspect;
        let world: Shared = Rc::new(RefCell::new(World::from_spec(&self.spec)));
        world.borrow_mut().aspect = aspect;
        self.world = world;
        self.host = ScriptHost::new(self.world.clone());
        if let Some(s) = &self.error_sink {
            *s.lock().unwrap() = None;
        }
        if let Some((game, f)) = self.native.as_mut() {
            *game = f();
        }
        self.start_logic();
    }

    /// Reload from disk if the file changed. Returns true if reloaded.
    pub fn poll_reload(&mut self) -> bool {
        if self.last_check.elapsed() < Duration::from_millis(400) {
            return false;
        }
        self.last_check = Instant::now();
        let stamp = Self::stamp_of(&self.path);
        if stamp == self.stamp {
            return false;
        }
        self.stamp = stamp;
        match project::load(&self.path) {
            Ok(spec) => {
                log::info!("reloaded {}", self.path.display());
                // Art-only change (sprites/models): swap assets without restarting the game.
                let art_only = spec.script == self.spec.script
                    && spec.entities.len() == self.spec.entities.len()
                    && serde_json::to_string(&spec.entities).ok() == serde_json::to_string(&self.spec.entities).ok()
                    && spec.texts.len() == self.spec.texts.len()
                    && spec.view == self.spec.view
                    && (spec.sprites.len() != self.spec.sprites.len() || spec.models.len() != self.spec.models.len()
                        || serde_json::to_string(&spec.sprites).ok() != serde_json::to_string(&self.spec.sprites).ok()
                        || serde_json::to_string(&spec.models).ok() != serde_json::to_string(&self.spec.models).ok());
                self.spec = spec;
                if art_only {
                    self.world.borrow_mut().replace_art(&self.spec);
                } else {
                    self.restart();
                }
                true
            }
            Err(e) => {
                self.report(Err(format!("{e:#}")));
                false
            }
        }
    }

    pub fn step(&mut self, dt: f32) {
        let dt = dt.min(1.0 / 20.0);
        {
            let mut w = self.world.borrow_mut();
            w.dt = dt;
            w.time += dt;
        }
        if let Some((game, _)) = self.native.as_mut() {
            let mut w = self.world.borrow_mut();
            game.update(&mut crate::api::Ctx::new(&mut w), dt);
        } else {
            let r = self.host.update(dt);
            self.report(r);
        }

        {
            let mut w = self.world.borrow_mut();
            physics::step(&mut w, dt);
            crate::anim::step(&mut w, dt);
        }

        let pairs: Vec<(String, String)> = {
            let w = self.world.borrow();
            w.collisions.iter().map(|(a, b)| (w.name_of(*a), w.name_of(*b))).collect()
        };
        for (a, b) in pairs {
            if a.is_empty() || b.is_empty() {
                continue;
            }
            if let Some((game, _)) = self.native.as_mut() {
                let mut w = self.world.borrow_mut();
                let mut cx = crate::api::Ctx::new(&mut w);
                game.on_collision(&mut cx, &a, &b);
                game.on_collision(&mut cx, &b, &a);
                continue;
            }
            let r = self.host.on_collision(&a, &b);
            self.report(r);
            let r = self.host.on_collision(&b, &a);
            self.report(r);
        }

        let restart = {
            let mut w = self.world.borrow_mut();
            w.update_camera();
            w.input.end_frame();
            std::mem::take(&mut w.restart_requested)
        };
        if restart {
            self.restart();
        }
    }
}

impl GameSpec {
    pub fn empty() -> GameSpec {
        GameSpec {
            title: String::new(),
            description: String::new(),
            view: Default::default(),
            camera: Default::default(),
            background: [0.0; 3],
            gravity: [0.0; 3],
            sprites: Default::default(),
            models: Default::default(),
            entities: vec![],
            texts: vec![],
            script: String::new(),
        }
    }
}

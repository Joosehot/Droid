//! Game runtime: owns the world + script host, steps a frame, hot-reloads `game.json`.

use crate::physics;
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
    mtime: Option<SystemTime>,
    last_check: Instant,
    /// Shared with the chat thread so the AI can see runtime errors.
    pub error_sink: Option<std::sync::Arc<std::sync::Mutex<Option<String>>>>,
}

impl Game {
    pub fn load(path: &Path) -> anyhow::Result<Game> {
        let src = std::fs::read_to_string(path)?;
        let spec = GameSpec::from_json(&src)?;
        let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
        let world: Shared = Rc::new(RefCell::new(World::from_spec(&spec)));
        let host = ScriptHost::new(world.clone());
        let mut g = Game { path: path.to_path_buf(), spec, world, host, mtime, last_check: Instant::now(), error_sink: None };
        let r = g.host.load(&g.spec.script.clone());
        g.report(r);
        Ok(g)
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
        let r = self.host.load(&self.spec.script);
        self.report(r);
    }

    /// Reload from disk if the file changed. Returns true if reloaded.
    pub fn poll_reload(&mut self) -> bool {
        if self.last_check.elapsed() < Duration::from_millis(400) {
            return false;
        }
        self.last_check = Instant::now();
        let mtime = std::fs::metadata(&self.path).and_then(|m| m.modified()).ok();
        if mtime == self.mtime {
            return false;
        }
        self.mtime = mtime;
        match std::fs::read_to_string(&self.path).map_err(|e| e.to_string()).and_then(|s| GameSpec::from_json(&s).map_err(|e| e.to_string())) {
            Ok(spec) => {
                log::info!("reloaded {}", self.path.display());
                self.spec = spec;
                self.restart();
                true
            }
            Err(e) => {
                let msg = format!("game.json: {e}");
                self.report(Err(msg));
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
        let r = self.host.update(dt);
        self.report(r);

        {
            let mut w = self.world.borrow_mut();
            physics::step(&mut w, dt);
        }

        let pairs: Vec<(String, String)> = {
            let w = self.world.borrow();
            w.collisions.iter().map(|(a, b)| (w.name_of(*a), w.name_of(*b))).collect()
        };
        for (a, b) in pairs {
            if a.is_empty() || b.is_empty() {
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
            entities: vec![],
            texts: vec![],
            script: String::new(),
        }
    }
}

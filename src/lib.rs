//! Droid: a game engine whose games are written from a sentence.
//!
//! The runtime (window, renderer, physics, sound, models, animation) lives here. A game is a
//! Rust type implementing [`api::Game`] plus a data folder (game.toml, scene.toml, sprites,
//! models) and is started with [`run`].

pub mod anim;
pub mod assets;
pub mod api;
pub mod app;
pub mod font;
pub mod game;
pub mod hot;
pub mod models;
pub mod physics;
pub mod project;
pub mod render;
pub mod script;
pub mod sound;
pub mod spec;
pub mod world;

pub mod prelude {
    pub use crate::api::{vec2, vec3, Ctx, Game, Status, Vec3};
    pub use crate::spec::BodyKind;
}

/// Open a window and run `G` on the game data in `data_dir`.
///
/// With `DROID_SIM=<seconds>` set, runs headless instead (no window, no input, fixed 1/60 s
/// steps) and prints a one-line JSON report: title, seconds, status, error, player position,
/// entity count and texts. The compiler's checks use this.
pub fn run<G: api::Game + Default + 'static>(data_dir: &str) {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("droid=info,warn")).try_init();
    let factory: game::Factory = std::rc::Rc::new(|| Box::new(G::default()) as Box<dyn api::Game>);
    if let Some(secs) = std::env::var("DROID_SIM").ok().and_then(|v| v.parse::<f32>().ok()) {
        match sim(std::path::Path::new(data_dir), factory, secs) {
            Ok(report) => println!("{report}"),
            Err(e) => {
                println!("{}", serde_json::json!({ "error": format!("{e:#}") }));
                std::process::exit(1);
            }
        }
        return;
    }
    if let Err(e) = app::App::new(std::path::PathBuf::from(data_dir), None).with_game(factory).run() {
        eprintln!("droid: {e:#}");
        std::process::exit(1);
    }
}

/// Run a game headless for `seconds` and describe how it ended up (JSON).
pub fn sim(data_dir: &std::path::Path, factory: game::Factory, seconds: f32) -> anyhow::Result<serde_json::Value> {
    let mut g = game::Game::load_with(data_dir, Some(factory))?;
    let frames = (seconds * 60.0).round().max(0.0) as u32;
    let mut first_end: Option<(f32, &'static str)> = None;
    for i in 0..frames {
        g.step(1.0 / 60.0);
        g.world.borrow_mut().sounds.clear();
        let st = g.status();
        if first_end.is_none() && st != api::Status::Playing {
            first_end = Some(((i + 1) as f32 / 60.0, st.name()));
        }
    }
    let w = g.world.borrow();
    let player = w.entity("player").and_then(|e| w.ecs.get::<&world::Transform>(e).ok().map(|t| [t.pos.x, t.pos.y, t.pos.z]));
    let texts: serde_json::Map<String, serde_json::Value> = w.texts.iter().map(|t| (t.id.clone(), serde_json::Value::String(t.text.clone()))).collect();
    Ok(serde_json::json!({
        "title": g.spec.title,
        "seconds": seconds,
        "status": g.status().name(),
        "ended_at": first_end.map(|(t, s)| serde_json::json!({ "seconds": t, "status": s })),
        "error": w.error,
        "player": player,
        "entities": w.ecs.len(),
        "texts": texts,
    }))
}

//! Droid: a game engine whose games are written from a sentence.
//!
//! The runtime (window, renderer, physics, sound, models, animation) lives here. A game is a
//! Rust type implementing [`api::Game`] plus a data folder (game.toml, scene.toml, sprites,
//! models) and is started with [`run`].

pub mod ai;
pub mod anim;
pub mod api;
pub mod app;
pub mod font;
pub mod game;
pub mod models;
pub mod physics;
pub mod project;
pub mod render;
pub mod script;
pub mod sound;
pub mod spec;
pub mod world;

pub mod prelude {
    pub use crate::api::{vec2, vec3, Ctx, Game, Vec3};
    pub use crate::spec::BodyKind;
}

/// Open a window and run `G` on the game data in `data_dir`.
pub fn run<G: api::Game + Default + 'static>(data_dir: &str) {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("droid=info,warn")).try_init();
    let factory: game::Factory = std::rc::Rc::new(|| Box::new(G::default()) as Box<dyn api::Game>);
    if let Err(e) = app::App::new(std::path::PathBuf::from(data_dir), None).with_game(factory).run() {
        eprintln!("droid: {e:#}");
        std::process::exit(1);
    }
}

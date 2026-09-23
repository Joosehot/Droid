//! AIGE - AI-native game engine. The AI writes the game; the engine runs it.

mod ai;
mod app;
mod font;
mod game;
mod physics;
mod render;
mod script;
mod spec;
mod world;

use anyhow::{bail, Context};
use clap::{Parser, Subcommand};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Parser)]
#[command(name = "aige", version, about = "AI-native game engine: describe a game, play it, change it while it runs")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Generate a new game from a description into games/<name>/game.json
    New { name: String, prompt: String },
    /// Run a game (hot-reloads game.json when it changes; F5 restarts)
    Run { name: String },
    /// Ask the AI to change an existing game
    Edit { name: String, prompt: String },
    /// Generate a game and immediately run it with the live chat open
    Play { name: String, prompt: String },
    /// Run a game with a terminal chat: every line you type is sent to the AI as a change request
    Chat { name: String },
    /// Validate a game file (JSON + script) without running it
    Check { name: String },
    /// Simulate N frames headlessly (no window) and report errors and UI texts
    Sim {
        name: String,
        #[arg(long, default_value_t = 300)]
        frames: u32,
        /// Fixed timestep in seconds
        #[arg(long, default_value_t = 1.0 / 60.0)]
        dt: f32,
    },
}

fn game_path(name: &str) -> PathBuf {
    let p = Path::new(name);
    if p.extension().map_or(false, |e| e == "json") {
        return p.to_path_buf();
    }
    Path::new("games").join(name).join("game.json")
}

fn save(path: &Path, spec: &spec::GameSpec) -> anyhow::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, spec.to_json())?;
    Ok(())
}

fn load(path: &Path) -> anyhow::Result<spec::GameSpec> {
    let s = std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    spec::GameSpec::from_json(&s)
}

fn generate_new(client: &ai::Client, path: &Path, prompt: &str) -> anyhow::Result<()> {
    eprintln!("[aige] {} is designing your game...", client.describe());
    let spec = client.new_game(prompt)?;
    save(path, &spec)?;
    eprintln!("[aige] wrote {} ({} entities, {} script bytes): {}", path.display(), spec.entities.len(), spec.script.len(), spec.title);
    Ok(())
}

/// Terminal chat loop, runs on a background thread while the window is open.
fn chat_thread(path: PathBuf, error_sink: Arc<Mutex<Option<String>>>) {
    std::thread::spawn(move || {
        let client = match ai::Client::from_env() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[aige] chat disabled: {e}");
                return;
            }
        };
        eprintln!("[aige] chat ready ({}). Type a change request and press Enter. 'fix' = fix the current error, 'quit' = exit chat.", client.describe());
        let stdin = std::io::stdin();
        loop {
            eprint!("aige> ");
            let _ = std::io::stderr().flush();
            let mut line = String::new();
            if stdin.lock().read_line(&mut line).unwrap_or(0) == 0 {
                break;
            }
            let req = line.trim().to_string();
            if req.is_empty() {
                continue;
            }
            if req == "quit" || req == "exit" {
                break;
            }
            let current = match load(&path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[aige] cannot load current game: {e}");
                    continue;
                }
            };
            let err = error_sink.lock().unwrap().clone();
            let request = if req == "fix" { "Fix the reported error without changing the design.".to_string() } else { req };
            match client.edit_game(&current, &request, err.as_deref()) {
                Ok(spec) => {
                    if let Err(e) = save(&path, &spec) {
                        eprintln!("[aige] save failed: {e}");
                    } else {
                        eprintln!("[aige] updated: {} (the window reloads automatically)", spec.title);
                    }
                }
                Err(e) => eprintln!("[aige] AI error: {e}"),
            }
        }
    });
}

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("aige=info,warn")).init();
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::New { name, prompt } => {
            let client = ai::Client::from_env()?;
            generate_new(&client, &game_path(&name), &prompt)?;
        }
        Cmd::Edit { name, prompt } => {
            let client = ai::Client::from_env()?;
            let path = game_path(&name);
            let current = load(&path)?;
            eprintln!("[aige] {} is editing {}...", client.describe(), current.title);
            let spec = client.edit_game(&current, &prompt, None)?;
            save(&path, &spec)?;
            eprintln!("[aige] updated {}", path.display());
        }
        Cmd::Check { name } => {
            let path = game_path(&name);
            let spec = load(&path)?;
            script::ScriptHost::check(&spec.script).map_err(|e| anyhow::anyhow!("script: {e}"))?;
            println!("OK: {} ({} entities)", spec.title, spec.entities.len());
        }
        Cmd::Sim { name, frames, dt } => {
            let path = game_path(&name);
            let mut g = game::Game::load(&path)?;
            let mut last = std::time::Instant::now();
            for _ in 0..frames {
                if dt <= 0.0 {
                    std::thread::sleep(std::time::Duration::from_millis(16));
                    let now = std::time::Instant::now();
                    g.step((now - last).as_secs_f32());
                    last = now;
                } else {
                    g.step(dt);
                }
            }
            {
                let w = g.world.borrow();
                for (_, (n, t)) in w.ecs.query::<(&world::Name, &world::Transform)>().iter() {
                    println!("  {} at ({:.2}, {:.2}, {:.2})", n.0, t.pos.x, t.pos.y, t.pos.z);
                }
            }
            let w = g.world.borrow();
            println!("{}: {} frames, {} entities", g.spec.title, frames, w.ecs.len());
            for t in &w.texts {
                println!("  text {} = {:?}", t.id, t.text);
            }
            if let Some(e) = &w.error {
                println!("ERROR: {e}");
                std::process::exit(1);
            }
        }
        Cmd::Run { name } => {
            let path = game_path(&name);
            if !path.exists() {
                bail!("{} does not exist. Create it with: aige new {name} \"<description>\"", path.display());
            }
            app::App::new(path, None).run()?;
        }
        Cmd::Chat { name } => {
            let path = game_path(&name);
            if !path.exists() {
                bail!("{} does not exist", path.display());
            }
            let sink = Arc::new(Mutex::new(None));
            chat_thread(path.clone(), sink.clone());
            app::App::new(path, Some(sink)).run()?;
        }
        Cmd::Play { name, prompt } => {
            let client = ai::Client::from_env()?;
            let path = game_path(&name);
            generate_new(&client, &path, &prompt)?;
            let sink = Arc::new(Mutex::new(None));
            chat_thread(path.clone(), sink.clone());
            app::App::new(path, Some(sink)).run()?;
        }
    }
    Ok(())
}

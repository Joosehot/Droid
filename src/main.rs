//! Droid command line.

use droid::{ai, app, game, project, spec, world};


use anyhow::bail;
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
    /// Generate a new game from a description into games/<name>/
    New { name: String, prompt: String },
    /// Run a game (hot-reloads when its files change; F5 restarts; TAB opens the AI chat)
    Run { name: String },
    /// Ask the AI to change an existing game
    Edit { name: String, prompt: String },
    /// Generate a game and immediately run it
    Play { name: String, prompt: String },
    /// Run a game with an additional terminal chat (the window has TAB chat anyway)
    Chat { name: String },
    /// Validate a game (files + script) without running it
    Check { name: String },
    /// Simulate N frames headlessly (no window) and report errors and UI texts
    Sim {
        name: String,
        #[arg(long, default_value_t = 300)]
        frames: u32,
        /// Fixed timestep in seconds (0 = real time)
        #[arg(long, default_value_t = 1.0 / 60.0)]
        dt: f32,
    },
    /// Convert a legacy games/<name>/game.json into the folder format
    Convert { name: String },
    /// List the asset library (downloaded CC0 art) by kind; --check validates it
    Assets {
        /// Check every file exists and loads and every required kind is covered; exit 1 if not
        #[arg(long)]
        check: bool,
        /// Library folder (default: the assets/ folder Droid was built from)
        #[arg(long)]
        dir: Option<PathBuf>,
    },
}

/// `games/<name>` folder, or a direct path to a folder / legacy `.json`.
fn game_path(name: &str) -> PathBuf {
    let p = Path::new(name);
    if p.is_dir() || p.extension().map_or(false, |e| e == "json") {
        return p.to_path_buf();
    }
    let dir = Path::new("legacy").join(name);
    let legacy = dir.join("game.json");
    if !dir.join("game.toml").exists() && legacy.exists() {
        return legacy;
    }
    dir
}

fn generate_new(client: &ai::Client, path: &Path, prompt: &str) -> anyhow::Result<()> {
    eprintln!("[aige] {} is designing your game...", client.describe());
    let (spec, files) = client.new_game(prompt)?;
    project::write_dir(path, &files)?;
    eprintln!(
        "[aige] wrote {} ({} files, {} entities, {} sprites, {} models): {}",
        path.display(),
        files.len(),
        spec.entities.len(),
        spec.sprites.len(),
        spec.models.len(),
        spec.title
    );
    Ok(())
}

/// Ask the AI for a change and save the result. Shared by `edit`, the terminal chat and the window chat.
pub fn ai_edit(client: &ai::Client, path: &Path, request: &str, runtime_error: Option<&str>) -> anyhow::Result<spec::GameSpec> {
    let current = if path.is_dir() { project::read_dir(path)? } else { project::to_files(&project::load(path)?)? };
    let (spec, files) = client.edit_game(&current, request, runtime_error)?;
    if path.is_dir() {
        project::write_dir(path, &files)?;
    } else {
        project::save(path, &spec)?;
    }
    Ok(spec)
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
            let err = error_sink.lock().unwrap().clone();
            let request = if req == "fix" { "Fix the reported error without changing the design.".to_string() } else { req };
            match ai_edit(&client, &path, &request, err.as_deref()) {
                Ok(spec) => eprintln!("[aige] updated: {} (the window reloads automatically)", spec.title),
                Err(e) => eprintln!("[aige] AI error: {e}"),
            }
        }
    });
}

/// Load KEY=VALUE lines from `.env` into the environment without overriding set variables.
fn load_dotenv() {
    let Ok(text) = std::fs::read_to_string(".env") else { return };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let (k, v) = (k.trim(), v.trim().trim_matches('"').trim_matches('\''));
            if std::env::var_os(k).is_none() {
                std::env::set_var(k, v);
            }
        }
    }
}

fn main() -> anyhow::Result<()> {
    load_dotenv();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("aige=info,warn")).init();
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::New { name, prompt } => {
            let client = ai::Client::from_env()?;
            generate_new(&client, &Path::new("legacy").join(&name), &prompt)?;
        }
        Cmd::Edit { name, prompt } => {
            let client = ai::Client::from_env()?;
            let path = game_path(&name);
            eprintln!("[aige] {} is editing {}...", client.describe(), path.display());
            let spec = ai_edit(&client, &path, &prompt, None)?;
            eprintln!("[aige] updated {}: {}", path.display(), spec.title);
        }
        Cmd::Check { name } => {
            let path = game_path(&name);
            let spec = ai::validate(project::load(&path)?)?;
            println!("OK: {} ({} entities, {} sprites, {} models)", spec.title, spec.entities.len(), spec.sprites.len(), spec.models.len());
        }
        Cmd::Assets { check, dir } => {
            let lib = match dir {
                Some(d) => droid::assets::Library::load(&d)?,
                None => droid::assets::Library::builtin()?,
            };
            if check {
                let errs = lib.validate();
                if !errs.is_empty() {
                    for e in &errs {
                        eprintln!("error: {e}");
                    }
                    eprintln!("{} problem(s) in {}", errs.len(), lib.dir.display());
                    std::process::exit(1);
                }
                let files: usize = lib.packs.iter().map(|p| p.files.len()).sum();
                println!(
                    "OK: {} sprites, {} tilesets, {} models from {} packs ({} files); every required kind and theme is covered",
                    lib.sprites.len(),
                    lib.tilesets.len(),
                    lib.models.len(),
                    lib.packs.len(),
                    files
                );
            } else {
                print!("{}", lib.listing());
            }
        }
        Cmd::Convert { name } => {
            let dir = Path::new("legacy").join(&name);
            let legacy = dir.join("game.json");
            if !legacy.exists() {
                bail!("{} not found", legacy.display());
            }
            let spec = project::load(&legacy)?;
            project::write_dir(&dir, &project::to_files(&spec)?)?;
            std::fs::rename(&legacy, dir.join("game.json.bak"))?;
            println!("converted {} -> {}/ (old file kept as game.json.bak)", legacy.display(), dir.display());
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
            let w = g.world.borrow();
            for (_, (n, t)) in w.ecs.query::<(&world::Name, &world::Transform)>().iter() {
                println!("  {} at ({:.2}, {:.2}, {:.2})", n.0, t.pos.x, t.pos.y, t.pos.z);
            }
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
            let path = Path::new("legacy").join(&name);
            generate_new(&client, &path, &prompt)?;
            app::App::new(path, None).run()?;
        }
    }
    Ok(())
}

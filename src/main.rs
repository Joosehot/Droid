//! Droid command line: a whole game from one sentence (`droid make`), plus the legacy
//! commands for the script games in legacy/.

use droid::{app, game, hot, project, world};

use anyhow::bail;
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "droid", version, about = "Droid: a game engine where a whole game comes from one sentence")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Make games/<name>/ from a sentence: design, level, art, behaviour, Rust code, checks
    Make {
        name: String,
        sentence: String,
        /// Also print what every rule weighed and why the game is the way it is
        #[arg(long)]
        explain: bool,
        /// Use another rules.toml (every number the engine uses)
        #[arg(long)]
        rules: Option<PathBuf>,
        /// Skip words the vocabulary doesn't know instead of failing
        #[arg(long)]
        lenient: bool,
        /// Open the game in a window when it is made
        #[arg(long)]
        run: bool,
        /// Skip `cargo build` and the headless run (checks 4 and 5)
        #[arg(long)]
        no_build: bool,
    },
    /// Add words to a game's sentence and make it again (later words override earlier ones)
    Edit {
        name: String,
        words: String,
        #[arg(long)]
        explain: bool,
        #[arg(long)]
        rules: Option<PathBuf>,
        #[arg(long)]
        lenient: bool,
        #[arg(long)]
        run: bool,
        #[arg(long)]
        no_build: bool,
    },
    /// Show how a sentence is understood and what the rules would choose (writes nothing)
    Explain {
        sentence: String,
        #[arg(long)]
        rules: Option<PathBuf>,
        #[arg(long)]
        lenient: bool,
    },
    /// List every word a sentence may use
    Vocabulary,
    /// List the asset library (downloaded CC0 art) by kind; --check validates it
    Assets {
        /// Check every file exists and loads and every required kind is covered; exit 1 if not
        #[arg(long)]
        check: bool,
        /// Library folder (default: the assets/ folder Droid was built from)
        #[arg(long)]
        dir: Option<PathBuf>,
    },
    /// Legacy: run a script game from legacy/ (hot-reloads when its files change; F5 restarts)
    Run { name: String },
    /// Legacy: validate a script game (files + script) without running it
    Check { name: String },
    /// Legacy: simulate N frames headlessly (no window) and report errors and UI texts
    Sim {
        name: String,
        #[arg(long, default_value_t = 300)]
        frames: u32,
        /// Fixed timestep in seconds (0 = real time)
        #[arg(long, default_value_t = 1.0 / 60.0)]
        dt: f32,
    },
    /// Legacy: convert legacy/<name>/game.json into the folder format
    Convert { name: String },
}

/// `legacy/<name>` folder, or a direct path to a folder / legacy `.json`.
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

/// The Droid checkout: games/ lives here, and the staging crates build against it.
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fail(errs: Vec<String>) -> ! {
    for e in errs {
        eprintln!("{e}");
    }
    std::process::exit(1);
}

fn load_rules(rules: Option<&Path>) -> hot::config::Config {
    hot::config::Config::load(rules).unwrap_or_else(|e| fail(vec![format!("error: {e:#}")]))
}

fn library() -> droid::assets::Library {
    droid::assets::Library::builtin().unwrap_or_else(|e| fail(vec![format!("error: asset library: {e:#}")]))
}

fn make(name: &str, sentence: &str, explain: bool, rules: Option<&Path>, lenient: bool, run: bool, no_build: bool) -> anyhow::Result<()> {
    let cfg = load_rules(rules);
    let lib = library();
    if explain {
        match hot::explain(sentence, &cfg, &lib, lenient) {
            Ok(s) => println!("{s}"),
            Err(e) => fail(e),
        }
    }
    let opts = hot::Options { lenient, build: !no_build };
    eprintln!("[droid] making {name}: \"{sentence}\"{}", if no_build { "" } else { " (checks include cargo build and a headless run)" });
    let made = hot::make(&root(), name, sentence, &cfg, &lib, &opts).unwrap_or_else(|e| fail(e));
    for d in &made.dropped {
        eprintln!("[droid] {d}");
    }
    println!("made {} ({} files) from finalist {}", made.dir.display(), made.files, made.finalist);
    if let Some(r) = &made.report {
        println!("headless run: status {}, player at {}, {} entities, texts {}", r["status"], r["player"], r["entities"], r["texts"]);
    }
    println!("see why.txt for every decision; play it with: cargo run -p {}", made.slug);
    if run {
        let status = std::process::Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
            .args(["run", "--quiet", "-p", &made.slug])
            .current_dir(root())
            .status()?;
        if !status.success() {
            bail!("the game exited with {status}");
        }
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("droid=info,warn")).init();
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Make { name, sentence, explain, rules, lenient, run, no_build } => {
            make(&name, &sentence, explain, rules.as_deref(), lenient, run, no_build)?;
        }
        Cmd::Edit { name, words, explain, rules, lenient, run, no_build } => {
            let slug = hot::slug(&name).unwrap_or_else(|e| fail(vec![format!("error: {e}")]));
            let path = root().join("games").join(&slug).join("sentence.txt");
            let old = std::fs::read_to_string(&path).unwrap_or_else(|_| fail(vec![format!("error: {} not found; make the game first with: droid make {slug} \"<sentence>\"", path.display())]));
            let sentence = format!("{}, {}", old.trim().trim_end_matches('.'), words.trim());
            make(&slug, &sentence, explain, rules.as_deref(), lenient, run, no_build)?;
        }
        Cmd::Explain { sentence, rules, lenient } => {
            let cfg = load_rules(rules.as_deref());
            match hot::explain(&sentence, &cfg, &library(), lenient) {
                Ok(s) => println!("{s}"),
                Err(e) => fail(e),
            }
        }
        Cmd::Vocabulary => {
            for w in hot::vocabulary() {
                println!("{w}");
            }
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
        Cmd::Check { name } => {
            let path = game_path(&name);
            let spec = project::check(project::load(&path)?)?;
            println!("OK: {} ({} entities, {} sprites, {} models)", spec.title, spec.entities.len(), spec.sprites.len(), spec.models.len());
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
                bail!("{} does not exist (new games are made with: droid make <name> \"<sentence>\")", path.display());
            }
            app::App::new(path, None).run()?;
        }
    }
    Ok(())
}

//! The Droid front (HotMotor design): one sentence -> a whole game.
//!
//! ```text
//! sentence -> lexicon -> parser -> Request (with provenance)
//!          -> rules (one decision each, variants on [challenge, pace, simplicity])
//!          -> beam search with judges -> finalists, best first
//!          -> for each finalist: design (numbers) -> art (asset library) -> level (seeded PRNG)
//!             -> behavior.txt (Senne's language) -> Senne -> src/main.rs
//!             -> checks (Senne, reachability, spawn safety, chase fairness, budget,
//!                cargo build -D warnings, headless run); the first finalist that passes is written
//! ```
//! No model runs; the same sentence and the same rules.toml give byte-identical files.

pub mod behavior;
pub mod checks;
pub mod config;
pub mod design;
pub mod lexicon;
pub mod level;
pub mod paint;
pub mod parser;
pub mod request;
pub mod rng;
pub mod rules;
pub mod search;

use crate::assets::Library;
use config::Config;
use request::Request;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default)]
pub struct Options {
    /// Skip unknown words instead of failing.
    pub lenient: bool,
    /// Build the crate and run it headless (checks 4 and 5).
    pub build: bool,
}

/// Everything a finalist turns into, before it is written.
pub struct Generated {
    pub design: design::Design,
    pub level: level::Level,
    pub lines: Vec<behavior::Line>,
    /// Text files relative to the game crate (Cargo.toml, behavior.txt, game/scene.toml, ...).
    pub files: BTreeMap<String, String>,
    /// (absolute source, destination relative to the game crate): the library art it uses.
    pub copies: Vec<(PathBuf, String)>,
}

/// A crate name from a game name: lowercase letters, digits and '_', starting with a letter.
pub fn slug(name: &str) -> Result<String, String> {
    let mut s: String = name.trim().to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    while s.contains("__") {
        s = s.replace("__", "_");
    }
    let s = s.trim_matches('_').to_string();
    if s.is_empty() || !s.starts_with(|c: char| c.is_ascii_lowercase()) {
        return Err(format!("\"{name}\" can't name a game: start it with a letter (e.g. forest_run)"));
    }
    if ["droid", "senne", "test", "core", "std"].contains(&s.as_str()) {
        return Err(format!("\"{s}\" is taken; pick another game name"));
    }
    Ok(s)
}

fn camel(slug: &str) -> String {
    let s: String = slug.split('_').filter(|p| !p.is_empty()).map(|p| {
        let mut c = p.chars();
        c.next().map(|f| f.to_ascii_uppercase().to_string() + c.as_str()).unwrap_or_default()
    }).collect();
    if s == "Game" || s == "Status" || s == "Ctx" {
        format!("{s}Game")
    } else {
        s
    }
}

fn title(slug: &str) -> String {
    slug.split('_').filter(|p| !p.is_empty()).map(|p| {
        let mut c = p.chars();
        c.next().map(|f| f.to_ascii_uppercase().to_string() + c.as_str()).unwrap_or_default()
    }).collect::<Vec<_>>().join(" ")
}

/// Parse and search. Errors are the parser's diagnostics or the search's honest failure.
pub fn plan(sentence: &str, cfg: &Config, lenient: bool) -> Result<(Request, search::Outcome), Vec<String>> {
    let req = parser::parse(sentence, lenient).map_err(|d| d.iter().map(|x| x.to_string()).collect::<Vec<_>>())?;
    let cx = rules::Ctx { req: &req, cfg, profile: rules::profile(&req, cfg) };
    let out = search::search(&cx).map_err(|e| vec![format!("error: {e}")])?;
    Ok((req, out))
}

#[derive(serde::Serialize)]
struct Meta<'a> {
    title: &'a str,
    view: crate::spec::View,
    camera: &'a crate::spec::CameraSpec,
    background: [f32; 3],
    gravity: [f32; 3],
}

#[derive(serde::Serialize)]
struct Scene<'a> {
    entities: &'a [crate::spec::EntitySpec],
    texts: &'a [crate::spec::TextSpec],
}

/// One finalist -> files, running checks 1-3.
pub fn assemble(req: &Request, cfg: &Config, lib: &Library, cand: &search::Candidate, slug: &str) -> Result<Generated, String> {
    let d = design::design(req, cfg, cand)?;
    let art = paint::paint(&d, cfg, lib)?;
    let lv = level::build(&d, cfg, &art)?;
    let lines = behavior::write(&d, cfg, &lv, &art);
    let behavior_txt = behavior::text(&lines);
    let code = checks::senne(&behavior_txt, &camel(slug))?;
    checks::level(&d, cfg, &lv)?;

    let mut files = BTreeMap::new();
    files.insert("Cargo.toml".to_string(), checks::crate_toml(slug));
    files.insert("sentence.txt".to_string(), format!("{}\n", req.sentence));
    files.insert("behavior.txt".to_string(), behavior_txt);
    files.insert("src/main.rs".to_string(), code);
    let t = title(slug);
    let meta = Meta { title: &t, view: lv.view, camera: &lv.camera, background: lv.background, gravity: lv.gravity };
    files.insert("game/game.toml".to_string(), crate::project::to_toml(&meta).map_err(|e| e.to_string())?);
    let scene = Scene { entities: &lv.entities, texts: &lv.texts };
    files.insert("game/scene.toml".to_string(), crate::project::to_toml(&scene).map_err(|e| e.to_string())?);
    for (rel, text) in &art.install.texts {
        files.insert(format!("game/{rel}"), text.clone());
    }
    let copies: Vec<(PathBuf, String)> = art.install.copies.iter().map(|(src, dst)| (src.clone(), format!("game/{dst}"))).collect();
    files.insert("why.txt".to_string(), why(req, cand, &d, &art, &lv, &lines));
    Ok(Generated { design: d, level: lv, lines, files, copies })
}

fn why(req: &Request, cand: &search::Candidate, d: &design::Design, art: &paint::ArtPlan, lv: &level::Level, lines: &[behavior::Line]) -> String {
    let mut s = String::new();
    s.push_str("# Why this game is the way it is. Each line: piece  <- [rule/variant]  <- \"the words that asked for it\"\n");
    s.push_str(&format!("# sentence: {}\n", req.sentence));
    s.push_str(&format!("# score {:.3} (rules {:.3}, judges {:+.3})\n", cand.score, cand.local, cand.score - cand.local));
    for (v, n) in &cand.judges {
        s.push_str(&format!("# judge {v:+.2}: {n}\n"));
    }
    let row = |s: &mut String, w: &design::Why| {
        if w.words.is_empty() {
            s.push_str(&format!("{}  <- [{}]\n", w.piece, w.rule));
        } else {
            s.push_str(&format!("{}  <- [{}]  <- \"{}\"\n", w.piece, w.rule, w.words));
        }
    };
    s.push_str("\n== decisions\n");
    for w in &d.why {
        row(&mut s, w);
    }
    s.push_str("\n== art (assets/library.toml)\n");
    for w in &art.why {
        row(&mut s, w);
    }
    if !lv.why.is_empty() {
        s.push_str("\n== level\n");
        for w in &lv.why {
            row(&mut s, w);
        }
    }
    s.push_str(&format!("\n== level: {} entities, seed {:016x}\n", lv.entities.len(), d.seed));
    s.push_str("\n== behavior.txt\n");
    for (i, l) in lines.iter().enumerate() {
        s.push_str(&format!("{:>2}. [{}] {}\n", i + 1, l.rule, l.text));
    }
    s
}

/// What `droid explain` prints: the request with provenance, what each rule weighed, the finalists.
pub fn explain(sentence: &str, cfg: &Config, lib: &Library, lenient: bool) -> Result<String, Vec<String>> {
    let (req, out) = plan(sentence, cfg, lenient)?;
    let mut s = String::new();
    s.push_str(&format!("sentence: {}\n\nrequest (value <- words):\n{}", req.sentence, describe(&req)));
    let p = rules::profile(&req, cfg);
    s.push_str(&format!("\nprofile [challenge, pace, simplicity] = [{:.2}, {:.2}, {:.2}]\n", p[0], p[1], p[2]));
    s.push_str("\nrules (variant score, * = named by the sentence):\n");
    for w in &out.weighed {
        let offers: Vec<String> = w.offers.iter().map(|(v, sc, ev)| format!("{v} {sc:.2}{}", if ev.is_some() { "*" } else { "" })).collect();
        s.push_str(&format!("  {:<15} {}   ({})\n", w.rule, offers.join("  "), w.why));
    }
    s.push_str(&format!("\n{} partial solutions explored; finalists, best first:\n", out.explored));
    for (i, c) in out.finalists.iter().enumerate() {
        let ch: Vec<String> = c.chosen.iter().map(|(k, v)| format!("{k}={v}")).collect();
        s.push_str(&format!("  {}. score {:.3}: {}\n", i + 1, c.score, ch.join(" ")));
        for (v, n) in &c.judges {
            s.push_str(&format!("       judge {v:+.2}: {n}\n"));
        }
    }
    if let Some(best) = out.finalists.first() {
        match assemble(&req, cfg, lib, best, "explain") {
            Ok(g) => {
                s.push('\n');
                s.push_str(&g.files["why.txt"]);
            }
            Err(e) => s.push_str(&format!("\nthe best finalist fails its checks: {e}\n")),
        }
    }
    Ok(s)
}

/// The request, one value per line with the words behind it.
pub fn describe(r: &Request) -> String {
    let mut s = String::new();
    let mut line = |k: &str, v: String, w: &str| s.push_str(&format!("  {k:<10} {v}  <- \"{w}\"\n"));
    if let Some(g) = &r.genre {
        line("genre", g.v.name().into(), &g.words);
    }
    if let Some(v) = &r.view {
        line("view", v.v.name().into(), &v.words);
    }
    if let Some(p) = &r.player {
        line("player", p.v.name().into(), &p.words);
    }
    if let Some(c) = &r.player_color {
        line("player", c.v.name().into(), &c.words);
    }
    for e in &r.enemies {
        let mut v = e.kind.v.name().to_string();
        if let Some(c) = &e.count {
            v.push_str(&format!(" count {:?} <- \"{}\"", c.v, c.words));
        }
        if let Some(rw) = &e.rows {
            v.push_str(&format!(" rows {} <- \"{}\"", rw.v, rw.words));
        }
        if let Some(c) = &e.color {
            v.push_str(&format!(" {} <- \"{}\"", c.v.name(), c.words));
        }
        for b in &e.behaviors {
            v.push_str(&format!(" {} <- \"{}\"", b.v.name(), b.words));
        }
        if let Some(sh) = &e.shoots {
            v.push_str(&format!(" shoots <- \"{}\"", sh.words));
        }
        line("enemy", v, &e.kind.words);
    }
    if let Some(g) = &r.generic_enemies {
        line("enemies", format!("{:?}", g.v), &g.words);
    }
    for b in &r.generic_behaviors {
        line("enemies", b.v.name().into(), &b.words);
    }
    if let Some(n) = &r.no_enemies {
        line("enemies", "none".into(), &n.words);
    }
    for p in &r.pickups {
        let mut v = p.kind.v.name().to_string();
        if let Some(c) = &p.count {
            v.push_str(&format!(" count {:?} <- \"{}\"", c.v, c.words));
        }
        if let Some(c) = &p.color {
            v.push_str(&format!(" {} <- \"{}\"", c.v.name(), c.words));
        }
        line("pickup", v, &p.kind.words);
    }
    if let Some(g) = &r.generic_pickups {
        line("pickups", format!("{:?}", g.v), &g.words);
    }
    for h in &r.hazards {
        line("hazard", h.kind.v.name().into(), &h.kind.words);
    }
    if let Some(x) = &r.exit {
        line("exit", x.v.name().into(), &x.words);
    }
    for g in &r.goals {
        line("goal", format!("{:?}", g.v), &g.words);
    }
    for a in &r.abilities {
        line("ability", a.v.name().into(), &a.words);
    }
    if let Some(t) = &r.theme {
        line("theme", t.v.name().into(), &t.words);
    }
    if let Some(t) = &r.time {
        line("time", t.v.name().into(), &t.words);
    }
    if let Some(t) = &r.style {
        line("style", t.v.name().into(), &t.words);
    }
    for m in &r.mods {
        line("modifier", m.v.name().into(), &m.words);
    }
    if let Some(l) = &r.lives {
        line("lives", l.v.to_string(), &l.words);
    }
    for e in &r.edits {
        line("edit", format!("{} {}", e.v.0.name(), e.v.1.name()), &e.words);
    }
    s
}

/// The result of `droid make`.
pub struct Made {
    pub slug: String,
    pub dir: PathBuf,
    /// Which finalist was written (1-based) and why earlier ones were dropped.
    pub finalist: usize,
    pub dropped: Vec<String>,
    pub report: Option<serde_json::Value>,
    pub files: usize,
}

fn write_to(dir: &Path, g: &Generated) -> Result<(), String> {
    let _ = std::fs::remove_dir_all(dir);
    for (rel, text) in &g.files {
        let p = dir.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&p, text).map_err(|e| format!("cannot write {}: {e}", p.display()))?;
    }
    for (src, rel) in &g.copies {
        let p = dir.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::copy(src, &p).map_err(|e| format!("cannot copy {} to {}: {e}", src.display(), p.display()))?;
    }
    Ok(())
}

/// `droid make`: plan, then try finalists best first until one passes every check; write it to
/// `<root>/games/<slug>/` (staged in `<root>/target/droid-make/<slug>/` and moved into place
/// only after the checks pass). If none passes, nothing is written and every reason is returned.
pub fn make(root: &Path, name: &str, sentence: &str, cfg: &Config, lib: &Library, opts: &Options) -> Result<Made, Vec<String>> {
    let slug = slug(name).map_err(|e| vec![format!("error: {e}")])?;
    let (req, out) = plan(sentence, cfg, opts.lenient)?;
    let stage = root.join("target").join("droid-make").join(&slug);
    let mut dropped = Vec::new();
    for (i, cand) in out.finalists.iter().enumerate() {
        let g = match assemble(&req, cfg, lib, cand, &slug) {
            Ok(g) => g,
            Err(e) => {
                dropped.push(format!("finalist {} dropped: {e}", i + 1));
                continue;
            }
        };
        write_to(&stage, &g).map_err(|e| vec![e])?;
        let mut report = None;
        if opts.build {
            match checks::build_and_sim(root, &stage, &slug, cfg) {
                Ok(v) => report = Some(v),
                Err(e) => {
                    dropped.push(format!("finalist {} dropped: {e}", i + 1));
                    continue;
                }
            }
            // back to the workspace form
            std::fs::write(stage.join("Cargo.toml"), &g.files["Cargo.toml"]).map_err(|e| vec![e.to_string()])?;
            let _ = std::fs::remove_file(stage.join("Cargo.lock"));
        }
        let dir = root.join("games").join(&slug);
        if dir.exists() {
            std::fs::remove_dir_all(&dir).map_err(|e| vec![format!("cannot replace {}: {e}", dir.display())])?;
        }
        std::fs::create_dir_all(root.join("games")).map_err(|e| vec![e.to_string()])?;
        if std::fs::rename(&stage, &dir).is_err() {
            write_to(&dir, &g).map_err(|e| vec![e])?;
            let _ = std::fs::remove_dir_all(&stage);
        }
        return Ok(Made { slug, dir, finalist: i + 1, dropped, report, files: g.files.len() + g.copies.len() });
    }
    let _ = std::fs::remove_dir_all(&stage);
    let mut errs = vec![format!("error: none of the {} finalists passed its checks; nothing was written", out.finalists.len())];
    errs.extend(dropped);
    Err(errs)
}

/// Every word the sentence language knows.
pub fn vocabulary() -> Vec<&'static str> {
    lexicon::vocabulary()
}

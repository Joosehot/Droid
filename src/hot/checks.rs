//! Checks on a finalist before anything is written (DROID_DESIGN.md section 5):
//! 1. Senne compiles the behaviour (its diagnostics verbatim),
//! 2. reachability: a jump graph for platformers, a flood fill for arenas,
//! 3. spawn safety, chase fairness, entity budget,
//! 4. the game crate builds with `-D warnings`,
//! 5. a headless run (`DROID_SIM`) ends without error, not lost, with a player.
//!
//! 1-3 are pure and fast; 4-5 run cargo and the game (see [`build_and_sim`]).

use super::config::Config;
use super::design::Design;
use super::level::{reach_at, jump_reach, Level, Rect};
use super::request::*;
use std::path::{Path, PathBuf};

/// Senne: behaviour sentences -> Rust. Its diagnostics are passed on verbatim.
pub fn senne(behavior: &str, struct_name: &str) -> Result<String, String> {
    let cfg = senne::config::Config::builtin();
    let opts = senne::game::GameOptions { struct_name: struct_name.to_string(), lenient: false, comments: true };
    match senne::game::compile(behavior, &cfg, &opts) {
        Ok(c) => Ok(c.code),
        Err(diags) => Err(format!("Senne rejected the behaviour:\n{}", diags.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("\n"))),
    }
}

fn has_tag(e: &crate::spec::EntitySpec, t: &str) -> bool {
    e.tags.iter().any(|x| x == t)
}

/// Checks 2 and 3.
pub fn level(d: &Design, cfg: &Config, lv: &Level) -> Result<(), String> {
    if lv.entities.len() > cfg.checks.max_entities {
        return Err(format!("{} entities is over the budget of {} (checks.max_entities)", lv.entities.len(), cfg.checks.max_entities));
    }
    // spawn safety
    let plane = |p: [f32; 3]| if d.depth { (p[0], p[2]) } else { (p[0], p[1]) };
    let (sx, sy) = plane(lv.spawn);
    for e in &lv.entities {
        if has_tag(e, "enemy") || has_tag(e, "hazard") {
            let (x, y) = plane(e.position);
            let dist = ((x - sx).powi(2) + (y - sy).powi(2)).sqrt();
            if dist < cfg.checks.spawn_clear {
                return Err(format!("{} is {dist:.1} units from the player's start (checks.spawn_clear = {})", e.name, cfg.checks.spawn_clear));
            }
        }
    }
    // chase fairness
    let hard = d.profile[0] > cfg.profile.default[0];
    let ratio = cfg.checks.max_chase_ratio * if hard { 1.2 } else { 1.0 };
    for e in &d.enemies {
        if e.behavior == Behavior::Chase && e.speed > d.speed * ratio + 1e-3 {
            return Err(format!("{}s chase at {:.2} but the player runs at {:.2} (checks.max_chase_ratio = {})", e.kind.name(), e.speed, d.speed, cfg.checks.max_chase_ratio));
        }
    }
    match d.genre {
        Genre::Platformer => jump_graph(d, cfg, lv),
        Genre::Arena => flood(d, cfg, lv),
        Genre::Shooter => Ok(()),
    }
}

/// Platformer: from the spawn surface, which surfaces can the player jump to? Every pickup and
/// the exit must be within a jump of one of them.
fn jump_graph(d: &Design, cfg: &Config, lv: &Level) -> Result<(), String> {
    let m = cfg.checks.reach_margin;
    let (h, reach) = jump_reach(d);
    let ph = lv.player_size[1];
    let s: Vec<&super::level::Surface> = lv.surfaces.iter().filter(|s| !s.hazard).collect();
    let foot = lv.spawn[1] - ph / 2.0;
    let start = s
        .iter()
        .position(|x| lv.spawn[0] >= x.x0 && lv.spawn[0] <= x.x1 && (x.top - foot).abs() < 0.2)
        .ok_or("the player doesn't start on the ground")?;
    let mut seen = vec![false; s.len()];
    seen[start] = true;
    let mut todo = vec![start];
    while let Some(i) = todo.pop() {
        for j in 0..s.len() {
            if seen[j] {
                continue;
            }
            let dh = s[j].top - s[i].top;
            if dh > h * m {
                continue;
            }
            let gap = (s[j].x0 - s[i].x1).max(s[i].x0 - s[j].x1).max(0.0);
            if gap <= reach_at(d, dh) * m {
                seen[j] = true;
                todo.push(j);
            }
        }
    }
    let reachable = |x: f32, y: f32| {
        s.iter().zip(&seen).any(|(sf, ok)| {
            if !*ok {
                return false;
            }
            let dx = (sf.x0 - x).max(x - sf.x1).max(0.0);
            let dy = y - sf.top;
            dy <= h * m + ph && dx <= reach * m / 2.0
        })
    };
    for e in &lv.entities {
        if has_tag(e, "pickup") || has_tag(e, "exit") {
            if !reachable(e.position[0], e.position[1]) {
                return Err(format!("{} at ({:.1}, {:.1}) can't be reached with a {:.2}-unit jump", e.name, e.position[0], e.position[1], h));
            }
        }
    }
    Ok(())
}

/// Arena: flood fill the floor from the spawn; every pickup must be on a reached cell.
fn flood(d: &Design, cfg: &Config, lv: &Level) -> Result<(), String> {
    let c = cfg.layout.arena.clear_cell;
    let b = lv.bounds;
    let r = if d.depth { lv.player_size[0].max(lv.player_size[2]) } else { lv.player_size[0].max(lv.player_size[1]) } * 0.45;
    let nx = ((b.x1 - b.x0) / c).ceil() as i32;
    let ny = ((b.y1 - b.y0) / c).ceil() as i32;
    let center = |i: i32, j: i32| (b.x0 + (i as f32 + 0.5) * c, b.y0 + (j as f32 + 0.5) * c);
    let blocked: Vec<Rect> = lv.blocks.iter().map(|x| x.grow(r)).collect();
    let open = |i: i32, j: i32| {
        let (x, y) = center(i, j);
        i >= 0 && j >= 0 && i < nx && j < ny && !blocked.iter().any(|bl| bl.contains(x, y))
    };
    let cell = |x: f32, y: f32| (((x - b.x0) / c).floor() as i32, ((y - b.y0) / c).floor() as i32);
    let plane = |p: [f32; 3]| if d.depth { (p[0], p[2]) } else { (p[0], p[1]) };
    let (sx, sy) = plane(lv.spawn);
    let s = cell(sx, sy);
    if !open(s.0, s.1) {
        return Err("the player starts inside a wall".into());
    }
    let mut seen = vec![false; (nx * ny) as usize];
    let idx = |i: i32, j: i32| (j * nx + i) as usize;
    seen[idx(s.0, s.1)] = true;
    let mut todo = vec![s];
    while let Some((i, j)) = todo.pop() {
        for (di, dj) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (a, bb) = (i + di, j + dj);
            if open(a, bb) && !seen[idx(a, bb)] {
                seen[idx(a, bb)] = true;
                todo.push((a, bb));
            }
        }
    }
    for e in lv.entities.iter().filter(|e| has_tag(e, "pickup")) {
        let (x, y) = plane(e.position);
        let (ci, cj) = cell(x, y);
        let near = (-2..=2).any(|di| (-2..=2).any(|dj| {
            let (a, bb) = (ci + di, cj + dj);
            a >= 0 && bb >= 0 && a < nx && bb < ny && seen[idx(a, bb)]
        }));
        if !near {
            return Err(format!("{} at ({x:.1}, {y:.1}) is walled off from the player", e.name));
        }
    }
    Ok(())
}

/// The Cargo.toml of a game crate inside the Droid workspace.
pub fn crate_toml(slug: &str) -> String {
    format!("[package]\nname = \"{slug}\"\nversion = \"0.1.0\"\nedition = \"2021\"\npublish = false\n\n[dependencies]\ndroid = {{ path = \"../..\" }}\n")
}

/// Checks 4 and 5: build the game crate in `dir` (a staging copy with its own workspace, sharing
/// Droid's target folder so nothing is rebuilt twice) with `-D warnings`, then run it headless.
pub fn build_and_sim(root: &Path, dir: &Path, slug: &str, cfg: &Config) -> Result<serde_json::Value, String> {
    // A standalone workspace: the Droid path is absolute, the lock file and profiles are Droid's.
    let droid = root.to_string_lossy().replace('\\', "/");
    // its own binary name, so the staged build never stands in for the real game's binary
    // (the data path is compiled in: the staged binary reads the staging folder)
    let bin = format!("droid-stage-{slug}");
    let profiles = std::fs::read_to_string(root.join("Cargo.toml"))
        .ok()
        .and_then(|s| s.parse::<toml::Table>().ok())
        .and_then(|t| t.get("profile").cloned())
        .map(|p| {
            let mut t = toml::Table::new();
            t.insert("profile".into(), p);
            toml::to_string(&t).unwrap_or_default()
        })
        .unwrap_or_default();
    let manifest = format!(
        "[package]\nname = \"{slug}\"\nversion = \"0.1.0\"\nedition = \"2021\"\npublish = false\n\n[[bin]]\nname = \"{bin}\"\npath = \"src/main.rs\"\n\n[dependencies]\ndroid = {{ path = \"{droid}\" }}\n\n[workspace]\n\n{profiles}"
    );
    std::fs::write(dir.join("Cargo.toml"), manifest).map_err(|e| e.to_string())?;
    if let Ok(lock) = std::fs::read(root.join("Cargo.lock")) {
        let _ = std::fs::write(dir.join("Cargo.lock"), lock);
    }
    let target = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from).unwrap_or_else(|| root.join("target"));
    let out = std::process::Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(["rustc", "--quiet", "--bin", &bin, "--manifest-path"])
        .arg(dir.join("Cargo.toml"))
        .args(["--", "-D", "warnings"])
        .env("CARGO_TARGET_DIR", &target)
        .env_remove("RUSTFLAGS")
        .output()
        .map_err(|e| format!("cannot run cargo: {e}"))?;
    if !out.status.success() {
        return Err(format!("the game crate does not build with -D warnings:\n{}", String::from_utf8_lossy(&out.stderr)));
    }
    let exe = target.join("debug").join(format!("{bin}{}", std::env::consts::EXE_SUFFIX));
    let out = std::process::Command::new(&exe)
        .env("DROID_SIM", format!("{}", cfg.checks.sim_seconds))
        .output()
        .map_err(|e| format!("cannot run {}: {e}", exe.display()))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let line = stdout.lines().rev().find(|l| l.trim_start().starts_with('{')).ok_or_else(|| format!("the game printed no report:\n{stdout}\n{}", String::from_utf8_lossy(&out.stderr)))?;
    let v: serde_json::Value = serde_json::from_str(line).map_err(|e| format!("bad report {line:?}: {e}"))?;
    if !v["error"].is_null() {
        return Err(format!("the game failed while running: {}", v["error"]));
    }
    if v["status"] == "lost" {
        return Err(format!("an idle player loses within {} seconds (checks.sim_seconds): {v}", cfg.checks.sim_seconds));
    }
    if v["player"].is_null() {
        return Err("the player vanished in the headless run".into());
    }
    Ok(v)
}

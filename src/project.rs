//! Game folder format. A game is a directory:
//!
//! ```text
//! game.toml          title, view, camera, gravity, background
//! scene.toml         [[entities]] and [[texts]]
//! script.rhai        main script (plus optional scripts/*.rhai, appended in name order)
//! sprites/NAME.txt   palette lines `c = r g b` (or `#rrggbb`), a `---` line, then pixel rows
//! models/NAME.toml   resolution + [[parts]]
//! ```
//!
//! `FileSet` is the in-memory form (relative path -> text) used by the loader, the saver
//! and the AI patcher alike, so the same code validates what the AI produced.

use crate::spec::{CameraSpec, EntitySpec, GameSpec, ModelSpec, SpriteSpec, TextSpec, View};
use anyhow::{anyhow, bail, Context};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

pub type FileSet = BTreeMap<String, String>;

#[derive(Serialize, Deserialize)]
struct GameMeta {
    title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    description: String,
    #[serde(default)]
    view: View,
    #[serde(default)]
    camera: CameraSpec,
    #[serde(default = "bg")]
    background: [f32; 3],
    #[serde(default = "grav")]
    gravity: [f32; 3],
}
fn bg() -> [f32; 3] { [0.08, 0.09, 0.12] }
fn grav() -> [f32; 3] { [0.0, -20.0, 0.0] }

#[derive(Serialize, Deserialize, Default)]
struct Scene {
    #[serde(default)]
    entities: Vec<EntitySpec>,
    #[serde(default)]
    texts: Vec<TextSpec>,
}

// ---------------------------------------------------------------- sprites as text

pub fn sprite_to_text(s: &SpriteSpec) -> String {
    let mut out = String::new();
    for (c, rgb) in &s.palette {
        out.push_str(&format!("{c} = {:.3} {:.3} {:.3}\n", rgb[0], rgb[1], rgb[2]));
    }
    out.push_str("---\n");
    for row in &s.pixels {
        out.push_str(row);
        out.push('\n');
    }
    out
}

pub fn sprite_from_text(text: &str) -> anyhow::Result<SpriteSpec> {
    let mut spec = SpriteSpec::default();
    let mut in_pixels = false;
    for (ln, raw) in text.lines().enumerate() {
        let line = raw.trim_end();
        if !in_pixels {
            let t = line.trim();
            if t == "---" {
                in_pixels = true;
                continue;
            }
            if t.is_empty() || t.starts_with('#') && !t[1..].trim_start().starts_with('=') {
                continue;
            }
            let (c, v) = t.split_once('=').ok_or_else(|| anyhow!("line {}: expected `char = r g b` or `---`", ln + 1))?;
            let c = c.trim();
            if c.chars().count() != 1 {
                bail!("line {}: palette key must be a single character, got {c:?}", ln + 1);
            }
            spec.palette.insert(c.to_string(), parse_color(v.trim()).with_context(|| format!("line {}", ln + 1))?);
        } else {
            if line.trim().is_empty() {
                continue;
            }
            spec.pixels.push(line.to_string());
        }
    }
    if !in_pixels {
        bail!("sprite needs a `---` line between the palette and the pixel rows");
    }
    let w = spec.pixels.iter().map(|r| r.chars().count()).max().unwrap_or(0);
    for row in &mut spec.pixels {
        let n = row.chars().count();
        if n < w {
            row.extend(std::iter::repeat('.').take(w - n));
        }
    }
    for row in &spec.pixels {
        for ch in row.chars() {
            if ch != '.' && ch != ' ' && !spec.palette.contains_key(&ch.to_string()) {
                bail!("pixel character {ch:?} is not in the palette");
            }
        }
    }
    Ok(spec)
}

fn parse_color(v: &str) -> anyhow::Result<[f32; 3]> {
    if let Some(hex) = v.strip_prefix('#') {
        if hex.len() == 6 {
            let p = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map(|b| b as f32 / 255.0);
            return Ok([p(0)?, p(2)?, p(4)?]);
        }
        bail!("bad hex colour {v:?}");
    }
    let nums: Vec<f32> = v.split(|c: char| c == ' ' || c == ',').filter(|s| !s.is_empty()).map(|s| s.parse::<f32>()).collect::<Result<_, _>>().map_err(|_| anyhow!("bad colour {v:?}"))?;
    if nums.len() != 3 {
        bail!("colour needs 3 numbers, got {v:?}");
    }
    Ok([nums[0], nums[1], nums[2]])
}

// ---------------------------------------------------------------- FileSet <-> GameSpec

/// TOML with inline arrays and floats rounded to 4 decimals (no f32 noise like 0.0799999).
fn to_toml<T: Serialize>(v: &T) -> anyhow::Result<String> {
    let mut json = serde_json::to_value(v)?;
    round_floats(&mut json);
    let value: toml::Value = serde_json::from_value::<toml::Value>(json)?;
    Ok(toml::to_string(&value)?)
}

fn round_floats(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                if !n.is_i64() && !n.is_u64() {
                    let r = (f * 10000.0).round() / 10000.0;
                    if let Some(nn) = serde_json::Number::from_f64(r) {
                        *n = nn;
                    }
                }
            }
        }
        serde_json::Value::Array(a) => a.iter_mut().for_each(round_floats),
        serde_json::Value::Object(o) => o.values_mut().for_each(round_floats),
        _ => {}
    }
}

pub fn to_files(spec: &GameSpec) -> anyhow::Result<FileSet> {
    let mut files = FileSet::new();
    let meta = GameMeta {
        title: spec.title.clone(),
        description: spec.description.clone(),
        view: spec.view,
        camera: spec.camera.clone(),
        background: spec.background,
        gravity: spec.gravity,
    };
    files.insert("game.toml".into(), to_toml(&meta)?);
    let scene = Scene { entities: spec.entities.clone(), texts: spec.texts.clone() };
    files.insert("scene.toml".into(), to_toml(&scene)?);
    let mut script = spec.script.clone();
    if !script.ends_with('\n') {
        script.push('\n');
    }
    files.insert("script.rhai".into(), script);
    for (name, s) in &spec.sprites {
        files.insert(format!("sprites/{name}.txt"), sprite_to_text(s));
    }
    for (name, m) in &spec.models {
        files.insert(format!("models/{name}.toml"), to_toml(m)?);
    }
    Ok(files)
}

pub fn from_files(files: &FileSet) -> anyhow::Result<GameSpec> {
    let meta_src = files.get("game.toml").ok_or_else(|| anyhow!("missing game.toml"))?;
    let meta: GameMeta = toml::from_str(meta_src).map_err(|e| anyhow!("game.toml: {e}"))?;
    let scene: Scene = match files.get("scene.toml") {
        Some(s) => toml::from_str(s).map_err(|e| anyhow!("scene.toml: {e}"))?,
        None => Scene::default(),
    };
    let mut script = files.get("script.rhai").cloned().unwrap_or_default();
    for (path, src) in files {
        if path.starts_with("scripts/") && path.ends_with(".rhai") {
            script.push('\n');
            script.push_str(src);
        }
    }
    let mut sprites = BTreeMap::new();
    let mut models = BTreeMap::new();
    for (path, src) in files {
        if let Some(name) = path.strip_prefix("sprites/").and_then(|p| p.strip_suffix(".txt")) {
            sprites.insert(name.to_string(), sprite_from_text(src).map_err(|e| anyhow!("{path}: {e}"))?);
        } else if let Some(name) = path.strip_prefix("models/").and_then(|p| p.strip_suffix(".toml")) {
            let m: ModelSpec = toml::from_str(src).map_err(|e| anyhow!("{path}: {e}"))?;
            models.insert(name.to_string(), m);
        }
    }
    Ok(GameSpec {
        title: meta.title,
        description: meta.description,
        view: meta.view,
        camera: meta.camera,
        background: meta.background,
        gravity: meta.gravity,
        sprites,
        models,
        entities: scene.entities,
        texts: scene.texts,
        script,
    })
}

// ---------------------------------------------------------------- disk

const EXTS: [&str; 3] = ["toml", "rhai", "txt"];

pub fn read_dir(dir: &Path) -> anyhow::Result<FileSet> {
    let mut files = FileSet::new();
    fn walk(base: &Path, dir: &Path, files: &mut FileSet) -> anyhow::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let p = entry?.path();
            if p.is_dir() {
                walk(base, &p, files)?;
            } else if p.extension().and_then(|e| e.to_str()).map_or(false, |e| EXTS.contains(&e)) {
                let rel = p.strip_prefix(base)?.to_string_lossy().replace('\\', "/");
                files.insert(rel, std::fs::read_to_string(&p)?);
            }
        }
        Ok(())
    }
    walk(dir, dir, &mut files)?;
    if files.is_empty() {
        bail!("{} contains no game files", dir.display());
    }
    Ok(files)
}

/// Writes the file set, only touching files whose content changed, and removes stale
/// sprite/model files that are no longer part of the game.
pub fn write_dir(dir: &Path, files: &FileSet) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    for (rel, content) in files {
        let p = dir.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if std::fs::read_to_string(&p).map_or(true, |old| old != *content) {
            std::fs::write(&p, content)?;
        }
    }
    for sub in ["sprites", "models", "scripts"] {
        let d = dir.join(sub);
        if let Ok(rd) = std::fs::read_dir(&d) {
            for entry in rd.flatten() {
                let p = entry.path();
                let rel = p.strip_prefix(dir).map(|r| r.to_string_lossy().replace('\\', "/")).unwrap_or_default();
                if p.is_file() && !files.contains_key(&rel) {
                    let _ = std::fs::remove_file(&p);
                }
            }
        }
    }
    Ok(())
}

/// A modification stamp for a game folder: newest mtime plus file count (catches deletions).
pub fn stamp(dir: &Path) -> (Option<std::time::SystemTime>, usize) {
    let mut newest = None;
    let mut count = 0;
    fn walk(dir: &Path, newest: &mut Option<std::time::SystemTime>, count: &mut usize) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for entry in rd.flatten() {
            let p = entry.path();
            if p.is_dir() {
                walk(&p, newest, count);
            } else if let Ok(m) = entry.metadata().and_then(|m| m.modified()) {
                *count += 1;
                if newest.map_or(true, |n| m > n) {
                    *newest = Some(m);
                }
            }
        }
    }
    walk(dir, &mut newest, &mut count);
    (newest, count)
}

/// Loads a game from either a folder or a legacy single `game.json`.
pub fn load(path: &Path) -> anyhow::Result<GameSpec> {
    if path.is_dir() {
        from_files(&read_dir(path)?)
    } else {
        let s = std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
        GameSpec::from_json(&s)
    }
}

/// Saves a game: folders get the multi-file layout, a `.json` path keeps the legacy format.
pub fn save(path: &Path, spec: &GameSpec) -> anyhow::Result<()> {
    if path.extension().map_or(false, |e| e == "json") {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, spec.to_json())?;
        Ok(())
    } else {
        write_dir(path, &to_files(spec)?)
    }
}

// ---------------------------------------------------------------- AI bundle format

/// The multi-file text format the AI writes. Full files:
/// ```text
/// === file: script.rhai ===
/// ...contents...
/// === file: sprites/hero.txt ===
/// ...
/// === delete: sprites/old.txt ===
/// ```
/// and in-place edits:
/// ```text
/// === edit: script.rhai ===
/// <<<<<<< FIND
/// exact existing text
/// =======
/// replacement text
/// >>>>>>> REPLACE
/// ```
pub const BUNDLE_FORMAT: &str = r#"Write files using this exact bundle format (no markdown fences around it):

=== file: game.toml ===
(full contents)
=== file: scene.toml ===
(full contents)
=== file: script.rhai ===
(full contents)
=== file: sprites/hero.txt ===
(full contents)

For changes to an existing game, include only the files you change. Small changes inside a file use an edit section instead of resending the file:

=== edit: script.rhai ===
<<<<<<< FIND
exact text that exists in the file now
=======
the replacement text
>>>>>>> REPLACE

(any number of FIND/REPLACE blocks per edit section; FIND must match exactly once, whitespace included)
=== delete: sprites/unused.txt ===
"#;

pub enum BundleItem {
    File(String, String),
    Edit(String, Vec<(String, String)>),
    Delete(String),
}

pub fn parse_bundle(text: &str) -> anyhow::Result<Vec<BundleItem>> {
    let mut items = Vec::new();
    let mut current: Option<(String, String, String)> = None; // (kind, path, body)
    let flush = |current: &mut Option<(String, String, String)>, items: &mut Vec<BundleItem>| -> anyhow::Result<()> {
        if let Some((kind, path, body)) = current.take() {
            let body = body.trim_start_matches('\n').trim_end().to_string() + "\n";
            match kind.as_str() {
                "file" => items.push(BundleItem::File(path, body)),
                "delete" => items.push(BundleItem::Delete(path)),
                "edit" => items.push(BundleItem::Edit(path.clone(), parse_edits(&body).with_context(|| format!("edit section for {path}"))?)),
                _ => unreachable!(),
            }
        }
        Ok(())
    };
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("=== ").and_then(|r| r.strip_suffix(" ===")) {
            if let Some((kind, path)) = rest.split_once(':') {
                let kind = kind.trim();
                if ["file", "edit", "delete"].contains(&kind) {
                    flush(&mut current, &mut items)?;
                    let path = path.trim().trim_matches('`').replace('\\', "/");
                    if path.contains("..") || path.starts_with('/') {
                        bail!("invalid path {path:?}");
                    }
                    current = Some((kind.to_string(), path, String::new()));
                    continue;
                }
            }
        }
        if let Some((_, _, body)) = current.as_mut() {
            body.push_str(line);
            body.push('\n');
        }
    }
    flush(&mut current, &mut items)?;
    if items.is_empty() {
        bail!("no `=== file: ... ===` sections found in the reply");
    }
    Ok(items)
}

fn parse_edits(body: &str) -> anyhow::Result<Vec<(String, String)>> {
    let mut edits = Vec::new();
    let mut state = 0; // 0 outside, 1 in find, 2 in replace
    let (mut find, mut replace) = (String::new(), String::new());
    for line in body.lines() {
        let t = line.trim_end();
        match (state, t.trim()) {
            (0, s) if s.starts_with("<<<<<<<") => {
                state = 1;
                find.clear();
                replace.clear();
            }
            (1, "=======") => state = 2,
            (2, s) if s.starts_with(">>>>>>>") => {
                state = 0;
                edits.push((strip_last_nl(&find), strip_last_nl(&replace)));
            }
            (1, _) => {
                find.push_str(line);
                find.push('\n');
            }
            (2, _) => {
                replace.push_str(line);
                replace.push('\n');
            }
            _ => {}
        }
    }
    if state != 0 {
        bail!("unterminated FIND/REPLACE block");
    }
    if edits.is_empty() {
        bail!("no FIND/REPLACE blocks");
    }
    Ok(edits)
}

fn strip_last_nl(s: &str) -> String {
    s.strip_suffix('\n').unwrap_or(s).to_string()
}

/// Applies a parsed bundle to a file set (in place).
pub fn apply_bundle(files: &mut FileSet, items: &[BundleItem]) -> anyhow::Result<()> {
    for item in items {
        match item {
            BundleItem::File(path, body) => {
                files.insert(path.clone(), body.clone());
            }
            BundleItem::Delete(path) => {
                if path == "game.toml" || path == "scene.toml" || path == "script.rhai" {
                    bail!("cannot delete {path}");
                }
                files.remove(path);
            }
            BundleItem::Edit(path, edits) => {
                let content = files.get(path).ok_or_else(|| anyhow!("edit target {path} does not exist; write it as a full file instead"))?;
                let mut content = content.clone();
                for (i, (find, replace)) in edits.iter().enumerate() {
                    let n = content.matches(find.as_str()).count();
                    if n == 0 {
                        // Retry ignoring trailing whitespace differences per line.
                        let norm = |s: &str| s.lines().map(|l| l.trim_end()).collect::<Vec<_>>().join("\n");
                        let (nc, nf) = (norm(&content), norm(find));
                        if nf.is_empty() || nc.matches(nf.as_str()).count() != 1 {
                            bail!("{path} edit {}: FIND text not found (must match the current file exactly). FIND was:\n{find}", i + 1);
                        }
                        content = nc.replacen(nf.as_str(), &norm(replace), 1) + "\n";
                        continue;
                    }
                    if n > 1 {
                        bail!("{path} edit {}: FIND text occurs {n} times; include more surrounding lines so it is unique", i + 1);
                    }
                    content = content.replacen(find.as_str(), replace, 1);
                }
                files.insert(path.clone(), content);
            }
        }
    }
    Ok(())
}

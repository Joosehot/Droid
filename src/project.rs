//! Game folder format. A game is a directory:
//!
//! ```text
//! game.toml          title, view, camera, gravity, background
//! scene.toml         [[entities]] and [[texts]]
//! script.rhai        main script (plus optional scripts/*.rhai, appended in name order)
//! sprites/NAME.txt   palette lines `c = r g b` (or `#rrggbb`), a `---` line, then pixel rows
//! sprites/NAME.toml  an image sprite (`file = "images/x.png"` or `sheet` + `tile` + `tile_size`,
//!                    or `stack`) or a tiling rule (`[tiles]` fill/top/left/right/size)
//! sprites/NAME.png   a whole PNG as sprite NAME
//! models/NAME.toml   resolution + [[parts]], or `file = "models/x.glb"` (+ `groups`, `texture`)
//! models/NAME.glb    a glTF/GLB (or .obj) file as model NAME (groups guessed from node names)
//! images/...         PNG files referenced by sprites
//! ```
//!
//! Paths inside the files are relative to the game folder.
//!
//! `FileSet` is the in-memory form (relative path -> text) used by the loader and the saver.

use crate::spec::{CameraSpec, EntitySpec, GameSpec, ImageRef, ModelSpec, Rgba, SpriteSpec, TextSpec, TileRule, View};
use std::sync::Arc;
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

/// `sprites/NAME.toml`: an image reference, or a `[tiles]` rule.
pub fn sprite_from_toml(text: &str) -> anyhow::Result<SpriteSpec> {
    let mut table: toml::Table = toml::from_str(text)?;
    let mut spec = SpriteSpec::default();
    if let Some(t) = table.remove("tiles") {
        spec.tiles = Some(t.try_into::<TileRule>().map_err(|e| anyhow!("[tiles]: {e}"))?);
    }
    if !table.is_empty() || spec.tiles.is_none() {
        let image: ImageRef = toml::Value::Table(table).try_into().map_err(|e| anyhow!("{e}"))?;
        if image == ImageRef::default() {
            bail!("a sprite .toml needs `file`, `sheet` + `tile`, `stack` or `[tiles]`");
        }
        spec.image = Some(image);
    }
    Ok(spec)
}

pub fn sprite_to_toml(s: &SpriteSpec) -> anyhow::Result<String> {
    let mut table = match &s.image {
        Some(img) => match toml::Value::try_from(img)? {
            toml::Value::Table(t) => t,
            _ => toml::Table::new(),
        },
        None => toml::Table::new(),
    };
    if let Some(t) = &s.tiles {
        table.insert("tiles".into(), toml::Value::try_from(t)?);
    }
    Ok(toml::to_string(&table)?)
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
pub fn to_toml<T: Serialize>(v: &T) -> anyhow::Result<String> {
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
        if s.image.is_some() || s.tiles.is_some() {
            files.insert(format!("sprites/{name}.toml"), sprite_to_toml(s)?);
        } else {
            files.insert(format!("sprites/{name}.txt"), sprite_to_text(s));
        }
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
        } else if let Some(name) = path.strip_prefix("sprites/").and_then(|p| p.strip_suffix(".toml")) {
            if sprites.contains_key(name) {
                bail!("sprite '{name}' is defined twice (sprites/{name}.txt and sprites/{name}.toml)");
            }
            sprites.insert(name.to_string(), sprite_from_toml(src).map_err(|e| anyhow!("{path}: {e}"))?);
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
                let text = p.extension().and_then(|e| e.to_str()).map_or(false, |e| EXTS.contains(&e));
                if p.is_file() && text && !files.contains_key(&rel) {
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
        let mut spec = from_files(&read_dir(path)?)?;
        add_bare_files(&mut spec, path);
        resolve_art(&mut spec, path)?;
        Ok(spec)
    } else {
        let s = std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
        let mut spec = GameSpec::from_json(&s)?;
        resolve_art(&mut spec, path.parent().unwrap_or(Path::new(".")))?;
        Ok(spec)
    }
}

const MODEL_EXTS: [&str; 3] = ["glb", "gltf", "obj"];

/// `sprites/NAME.png` and `models/NAME.glb|gltf|obj` without a matching .txt/.toml become
/// sprite / model NAME (model files a models/*.toml already points at are skipped).
fn add_bare_files(spec: &mut GameSpec, dir: &Path) {
    let list = |sub: &str| -> Vec<(String, String)> {
        let mut v = Vec::new();
        if let Ok(rd) = std::fs::read_dir(dir.join(sub)) {
            for e in rd.flatten() {
                let p = e.path();
                if !p.is_file() {
                    continue;
                }
                let (Some(stem), Some(ext)) = (p.file_stem().and_then(|s| s.to_str()), p.extension().and_then(|s| s.to_str())) else { continue };
                v.push((stem.to_string(), ext.to_ascii_lowercase()));
            }
        }
        v.sort();
        v
    };
    for (stem, ext) in list("sprites") {
        if ext == "png" && !spec.sprites.contains_key(&stem) {
            let image = ImageRef::file(&format!("sprites/{stem}.{ext}"));
            spec.sprites.insert(stem, SpriteSpec { image: Some(image), ..Default::default() });
        }
    }
    let referenced: Vec<String> = spec.models.values().filter_map(|m| m.file.clone()).map(|f| f.replace('\\', "/")).collect();
    for (stem, ext) in list("models") {
        let rel = format!("models/{stem}.{ext}");
        if MODEL_EXTS.contains(&ext.as_str()) && !spec.models.contains_key(&stem) && !referenced.contains(&rel) {
            spec.models.insert(stem, ModelSpec { file: Some(rel), ..Default::default() });
        }
    }
}

/// Checks a path written inside a game file: relative, no `..`, no drive.
fn game_rel(dir: &Path, rel: &str) -> anyhow::Result<std::path::PathBuf> {
    let r = rel.replace('\\', "/");
    if r.starts_with('/') || r.contains(':') || r.split('/').any(|c| c == "..") {
        bail!("path {rel:?} must be relative to the game folder (no `..`, no drive or leading /)");
    }
    Ok(dir.join(r))
}

/// Decode every image sprite and import every mesh model the spec names, relative to `dir`.
/// Tiling sprites must name sprites that exist.
pub fn resolve_art(spec: &mut GameSpec, dir: &Path) -> anyhow::Result<()> {
    let mut cache: BTreeMap<String, Arc<Rgba>> = BTreeMap::new();
    let mut read = |rel: &str| -> anyhow::Result<Arc<Rgba>> {
        if let Some(img) = cache.get(rel) {
            return Ok(img.clone());
        }
        let p = game_rel(dir, rel)?;
        let bytes = std::fs::read(&p).with_context(|| format!("cannot read {}", p.display()))?;
        let img = Arc::new(Rgba::from_png(&bytes).with_context(|| format!("{} is not a readable PNG", p.display()))?);
        cache.insert(rel.to_string(), img.clone());
        Ok(img)
    };
    let names: Vec<String> = spec.sprites.keys().cloned().collect();
    for (name, s) in spec.sprites.iter_mut() {
        if let Some(image) = &s.image {
            let img = image.decode(&mut read).with_context(|| format!("sprite '{name}'"))?;
            if img.width == 0 || img.height == 0 {
                bail!("sprite '{name}' is empty");
            }
            s.loaded = Some(Arc::new(img));
        }
        if let Some(t) = &s.tiles {
            for part in std::iter::once(&t.fill).chain(t.top.iter()).chain(t.left.iter()).chain(t.right.iter()) {
                if !names.contains(part) || part == name {
                    bail!("sprite '{name}': tile sprite '{part}' does not exist");
                }
            }
            if t.size.is_nan() || t.size <= 0.0 {
                bail!("sprite '{name}': tiles.size must be above 0");
            }
        }
    }
    for (name, m) in spec.models.iter_mut() {
        if let Some(file) = &m.file {
            let p = game_rel(dir, file)?;
            let tex = m.texture.as_deref().map(|t| game_rel(dir, t)).transpose()?;
            let imported = crate::models::import(&p, &m.groups, tex.as_deref()).with_context(|| format!("model '{name}'"))?;
            m.imported = Some(Arc::new(imported));
        }
    }
    Ok(())
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

// ---------------------------------------------------------------- checks

/// Every sprite and model an entity names exists, and the script (legacy games) compiles.
pub fn check(spec: GameSpec) -> anyhow::Result<GameSpec> {
    if spec.entities.is_empty() {
        bail!("scene.toml has no entities");
    }
    for e in &spec.entities {
        if let Some(sp) = &e.sprite {
            if !spec.sprites.contains_key(sp) {
                bail!("entity '{}' uses sprite '{}' but there is no sprites/{}.txt|.toml|.png", e.name, sp, sp);
            }
        }
        if let Some(m) = &e.model {
            if !spec.models.contains_key(m) {
                bail!("entity '{}' uses model '{}' but there is no models/{}.toml", e.name, m, m);
            }
        }
    }
    for (name, s) in &spec.sprites {
        let (w, h) = s.size();
        if s.tiles.is_none() && (w == 0 || h == 0) {
            bail!("sprite '{name}' has no pixels");
        }
    }
    crate::script::ScriptHost::check(&spec.script).map_err(|e| anyhow!("script.rhai: {e}"))?;
    Ok(spec)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(files: &[(&str, &[u8])]) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!("droid_project_test_{}_{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
        let _ = std::fs::remove_dir_all(&dir);
        for (rel, data) in files {
            let p = dir.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, data).unwrap();
        }
        dir
    }

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbaImage::from_fn(w, h, |x, _| if x == 0 { image::Rgba([0, 0, 0, 10]) } else { image::Rgba([200, 30, 30, 255]) });
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn png_sprites_sheet_tiles_and_tiling_rules_load() {
        let sheet = png(8, 4);
        let dir = game(&[
            ("game.toml", b"title = \"t\"\n"),
            ("images/sheet.png", &sheet),
            ("sprites/whole.png", &sheet),
            ("sprites/a.toml", b"sheet = \"images/sheet.png\"\ntile = [1, 0]\ntile_size = [4, 4]\n"),
            ("sprites/tall.toml", b"stack = [{ file = \"images/sheet.png\" }, { file = \"images/sheet.png\" }]\n"),
            ("sprites/ground.toml", b"[tiles]\nfill = \"a\"\ntop = \"whole\"\nsize = 0.5\n"),
            ("sprites/hero.txt", b"x = 1 0 0\n---\nx.\n.x\n"),
        ]);
        let spec = load(&dir).unwrap();
        assert_eq!(spec.sprites["whole"].size(), (8, 4));
        assert_eq!(spec.sprites["a"].size(), (4, 4));
        assert_eq!(spec.sprites["tall"].size(), (8, 8));
        assert_eq!(spec.sprites["hero"].size(), (2, 2), "text sprites still work");
        // Alpha is cut out: the faint first column is fully transparent, the rest opaque.
        let px = &spec.sprites["whole"].loaded.as_ref().unwrap().pixels;
        assert_eq!(&px[0..4], &[0, 0, 0, 0]);
        assert_eq!(px[7], 255);
        assert_eq!(spec.sprites["ground"].tiles.as_ref().unwrap().size, 0.5);
        // Written back out, the image and tiling sprites stay .toml files.
        let files = to_files(&spec).unwrap();
        assert!(files.contains_key("sprites/a.toml") && files.contains_key("sprites/ground.toml") && files.contains_key("sprites/hero.txt"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bad_art_references_are_errors() {
        let sheet = png(8, 4);
        let cases: [(&str, &[u8], &str); 5] = [
            ("sprites/a.toml", b"sheet = \"images/sheet.png\"\ntile = [5, 0]\ntile_size = [4, 4]\n", "outside"),
            ("sprites/a.toml", b"file = \"images/nope.png\"\n", "cannot read"),
            ("sprites/a.toml", b"file = \"../images/sheet.png\"\n", "relative to the game folder"),
            ("sprites/a.toml", b"fil = \"images/sheet.png\"\n", "unknown field"),
            ("sprites/a.toml", b"[tiles]\nfill = \"missing\"\n", "does not exist"),
        ];
        for (path, text, want) in cases {
            let dir = game(&[("game.toml", b"title = \"t\"\n"), ("images/sheet.png", &sheet), (path, text)]);
            let err = format!("{:#}", load(&dir).unwrap_err());
            assert!(err.contains(want), "{want:?} not in {err:?}");
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn write_dir_keeps_binary_art() {
        let dir = game(&[("game.toml", b"title = \"t\"\n"), ("sprites/x.png", &png(2, 2)), ("models/m.glb", b"glTF")]);
        write_dir(&dir, &FileSet::from([("game.toml".to_string(), "title = \"t\"\n".to_string())])).unwrap();
        assert!(dir.join("sprites/x.png").exists() && dir.join("models/m.glb").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

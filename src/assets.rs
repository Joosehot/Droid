//! The asset library: real CC0 art downloaded from the internet (Kenney, Quaternius), indexed by
//! `assets/library.toml`, with every pack's origin in `assets/packs.toml`.
//!
//! The library answers "which art can depict a slime?" (sprites and models by lexicon kind,
//! tiles by theme), checks itself (every file exists and loads, frames of one asset share a
//! size, every required kind is covered, every file is restorable by `assets/fetch.sh`), and
//! installs chosen assets into a game folder so the game is self-contained.

use crate::spec::{ImageRef, Rgba, SpriteSpec, TileRule};
use anyhow::{anyhow, bail, Context};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// 2D kinds the library must cover (players, enemies, pickups, hazards, exits, projectiles, decor).
pub const REQUIRED_SPRITE_KINDS: &[&str] = &[
    "knight", "cat", "frog", "ship", "gnome", //
    "slime", "spider", "skeleton", "zombie", "blob", "bat", "ghost", "alien", //
    "coin", "gem", "star", "apple", "heart", "key", //
    "spike", "saw", "lava", //
    "door", "flag", "portal", //
    "shot", "bomb", //
    "tree", "bush", "rock", "cloud",
];
/// Level looks that must have ground, platform and wall tiles.
pub const REQUIRED_THEMES: &[&str] = &["forest", "cave", "snow", "desert", "castle"];
/// 3D kinds the library must cover.
pub const REQUIRED_MODEL_KINDS: &[&str] = &[
    "knight", "humanoid", "zombie", "skeleton", "slime", "ghost", "bat", "spider", //
    "coin", "gem", "star", "heart", "key", "tree", "rock", "door", "portal", "flag", "spike", "floor", "wall",
];
/// Kinds that need at least two different sprites so the art rule has a real choice.
pub const NEED_TWO_SPRITES: &[&str] = &["slime", "coin", "knight"];
/// Licences the library accepts.
pub const LICENCES: &[&str] = &["CC0-1.0", "CC-BY-3.0", "CC-BY-4.0"];

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct SpriteAsset {
    pub id: String,
    pub kinds: Vec<String>,
    pub pack: String,
    pub frames: Vec<ImageRef>,
    #[serde(default)]
    pub tile_size: Option<[u32; 2]>,
    #[serde(default)]
    pub spacing: u32,
    #[serde(default)]
    pub margin: u32,
    /// Animation speed; 0 = a still image.
    #[serde(default)]
    pub fps: f32,
    /// May be multiplied by a colour word.
    #[serde(default)]
    pub tint: bool,
    #[serde(default)]
    pub style: Vec<String>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Tileset {
    pub id: String,
    pub theme: Vec<String>,
    pub pack: String,
    #[serde(default)]
    pub style: Vec<String>,
    #[serde(default)]
    pub tile_size: Option<[u32; 2]>,
    #[serde(default)]
    pub spacing: u32,
    #[serde(default)]
    pub margin: u32,
    pub ground_top: ImageRef,
    pub ground_fill: ImageRef,
    #[serde(default)]
    pub platform_left: Option<ImageRef>,
    pub platform_mid: ImageRef,
    #[serde(default)]
    pub platform_right: Option<ImageRef>,
    pub wall: ImageRef,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ModelAsset {
    pub id: String,
    pub kinds: Vec<String>,
    pub pack: String,
    pub file: String,
    /// Node name -> animation group; empty = guessed from node names.
    #[serde(default)]
    pub groups: BTreeMap<String, String>,
    /// Colour texture overriding the one the file names (relative to the pack).
    #[serde(default)]
    pub texture: Option<String>,
    #[serde(default)]
    pub style: Vec<String>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Pack {
    pub id: String,
    pub title: String,
    pub author: String,
    pub licence: String,
    pub page: String,
    pub url: String,
    pub sha256: String,
    #[serde(default)]
    pub licence_file: Option<String>,
    pub files: Vec<String>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Index {
    #[serde(default)]
    sprite: Vec<SpriteAsset>,
    #[serde(default)]
    tileset: Vec<Tileset>,
    #[serde(default)]
    model: Vec<ModelAsset>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct PackIndex {
    #[serde(default)]
    pack: Vec<Pack>,
}

pub struct Library {
    /// The `assets/` folder.
    pub dir: PathBuf,
    pub sprites: Vec<SpriteAsset>,
    pub tilesets: Vec<Tileset>,
    pub models: Vec<ModelAsset>,
    pub packs: Vec<Pack>,
}

/// Fill the sheet geometry of an asset into a reference (and its stacked pieces).
fn with_geometry(r: &ImageRef, tile_size: Option<[u32; 2]>, spacing: u32, margin: u32) -> ImageRef {
    let mut out = r.clone();
    if out.sheet.is_some() {
        out.tile_size = out.tile_size.or(tile_size);
        if out.spacing == 0 {
            out.spacing = spacing;
        }
        if out.margin == 0 {
            out.margin = margin;
        }
    }
    out.stack = r.stack.iter().map(|s| with_geometry(s, tile_size, spacing, margin)).collect();
    out
}

impl SpriteAsset {
    /// The frames as complete image references (paths relative to the pack folder).
    pub fn frame_refs(&self) -> Vec<ImageRef> {
        self.frames.iter().map(|f| with_geometry(f, self.tile_size, self.spacing, self.margin)).collect()
    }
}

/// The tile slots of a tileset, in a fixed order.
pub const TILE_SLOTS: &[&str] = &["ground_top", "ground_fill", "platform_left", "platform_mid", "platform_right", "wall"];

impl Tileset {
    /// (slot name, complete image reference) for every slot that is set.
    pub fn slots(&self) -> Vec<(&'static str, ImageRef)> {
        let g = |r: &ImageRef| with_geometry(r, self.tile_size, self.spacing, self.margin);
        let mut v = vec![("ground_top", g(&self.ground_top)), ("ground_fill", g(&self.ground_fill))];
        if let Some(r) = &self.platform_left {
            v.push(("platform_left", g(r)));
        }
        v.push(("platform_mid", g(&self.platform_mid)));
        if let Some(r) = &self.platform_right {
            v.push(("platform_right", g(r)));
        }
        v.push(("wall", g(&self.wall)));
        v
    }
}

impl ModelAsset {
    /// Files the model needs, relative to the pack folder: the mesh, textures it names, the override.
    pub fn files(&self, pack_dir: &Path) -> anyhow::Result<Vec<String>> {
        let mut v = vec![self.file.clone()];
        let dir = Path::new(&self.file).parent().map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default();
        for img in referenced_images(&pack_dir.join(&self.file))? {
            v.push(if dir.is_empty() { img } else { format!("{dir}/{img}") });
        }
        v.extend(self.texture.iter().cloned());
        v.sort();
        v.dedup();
        Ok(v)
    }
}

/// External image files a glTF/GLB names (URIs, percent-decoded). OBJ: `map_Kd` of its MTL files.
pub fn referenced_images(path: &Path) -> anyhow::Result<Vec<String>> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let mut out = Vec::new();
    if ext == "obj" {
        let text = std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
        for lib in text.lines().filter_map(|l| l.trim().strip_prefix("mtllib ")) {
            out.push(lib.trim().to_string());
            let base = path.parent().unwrap_or(Path::new("."));
            let mtl = std::fs::read_to_string(base.join(lib.trim())).unwrap_or_default();
            out.extend(mtl.lines().filter_map(|l| l.trim().strip_prefix("map_Kd ")).map(|s| s.trim().to_string()));
        }
        return Ok(out);
    }
    let bytes = std::fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
    let gltf = gltf::Gltf::from_slice(&bytes).with_context(|| format!("{} is not glTF", path.display()))?;
    for img in gltf.document.images() {
        if let gltf::image::Source::Uri { uri, .. } = img.source() {
            if !uri.starts_with("data:") {
                out.push(percent_decode(uri));
            }
        }
    }
    for buf in gltf.document.buffers() {
        if let gltf::buffer::Source::Uri(uri) = buf.source() {
            if !uri.starts_with("data:") {
                out.push(percent_decode(uri));
            }
        }
    }
    Ok(out)
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// What installing assets into a game folder means: files to copy and text files to write.
/// Everything is relative to the game's data folder (`games/<slug>/game/`).
#[derive(Default, Debug, Clone, PartialEq)]
pub struct Install {
    /// (absolute source file, destination relative to the game folder)
    pub copies: Vec<(PathBuf, String)>,
    /// (path relative to the game folder, contents)
    pub texts: Vec<(String, String)>,
    /// Names the game can use: sprite names (frames in order) or the model name.
    pub names: Vec<String>,
}

impl Install {
    pub fn merge(&mut self, other: Install) {
        for c in other.copies {
            if !self.copies.contains(&c) {
                self.copies.push(c);
            }
        }
        for t in other.texts {
            match self.texts.iter_mut().find(|(p, _)| *p == t.0) {
                // The same file again (an asset installed twice) is fine; different contents
                // under one name would silently lose one of them, so the later one wins loudly.
                Some(existing) if existing.1 != t.1 => {
                    log::warn!("asset install: {} written twice with different contents; keeping the later one", t.0);
                    existing.1 = t.1;
                }
                Some(_) => {}
                None => self.texts.push(t),
            }
        }
        self.names.extend(other.names);
    }

    /// Copy the files and write the texts into `game_dir`.
    pub fn write_to(&self, game_dir: &Path) -> anyhow::Result<()> {
        for (src, dst) in &self.copies {
            let d = game_dir.join(dst);
            if let Some(p) = d.parent() {
                std::fs::create_dir_all(p)?;
            }
            std::fs::copy(src, &d).with_context(|| format!("cannot copy {} to {}", src.display(), d.display()))?;
        }
        for (rel, text) in &self.texts {
            let d = game_dir.join(rel);
            if let Some(p) = d.parent() {
                std::fs::create_dir_all(p)?;
            }
            std::fs::write(&d, text)?;
        }
        Ok(())
    }
}

/// Where a pack file goes inside a game: `images/<pack>/...` for pictures, `models/<pack>/...`
/// for meshes and their textures.
fn game_path(sub: &str, pack: &str, file: &str) -> String {
    format!("{sub}/{pack}/{file}")
}

fn check_name(name: &str) -> anyhow::Result<()> {
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        bail!("asset name {name:?} must be letters, digits, '_' or '-'");
    }
    Ok(())
}

impl Library {
    /// The library shipped with Droid (`<repo>/assets`).
    pub fn builtin() -> anyhow::Result<Library> {
        Library::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("assets"))
    }

    /// Read `library.toml` and `packs.toml` from `dir`. Parsing only; see [`Library::validate`].
    pub fn load(dir: &Path) -> anyhow::Result<Library> {
        let lib = std::fs::read_to_string(dir.join("library.toml")).with_context(|| format!("cannot read {}", dir.join("library.toml").display()))?;
        let index: Index = toml::from_str(&lib).map_err(|e| anyhow!("library.toml: {e}"))?;
        let packs = std::fs::read_to_string(dir.join("packs.toml")).with_context(|| format!("cannot read {}", dir.join("packs.toml").display()))?;
        let packs: PackIndex = toml::from_str(&packs).map_err(|e| anyhow!("packs.toml: {e}"))?;
        Ok(Library { dir: dir.to_path_buf(), sprites: index.sprite, tilesets: index.tileset, models: index.model, packs: packs.pack })
    }

    pub fn pack_dir(&self, pack: &str) -> PathBuf {
        self.dir.join("packs").join(pack)
    }

    pub fn pack(&self, id: &str) -> Option<&Pack> {
        self.packs.iter().find(|p| p.id == id)
    }
    pub fn sprite(&self, id: &str) -> Option<&SpriteAsset> {
        self.sprites.iter().find(|s| s.id == id)
    }
    pub fn tileset(&self, id: &str) -> Option<&Tileset> {
        self.tilesets.iter().find(|s| s.id == id)
    }
    pub fn model(&self, id: &str) -> Option<&ModelAsset> {
        self.models.iter().find(|s| s.id == id)
    }

    /// Sprites that can depict `kind`, in library order (a stable order for choices).
    pub fn sprites_for(&self, kind: &str) -> Vec<&SpriteAsset> {
        self.sprites.iter().filter(|s| s.kinds.iter().any(|k| k == kind)).collect()
    }
    /// Models that can depict `kind`, in library order.
    pub fn models_for(&self, kind: &str) -> Vec<&ModelAsset> {
        self.models.iter().filter(|s| s.kinds.iter().any(|k| k == kind)).collect()
    }
    /// Tilesets for a theme, in library order.
    pub fn tilesets_for(&self, theme: &str) -> Vec<&Tileset> {
        self.tilesets.iter().filter(|t| t.theme.iter().any(|k| k == theme)).collect()
    }

    /// Every sprite kind, sorted.
    pub fn sprite_kinds(&self) -> BTreeSet<&str> {
        self.sprites.iter().flat_map(|s| s.kinds.iter().map(|k| k.as_str())).collect()
    }
    /// Every model kind, sorted.
    pub fn model_kinds(&self) -> BTreeSet<&str> {
        self.models.iter().flat_map(|s| s.kinds.iter().map(|k| k.as_str())).collect()
    }
    /// Every tileset theme, sorted.
    pub fn themes(&self) -> BTreeSet<&str> {
        self.tilesets.iter().flat_map(|s| s.theme.iter().map(|k| k.as_str())).collect()
    }

    /// Decode one image reference of a pack.
    pub fn decode(&self, pack: &str, r: &ImageRef) -> anyhow::Result<Rgba> {
        let dir = self.pack_dir(pack);
        let mut read = |rel: &str| -> anyhow::Result<Arc<Rgba>> {
            let p = dir.join(rel);
            let bytes = std::fs::read(&p).with_context(|| format!("cannot read {}", p.display()))?;
            Ok(Arc::new(Rgba::from_png(&bytes).with_context(|| format!("{} is not a readable PNG", p.display()))?))
        };
        r.decode(&mut read)
    }

    /// Pixel size of a sprite's first frame (for choosing an entity's aspect ratio).
    pub fn sprite_size(&self, id: &str) -> anyhow::Result<(u32, u32)> {
        let s = self.sprite(id).ok_or_else(|| anyhow!("no sprite asset {id:?}"))?;
        let f = s.frame_refs().into_iter().next().ok_or_else(|| anyhow!("sprite asset {id:?} has no frames"))?;
        let img = self.decode(&s.pack, &f)?;
        Ok((img.width, img.height))
    }

    /// Everything wrong with the library, one line each (empty = fine).
    pub fn validate(&self) -> Vec<String> {
        let mut errs = Vec::new();
        let mut ids = BTreeSet::new();
        let mut used: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
        let all_ids = self.sprites.iter().map(|s| &s.id).chain(self.tilesets.iter().map(|t| &t.id)).chain(self.models.iter().map(|m| &m.id));
        for id in all_ids {
            if !ids.insert(id.clone()) {
                errs.push(format!("id {id:?} is used twice"));
            }
            if check_name(id).is_err() {
                errs.push(format!("id {id:?} must be letters, digits, '_' or '-'"));
            }
        }
        let mut pack_ids = BTreeSet::new();
        for p in &self.packs {
            if !pack_ids.insert(p.id.as_str()) {
                errs.push(format!("pack {:?} is listed twice in packs.toml", p.id));
            }
            if !LICENCES.contains(&p.licence.as_str()) {
                errs.push(format!("pack {:?}: licence {:?} is not one of {LICENCES:?}", p.id, p.licence));
            }
            if p.sha256.len() != 64 || !p.sha256.chars().all(|c| c.is_ascii_hexdigit()) {
                errs.push(format!("pack {:?}: sha256 must be 64 hex digits", p.id));
            }
            if !self.pack_dir(&p.id).join("LICENSE.txt").is_file() {
                errs.push(format!("pack {:?}: packs/{}/LICENSE.txt is missing (run assets/fetch.sh)", p.id, p.id));
            }
            for f in &p.files {
                if !self.pack_dir(&p.id).join(f).is_file() {
                    errs.push(format!("pack {:?}: file {f:?} is missing (run assets/fetch.sh)", p.id));
                }
            }
        }
        let need_pack = |pack: &str, who: &str, errs: &mut Vec<String>| -> bool {
            if self.pack(pack).is_none() {
                errs.push(format!("{who}: pack {pack:?} is not in packs.toml"));
                return false;
            }
            true
        };

        // Images: cache decoded sheets per file so a tilemap is read once.
        let mut cache: BTreeMap<PathBuf, Arc<Rgba>> = BTreeMap::new();
        let mut decode = |pack: &str, r: &ImageRef| -> anyhow::Result<Rgba> {
            let dir = self.pack_dir(pack);
            let mut read = |rel: &str| -> anyhow::Result<Arc<Rgba>> {
                let p = dir.join(rel);
                if let Some(img) = cache.get(&p) {
                    return Ok(img.clone());
                }
                let bytes = std::fs::read(&p).with_context(|| format!("cannot read {}", p.display()))?;
                let img = Arc::new(Rgba::from_png(&bytes).with_context(|| format!("{} is not a readable PNG", p.display()))?);
                cache.insert(p, img.clone());
                Ok(img)
            };
            r.decode(&mut read)
        };

        for s in &self.sprites {
            let who = format!("sprite {:?}", s.id);
            if s.kinds.is_empty() {
                errs.push(format!("{who}: no kinds"));
            }
            if s.frames.is_empty() {
                errs.push(format!("{who}: no frames"));
            }
            if s.fps < 0.0 || (s.frames.len() > 1 && s.fps <= 0.0) {
                errs.push(format!("{who}: several frames need fps > 0"));
            }
            if !need_pack(&s.pack, &who, &mut errs) {
                continue;
            }
            let mut size: Option<(u32, u32)> = None;
            for (i, f) in s.frame_refs().iter().enumerate() {
                for file in f.files() {
                    used.entry(s.pack.as_str()).or_default().insert(file);
                }
                match decode(&s.pack, f) {
                    Ok(img) => {
                        if !img.any_opaque() {
                            errs.push(format!("{who}: frame {} is fully transparent", i + 1));
                        }
                        let sz = (img.width, img.height);
                        if size.map_or(false, |s0| s0 != sz) {
                            errs.push(format!("{who}: frame {} is {}x{}, frame 1 is {}x{} (frames must share a size)", i + 1, sz.0, sz.1, size.unwrap().0, size.unwrap().1));
                        }
                        size.get_or_insert(sz);
                    }
                    Err(e) => errs.push(format!("{who}: frame {}: {e:#}", i + 1)),
                }
            }
        }
        for t in &self.tilesets {
            let who = format!("tileset {:?}", t.id);
            if t.theme.is_empty() {
                errs.push(format!("{who}: no theme"));
            }
            if !need_pack(&t.pack, &who, &mut errs) {
                continue;
            }
            let mut size: Option<(u32, u32)> = None;
            for (slot, r) in t.slots() {
                for file in r.files() {
                    used.entry(t.pack.as_str()).or_default().insert(file);
                }
                match decode(&t.pack, &r) {
                    Ok(img) => {
                        let sz = (img.width, img.height);
                        if size.map_or(false, |s0| s0 != sz) {
                            errs.push(format!("{who}: {slot} is {}x{}, tiles of a set must share a size", sz.0, sz.1));
                        }
                        size.get_or_insert(sz);
                        if !img.any_opaque() {
                            errs.push(format!("{who}: {slot} is fully transparent"));
                        }
                    }
                    Err(e) => errs.push(format!("{who}: {slot}: {e:#}")),
                }
            }
        }
        for m in &self.models {
            let who = format!("model {:?}", m.id);
            if m.kinds.is_empty() {
                errs.push(format!("{who}: no kinds"));
            }
            if !need_pack(&m.pack, &who, &mut errs) {
                continue;
            }
            let dir = self.pack_dir(&m.pack);
            used.entry(m.pack.as_str()).or_default().insert(m.file.clone());
            match m.files(&dir) {
                Ok(files) => {
                    used.entry(m.pack.as_str()).or_default().extend(files);
                }
                Err(e) => errs.push(format!("{who}: {e:#}")),
            }
            for (node, group) in &m.groups {
                if !["body", "head", "arm_l", "arm_r", "leg_l", "leg_r", "tail"].contains(&group.as_str()) {
                    errs.push(format!("{who}: node {node:?} maps to unknown group {group:?}"));
                }
            }
            let tex = m.texture.as_ref().map(|t| dir.join(t));
            match crate::models::import(&dir.join(&m.file), &m.groups, tex.as_deref()) {
                Ok(imp) => {
                    if imp.size.max_element() <= 0.0 {
                        errs.push(format!("{who}: the model has no extent"));
                    }
                    if !m.groups.is_empty() {
                        let names: BTreeSet<&str> = imp.groups.iter().map(|g| g.name.as_str()).collect();
                        for g in m.groups.values() {
                            if !names.contains(g.as_str()) {
                                errs.push(format!("{who}: group {g:?} got no triangles (node names wrong?)"));
                            }
                        }
                    }
                }
                Err(e) => errs.push(format!("{who}: {e:#}")),
            }
        }
        // Every file an asset uses must be one fetch.sh restores.
        for (pack, files) in &used {
            if let Some(p) = self.pack(pack) {
                for f in files {
                    if !p.files.contains(f) {
                        errs.push(format!("pack {pack:?}: {f:?} is used by the library but not listed in packs.toml files"));
                    }
                }
            }
        }
        for p in &self.packs {
            if !used.contains_key(p.id.as_str()) {
                errs.push(format!("pack {:?} is not used by any asset", p.id));
            }
        }

        // Coverage.
        for k in REQUIRED_SPRITE_KINDS {
            if self.sprites_for(k).is_empty() {
                errs.push(format!("no sprite depicts required kind {k:?}"));
            }
        }
        for k in NEED_TWO_SPRITES {
            if self.sprites_for(k).len() < 2 {
                errs.push(format!("kind {k:?} needs at least two sprites (the art rule chooses between them)"));
            }
        }
        for t in REQUIRED_THEMES {
            if self.tilesets_for(t).is_empty() {
                errs.push(format!("no tileset for required theme {t:?}"));
            }
        }
        for k in REQUIRED_MODEL_KINDS {
            if self.models_for(k).is_empty() {
                errs.push(format!("no model depicts required kind {k:?}"));
            }
        }
        errs
    }

    // ------------------------------------------------------------ installing into a game

    /// Sprite asset `id` as game sprites. One frame: sprite `name`. Several frames: `name_1`,
    /// `name_2`, ... plus `name` (= the first frame). `names` lists the frames in order.
    pub fn install_sprite(&self, id: &str, name: &str) -> anyhow::Result<Install> {
        check_name(name)?;
        let s = self.sprite(id).ok_or_else(|| anyhow!("no sprite asset {id:?}"))?;
        let mut inst = Install::default();
        let frames = s.frame_refs();
        let frame_names: Vec<String> = if frames.len() == 1 { vec![name.to_string()] } else { (1..=frames.len()).map(|i| format!("{name}_{i}")).collect() };
        for (f, fname) in frames.iter().zip(&frame_names) {
            inst.merge(self.install_image(&s.pack, f, fname)?);
        }
        if frames.len() > 1 {
            inst.merge(self.install_image(&s.pack, &frames[0], name)?);
            inst.names.pop();
        }
        Ok(inst)
    }

    fn install_image(&self, pack: &str, r: &ImageRef, name: &str) -> anyhow::Result<Install> {
        let mut inst = Install::default();
        for f in r.files() {
            let src = self.pack_dir(pack).join(&f);
            if !src.is_file() {
                bail!("{} is missing (run assets/fetch.sh)", src.display());
            }
            inst.copies.push((src, game_path("images", pack, &f)));
        }
        let mapped = r.map_paths(&|p| game_path("images", pack, p));
        let spec = SpriteSpec { image: Some(mapped), ..Default::default() };
        inst.texts.push((format!("sprites/{name}.toml"), crate::project::sprite_to_toml(&spec)?));
        inst.names.push(name.to_string());
        Ok(inst)
    }

    /// Tileset `id` as game sprites: the pieces (`{prefix}_tile_ground_top`, ...) and three tiling
    /// sprites to put on box entities: `{prefix}_ground` (top row + fill), `{prefix}_platform`
    /// (caps + middle) and `{prefix}_wall`. `tile` is the world size of one tile.
    /// `names` = [ground, platform, wall].
    pub fn install_tileset(&self, id: &str, prefix: &str, tile: f32) -> anyhow::Result<Install> {
        check_name(prefix)?;
        if tile.is_nan() || tile <= 0.0 {
            bail!("tile size must be above 0");
        }
        let t = self.tileset(id).ok_or_else(|| anyhow!("no tileset {id:?}"))?;
        let mut inst = Install::default();
        for (slot, r) in t.slots() {
            inst.merge(self.install_image(&t.pack, &r, &format!("{prefix}_tile_{slot}"))?);
        }
        inst.names.clear();
        let piece = |slot: &str| format!("{prefix}_tile_{slot}");
        let has = |slot: &str| t.slots().iter().any(|(s, _)| *s == slot);
        let rules = [
            ("ground", TileRule { fill: piece("ground_fill"), top: Some(piece("ground_top")), left: None, right: None, size: tile }),
            (
                "platform",
                TileRule {
                    fill: piece("platform_mid"),
                    top: Some(piece("platform_mid")),
                    left: has("platform_left").then(|| piece("platform_left")),
                    right: has("platform_right").then(|| piece("platform_right")),
                    size: tile,
                },
            ),
            ("wall", TileRule { fill: piece("wall"), top: None, left: None, right: None, size: tile }),
        ];
        for (what, rule) in rules {
            let spec = SpriteSpec { tiles: Some(rule), ..Default::default() };
            inst.texts.push((format!("sprites/{prefix}_{what}.toml"), crate::project::sprite_to_toml(&spec)?));
            inst.names.push(format!("{prefix}_{what}"));
        }
        Ok(inst)
    }

    /// Model asset `id` as game model `name` (`models/{name}.toml` + the mesh and its textures
    /// under `models/<pack>/`). `names` = [name].
    pub fn install_model(&self, id: &str, name: &str) -> anyhow::Result<Install> {
        check_name(name)?;
        let m = self.model(id).ok_or_else(|| anyhow!("no model asset {id:?}"))?;
        let dir = self.pack_dir(&m.pack);
        let mut inst = Install::default();
        for f in m.files(&dir)? {
            let src = dir.join(&f);
            if !src.is_file() {
                bail!("{} is missing (run assets/fetch.sh)", src.display());
            }
            inst.copies.push((src, game_path("models", &m.pack, &f)));
        }
        let spec = crate::spec::ModelSpec {
            file: Some(game_path("models", &m.pack, &m.file)),
            groups: m.groups.clone(),
            texture: m.texture.as_ref().map(|t| game_path("models", &m.pack, t)),
            ..Default::default()
        };
        let mut table = toml::Table::new();
        if let Some(f) = &spec.file {
            table.insert("file".into(), f.clone().into());
        }
        if !spec.groups.is_empty() {
            table.insert("groups".into(), toml::Value::try_from(&spec.groups)?);
        }
        if let Some(t) = &spec.texture {
            table.insert("texture".into(), t.clone().into());
        }
        inst.texts.push((format!("models/{name}.toml"), toml::to_string(&table)?));
        inst.names.push(name.to_string());
        Ok(inst)
    }

    /// A readable listing: kinds -> asset ids, themes -> tilesets, packs.
    pub fn listing(&self) -> String {
        let mut out = String::new();
        let line = |out: &mut String, k: &str, ids: Vec<&str>| out.push_str(&format!("  {k:<10} {}\n", ids.join(" ")));
        out.push_str(&format!("2D sprites ({}), by kind:\n", self.sprites.len()));
        for k in self.sprite_kinds() {
            line(&mut out, k, self.sprites_for(k).iter().map(|s| s.id.as_str()).collect());
        }
        out.push_str(&format!("\ntiles ({} sets), by theme:\n", self.tilesets.len()));
        for t in self.themes() {
            line(&mut out, t, self.tilesets_for(t).iter().map(|s| s.id.as_str()).collect());
        }
        out.push_str(&format!("\n3D models ({}), by kind:\n", self.models.len()));
        for k in self.model_kinds() {
            line(&mut out, k, self.models_for(k).iter().map(|s| s.id.as_str()).collect());
        }
        let files: usize = self.packs.iter().map(|p| p.files.len()).sum();
        out.push_str(&format!("\npacks ({}, {files} files):\n", self.packs.len()));
        for p in &self.packs {
            out.push_str(&format!("  {:<34} {} - {} ({})\n", p.id, p.title, p.author, p.licence));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_library_is_valid_and_covers_every_required_kind() {
        let lib = Library::builtin().unwrap();
        let errs = lib.validate();
        assert!(errs.is_empty(), "asset library problems:\n{}", errs.join("\n"));
    }

    #[test]
    fn lookups_by_kind_and_theme() {
        let lib = Library::builtin().unwrap();
        assert!(lib.sprites_for("slime").len() >= 2);
        assert!(lib.sprites_for("coin").len() >= 2);
        assert!(lib.sprites_for("knight").len() >= 2);
        assert!(lib.sprites_for("no-such-kind").is_empty());
        for t in REQUIRED_THEMES {
            assert!(!lib.tilesets_for(t).is_empty(), "{t}");
        }
        assert!(!lib.models_for("zombie").is_empty());
        let (w, h) = lib.sprite_size("toon_zombie").unwrap();
        assert!(h > w, "the zombie is taller than wide");
    }

    #[test]
    fn every_listed_file_exists_and_every_image_decodes() {
        let lib = Library::builtin().unwrap();
        for p in &lib.packs {
            for f in &p.files {
                let path = lib.pack_dir(&p.id).join(f);
                assert!(path.is_file(), "{}", path.display());
                if f.ends_with(".png") {
                    Rgba::from_png(&std::fs::read(&path).unwrap()).unwrap();
                }
            }
        }
    }

    #[test]
    fn missing_coverage_and_bad_files_are_reported() {
        let mut lib = Library::builtin().unwrap();
        lib.sprites.retain(|s| !s.kinds.contains(&"bat".to_string()));
        lib.sprites[0].frames = vec![ImageRef::file("no/such/file.png")];
        let errs = lib.validate().join("\n");
        assert!(errs.contains("required kind \"bat\""), "{errs}");
        assert!(errs.contains("no/such/file.png"), "{errs}");
    }

    #[test]
    fn installed_assets_load_as_a_game() {
        let lib = Library::builtin().unwrap();
        let dir = std::env::temp_dir().join(format!("droid_assets_install_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut inst = lib.install_sprite("np_slime_purple", "slime").unwrap();
        assert_eq!(inst.names, vec!["slime_1", "slime_2"]);
        inst.merge(lib.install_sprite("np_door", "door").unwrap());
        inst.merge(lib.install_sprite("pp_coin", "coin").unwrap());
        let tiles = lib.install_tileset("np_forest", "forest", 1.0).unwrap();
        assert_eq!(tiles.names, vec!["forest_ground", "forest_platform", "forest_wall"]);
        inst.merge(tiles);
        inst.merge(lib.install_model("graveyard_zombie", "zombie").unwrap());
        inst.write_to(&dir).unwrap();
        std::fs::write(dir.join("game.toml"), "title = \"t\"\n").unwrap();
        std::fs::write(
            dir.join("scene.toml"),
            "[[entities]]\nname = \"ground\"\nsprite = \"forest_ground\"\nscale = [8.0, 2.0, 1.0]\n\n[[entities]]\nname = \"z\"\nmodel = \"zombie\"\n",
        )
        .unwrap();
        let spec = crate::project::load(&dir).unwrap();
        for n in ["slime", "slime_1", "slime_2", "door", "coin_1", "forest_tile_ground_top", "forest_tile_platform_mid", "forest_tile_wall"] {
            let s = spec.sprites.get(n).unwrap_or_else(|| panic!("sprite {n}"));
            assert!(s.loaded.is_some(), "{n} decoded");
        }
        assert_eq!(spec.sprites["slime_1"].size(), (64, 64));
        assert_eq!(spec.sprites["door"].size(), (64, 128), "door is two tiles stacked");
        assert_eq!(spec.sprites["coin_1"].size(), (18, 18));
        for n in ["forest_ground", "forest_platform", "forest_wall"] {
            assert!(spec.sprites[n].tiles.is_some(), "{n} is a tiling sprite");
        }
        let z = spec.models["zombie"].imported.as_ref().unwrap();
        let groups: BTreeSet<&str> = z.groups.iter().map(|g| g.name.as_str()).collect();
        for g in ["body", "head", "arm_l", "arm_r", "leg_l", "leg_r"] {
            assert!(groups.contains(g), "zombie group {g}: {groups:?}");
        }
        // A world from it: the tiled ground stays a box.
        let w = crate::world::World::from_spec(&spec);
        let e = w.entity("ground").unwrap();
        assert_eq!(w.ecs.get::<&crate::world::Visual>(e).unwrap().shape, crate::spec::Shape::Box);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

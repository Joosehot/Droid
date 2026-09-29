//! GameSpec: the declarative game description the AI produces.
//! A game = one JSON file (`game.json`) containing scene entities, camera,
//! UI texts and a Rhai script for behaviour.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GameSpec {
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default)]
    pub view: View,
    #[serde(default)]
    pub camera: CameraSpec,
    #[serde(default = "default_bg")]
    pub background: [f32; 3],
    #[serde(default = "default_gravity")]
    pub gravity: [f32; 3],
    /// Pixel-art sprites drawn by the AI, keyed by name.
    #[serde(default)]
    pub sprites: BTreeMap<String, SpriteSpec>,
    /// Procedural 3D models (blended primitive parts), keyed by name.
    #[serde(default)]
    pub models: BTreeMap<String, ModelSpec>,
    #[serde(default)]
    pub entities: Vec<EntitySpec>,
    #[serde(default)]
    pub texts: Vec<TextSpec>,
    #[serde(default)]
    pub script: String,
}

/// A sprite. Three kinds, one struct:
/// - text pixel art: `palette` maps a single character to an RGB colour, `pixels` is one
///   string per row (`.` or space = transparent);
/// - an image: `image` names a PNG file or a tile cut from a sheet (decoded into `loaded`
///   when the game is loaded from disk);
/// - a tiling rule: `tiles` names other sprites that are repeated across a box entity
///   (ground, platforms, walls).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct SpriteSpec {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub palette: BTreeMap<String, [f32; 3]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pixels: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<ImageRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tiles: Option<TileRule>,
    /// Decoded pixels of `image` (not serialised; filled by `project::load`).
    #[serde(skip)]
    pub loaded: Option<std::sync::Arc<Rgba>>,
}

/// Decoded RGBA8 pixels, row-major, top row first. Alpha is cut to 0 or 255.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

/// Where an image sprite's pixels come from. Paths are relative to the game folder
/// (or, in the asset library, to the pack folder).
///
/// ```toml
/// file = "images/coin_gold.png"                          # a whole PNG
/// sheet = "images/tilemap.png"                           # or one tile of a sheet
/// tile = [11, 7]
/// tile_size = [18, 18]
/// spacing = 0                                            # pixels between tiles
/// margin = 0                                             # pixels around the sheet
/// stack = [{ file = "top.png" }, { file = "bottom.png" }]  # or pieces stacked top to bottom
/// ```
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct ImageRef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sheet: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile: Option<[u32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile_size: Option<[u32; 2]>,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub spacing: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub margin: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stack: Vec<ImageRef>,
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

impl ImageRef {
    pub fn file(path: &str) -> ImageRef {
        ImageRef { file: Some(path.to_string()), ..Default::default() }
    }

    pub fn tile(sheet: &str, tile: [u32; 2], tile_size: [u32; 2]) -> ImageRef {
        ImageRef { sheet: Some(sheet.to_string()), tile: Some(tile), tile_size: Some(tile_size), ..Default::default() }
    }

    /// Every image file this reference reads (relative paths as written).
    pub fn files(&self) -> Vec<String> {
        let mut out: Vec<String> = self.file.iter().chain(self.sheet.iter()).cloned().collect();
        for s in &self.stack {
            out.extend(s.files());
        }
        out
    }

    /// The same reference with every path passed through `f` (used when files are copied).
    pub fn map_paths(&self, f: &dyn Fn(&str) -> String) -> ImageRef {
        ImageRef {
            file: self.file.as_deref().map(f),
            sheet: self.sheet.as_deref().map(f),
            stack: self.stack.iter().map(|s| s.map_paths(f)).collect(),
            ..self.clone()
        }
    }

    /// Decode the pixels. `read` loads a whole PNG by relative path (so callers can cache).
    pub fn decode(&self, read: &mut dyn FnMut(&str) -> anyhow::Result<std::sync::Arc<Rgba>>) -> anyhow::Result<Rgba> {
        use anyhow::{anyhow, bail};
        if !self.stack.is_empty() {
            let parts: Vec<Rgba> = self.stack.iter().map(|s| s.decode(read)).collect::<anyhow::Result<_>>()?;
            let w = parts.iter().map(|p| p.width).max().unwrap_or(0);
            let h: u32 = parts.iter().map(|p| p.height).sum();
            let mut out = Rgba { width: w, height: h, pixels: vec![0; (w * h * 4) as usize] };
            let mut y0 = 0;
            for p in &parts {
                let x0 = (w - p.width) / 2;
                out.blit(p, 0, 0, p.width, p.height, x0, y0);
                y0 += p.height;
            }
            return Ok(out);
        }
        match (&self.file, &self.sheet) {
            (Some(f), None) => Ok((*read(f)?).clone()),
            (None, Some(s)) => {
                let img = read(s)?;
                let tile = self.tile.ok_or_else(|| anyhow!("sheet {s:?} needs `tile = [col, row]`"))?;
                let size = self.tile_size.ok_or_else(|| anyhow!("sheet {s:?} needs `tile_size = [w, h]`"))?;
                if size[0] == 0 || size[1] == 0 {
                    bail!("sheet {s:?}: tile_size must not be zero");
                }
                let x = self.margin + tile[0] * (size[0] + self.spacing);
                let y = self.margin + tile[1] * (size[1] + self.spacing);
                if x + size[0] > img.width || y + size[1] > img.height {
                    bail!("sheet {s:?} is {}x{}; tile {:?} of size {:?} is outside it", img.width, img.height, tile, size);
                }
                let mut out = Rgba { width: size[0], height: size[1], pixels: vec![0; (size[0] * size[1] * 4) as usize] };
                out.blit(&img, x, y, size[0], size[1], 0, 0);
                Ok(out)
            }
            (Some(_), Some(_)) => bail!("an image has either `file` or `sheet`, not both"),
            (None, None) => bail!("an image needs `file`, `sheet` or `stack`"),
        }
    }
}

impl Rgba {
    /// Decode PNG bytes. Alpha is cut out (below half = fully transparent, else opaque) so
    /// nearest-filtered sprites have crisp edges and no dark fringes.
    pub fn from_png(bytes: &[u8]) -> anyhow::Result<Rgba> {
        let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)?.to_rgba8();
        let (width, height) = img.dimensions();
        let mut pixels = img.into_raw();
        for px in pixels.chunks_exact_mut(4) {
            if px[3] < 128 {
                px.copy_from_slice(&[0, 0, 0, 0]);
            } else {
                px[3] = 255;
            }
        }
        Ok(Rgba { width, height, pixels })
    }

    /// Copy a `w`x`h` block from `src` at (sx, sy) to (dx, dy).
    pub fn blit(&mut self, src: &Rgba, sx: u32, sy: u32, w: u32, h: u32, dx: u32, dy: u32) {
        for row in 0..h {
            if sy + row >= src.height || dy + row >= self.height {
                break;
            }
            let n = w.min(src.width.saturating_sub(sx)).min(self.width.saturating_sub(dx)) as usize * 4;
            let s = (((sy + row) * src.width + sx) * 4) as usize;
            let d = (((dy + row) * self.width + dx) * 4) as usize;
            self.pixels[d..d + n].copy_from_slice(&src.pixels[s..s + n]);
        }
    }

    /// True if at least one pixel is visible.
    pub fn any_opaque(&self) -> bool {
        self.pixels.chunks_exact(4).any(|p| p[3] > 0)
    }
}

/// How a tiling sprite covers a box: `fill` repeats over the whole box, `top` (if given)
/// replaces the top row, `left`/`right` cap the ends of that top row. `size` is the world size
/// of one tile. Names refer to other sprites of the game.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TileRule {
    pub fill: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right: Option<String>,
    #[serde(default = "default_tile_world")]
    pub size: f32,
}

fn default_tile_world() -> f32 {
    1.0
}

impl SpriteSpec {
    /// Width (max row length) and height in pixels.
    pub fn size(&self) -> (u32, u32) {
        if let Some(img) = &self.loaded {
            return (img.width, img.height);
        }
        let w = self.pixels.iter().map(|r| r.chars().count()).max().unwrap_or(0) as u32;
        (w, self.pixels.len() as u32)
    }

    /// RGBA8 pixels, row-major, top row first.
    pub fn rgba(&self) -> Vec<u8> {
        if let Some(img) = &self.loaded {
            return img.pixels.clone();
        }
        let (w, h) = self.size();
        let mut out = vec![0u8; (w * h * 4) as usize];
        for (y, row) in self.pixels.iter().enumerate() {
            for (x, c) in row.chars().enumerate() {
                if c == '.' || c == ' ' {
                    continue;
                }
                let Some(rgb) = self.palette.get(&c.to_string()) else { continue };
                let i = ((y as u32 * w + x as u32) * 4) as usize;
                out[i] = (rgb[0].clamp(0.0, 1.0) * 255.0) as u8;
                out[i + 1] = (rgb[1].clamp(0.0, 1.0) * 255.0) as u8;
                out[i + 2] = (rgb[2].clamp(0.0, 1.0) * 255.0) as u8;
                out[i + 3] = 255;
            }
        }
        out
    }
}

/// A 3D model: either primitive parts that are smoothly blended ("morphed") together, or a
/// mesh file (`file = "models/knight.glb"`, glTF/GLB or OBJ, relative to the game folder).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ModelSpec {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<ModelPart>,
    /// Grid resolution along the longest axis (8..96). Higher = smoother, slower to build.
    #[serde(default = "default_resolution")]
    pub resolution: u32,
    /// Mesh file to import instead of `parts`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// Node name -> animation group ("arm-left" = "arm_l"). Empty = guessed from the names.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub groups: BTreeMap<String, String>,
    /// Colour texture to bake into vertex colours, overriding the one the file names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texture: Option<String>,
    /// The imported mesh (not serialised; filled by `project::load`).
    #[serde(skip)]
    pub imported: Option<std::sync::Arc<crate::models::Imported>>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum PartShape {
    #[default]
    Sphere,
    Box,
    Capsule,
    Cylinder,
    Cone,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModelPart {
    #[serde(default)]
    pub shape: PartShape,
    #[serde(default)]
    pub position: [f32; 3],
    /// Full extents (width, height, depth) in model units.
    #[serde(default = "default_scale")]
    pub size: [f32; 3],
    #[serde(default, skip_serializing_if = "is_zero3")]
    pub rotation: [f32; 3],
    /// Smooth-blend radius with the other parts. 0 = hard union.
    #[serde(default = "default_blend")]
    pub blend: f32,
    #[serde(default = "default_color")]
    pub color: [f32; 3],
    /// true = carve this shape out of the model instead of adding it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub subtract: bool,
    /// Animation group ("body", "head", "arm_l", "arm_r", "leg_l", "leg_r", "tail", ...).
    /// Parts in the same group are blended together and move as one rigid piece.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Pivot point (model units) the group rotates around. Default: top of legs/arms, bottom of head, centre otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pivot: Option<[f32; 3]>,
}

fn default_resolution() -> u32 { 32 }
fn default_blend() -> f32 { 0.2 }

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum View {
    #[serde(rename = "2d")]
    #[default]
    TwoD,
    #[serde(rename = "3d")]
    ThreeD,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CameraSpec {
    /// 3D: eye position. 2D: x/y is the view centre.
    #[serde(default = "default_cam_pos")]
    pub position: [f32; 3],
    /// 3D only: point the camera looks at.
    #[serde(default)]
    pub look_at: [f32; 3],
    /// 2D only: how many world units tall the view is.
    #[serde(default = "default_zoom")]
    pub zoom: f32,
    /// Name of an entity the camera keeps centred / follows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follow: Option<String>,
    /// 2D follow only: the view centre is the followed entity plus this (e.g. look ahead).
    #[serde(default, skip_serializing_if = "is_zero3")]
    pub offset: [f32; 3],
}

impl Default for CameraSpec {
    fn default() -> Self {
        Self { position: default_cam_pos(), look_at: [0.0; 3], zoom: default_zoom(), follow: None, offset: [0.0; 3] }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Shape {
    #[default]
    Box,
    Sphere,
    /// Flat quad in the XY plane (2D sprite-like rectangle).
    Plane,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum BodyKind {
    /// No physics, no collisions.
    #[default]
    None,
    /// Immovable, collides.
    Static,
    /// Moved by script only (no gravity), collides and pushes dynamic bodies.
    Kinematic,
    /// Gravity + velocity + collision response.
    Dynamic,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PhysicsSpec {
    #[serde(default)]
    pub body: BodyKind,
    #[serde(default, skip_serializing_if = "is_zero3")]
    pub velocity: [f32; 3],
    /// 0 = no bounce, 1 = perfectly elastic.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub bounciness: f32,
    /// false = trigger: reports collisions but does not block movement.
    #[serde(default = "default_true", skip_serializing_if = "std::clone::Clone::clone")]
    pub solid: bool,
}

impl Default for PhysicsSpec {
    fn default() -> Self {
        Self { body: BodyKind::None, velocity: [0.0; 3], bounciness: 0.0, solid: true }
    }
}

impl PhysicsSpec {
    fn is_default(&self) -> bool {
        self.body == BodyKind::None && self.velocity == [0.0; 3] && self.bounciness == 0.0 && self.solid
    }
}

fn is_zero3(v: &[f32; 3]) -> bool { *v == [0.0; 3] }
fn is_zero(v: &f32) -> bool { *v == 0.0 }

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EntitySpec {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default)]
    pub shape: Shape,
    /// Name of a sprite from `sprites`; the entity is then drawn as a textured quad.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite: Option<String>,
    /// Name of a model from `models`; the entity is then drawn as that 3D mesh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default)]
    pub position: [f32; 3],
    /// Euler rotation in degrees (x, y, z).
    #[serde(default, skip_serializing_if = "is_zero3")]
    pub rotation: [f32; 3],
    #[serde(default = "default_scale")]
    pub scale: [f32; 3],
    #[serde(default = "default_color")]
    pub color: [f32; 3],
    #[serde(default, skip_serializing_if = "PhysicsSpec::is_default")]
    pub physics: PhysicsSpec,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TextSpec {
    pub id: String,
    #[serde(default)]
    pub text: String,
    /// Screen position, 0..1 from top-left.
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    /// Height of a glyph in pixels.
    #[serde(default = "default_text_size")]
    pub size: f32,
    #[serde(default = "default_color")]
    pub color: [f32; 3],
}

fn default_bg() -> [f32; 3] { [0.08, 0.09, 0.12] }
fn default_gravity() -> [f32; 3] { [0.0, -20.0, 0.0] }
fn default_cam_pos() -> [f32; 3] { [0.0, 0.0, 20.0] }
fn default_zoom() -> f32 { 20.0 }
fn default_scale() -> [f32; 3] { [1.0; 3] }
fn default_color() -> [f32; 3] { [1.0; 3] }
fn default_text_size() -> f32 { 24.0 }
fn default_true() -> bool { true }

impl GameSpec {
    pub fn from_json(s: &str) -> anyhow::Result<GameSpec> {
        Ok(serde_json::from_str(s)?)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("spec serialises")
    }
}

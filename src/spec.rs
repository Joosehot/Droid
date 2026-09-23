//! GameSpec: the declarative game description the AI produces.
//! A game = one JSON file (`game.json`) containing scene entities, camera,
//! UI texts and a Rhai script for behaviour.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GameSpec {
    pub title: String,
    #[serde(default)]
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
    #[serde(default)]
    pub entities: Vec<EntitySpec>,
    #[serde(default)]
    pub texts: Vec<TextSpec>,
    #[serde(default)]
    pub script: String,
}

/// A pixel-art sprite: `palette` maps a single character to an RGB colour,
/// `pixels` is one string per row. `.` or space = transparent.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct SpriteSpec {
    #[serde(default)]
    pub palette: BTreeMap<String, [f32; 3]>,
    #[serde(default)]
    pub pixels: Vec<String>,
}

impl SpriteSpec {
    /// Width (max row length) and height in pixels.
    pub fn size(&self) -> (u32, u32) {
        let w = self.pixels.iter().map(|r| r.chars().count()).max().unwrap_or(0) as u32;
        (w, self.pixels.len() as u32)
    }

    /// RGBA8 pixels, row-major, top row first.
    pub fn rgba(&self) -> Vec<u8> {
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
    #[serde(default)]
    pub follow: Option<String>,
}

impl Default for CameraSpec {
    fn default() -> Self {
        Self { position: default_cam_pos(), look_at: [0.0; 3], zoom: default_zoom(), follow: None }
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
    #[serde(default)]
    pub velocity: [f32; 3],
    /// 0 = no bounce, 1 = perfectly elastic.
    #[serde(default)]
    pub bounciness: f32,
    /// false = trigger: reports collisions but does not block movement.
    #[serde(default = "default_true")]
    pub solid: bool,
}

impl Default for PhysicsSpec {
    fn default() -> Self {
        Self { body: BodyKind::None, velocity: [0.0; 3], bounciness: 0.0, solid: true }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EntitySpec {
    pub name: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub shape: Shape,
    /// Name of a sprite from `sprites`; the entity is then drawn as a textured quad.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite: Option<String>,
    #[serde(default)]
    pub position: [f32; 3],
    /// Euler rotation in degrees (x, y, z).
    #[serde(default)]
    pub rotation: [f32; 3],
    #[serde(default = "default_scale")]
    pub scale: [f32; 3],
    #[serde(default = "default_color")]
    pub color: [f32; 3],
    #[serde(default)]
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

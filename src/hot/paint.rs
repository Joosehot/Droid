//! The build side of the `art` rule: which library asset depicts each kind in this game,
//! installed into the game's folder under a plain name ("hero", "slime", "coin", "tiles_ground").
//! Choice order (deterministic): the chosen style, then a colour the sentence asked for (an
//! asset made in that colour, else one that may be tinted), then animated over still, then
//! library order. A kind with no asset in the style falls back to any style (noted in why.txt).

use super::config::Config;
use super::design::{Design, Why};
use super::request::*;
use crate::assets::{Install, Library, SpriteAsset};
use std::collections::BTreeMap;

/// One asset installed into the game.
#[derive(Clone, Debug)]
pub struct Pick {
    pub asset: String,
    /// Sprite or model name the scene and behaviour use.
    pub name: String,
    /// Animation frames (sprite names, in order); empty for a still sprite or a model.
    pub frames: Vec<String>,
    pub fps: f32,
    /// Width / height of the art (sprite pixels or model extents).
    pub aspect: f32,
    /// 3D: depth / height of the model.
    pub depth_ratio: f32,
    pub model: bool,
    /// Entity colour: a tint for a colour word, else white.
    pub color: [f32; 3],
}

#[derive(Clone, Debug, Default)]
pub struct ArtPlan {
    /// role ("player", "enemy:slime", "pickup:coin", "hazard:spike", "exit", "shot", "bomb",
    /// "decor:tree", "cloud") -> pick
    pub picks: BTreeMap<String, Pick>,
    /// Tiling sprite names (2D): ground, platform, wall.
    pub tiles: Option<Tiles>,
    pub install: Install,
    pub why: Vec<Why>,
}

#[derive(Clone, Debug)]
pub struct Tiles {
    pub ground: String,
    pub platform: String,
    pub wall: String,
}

impl ArtPlan {
    pub fn get(&self, role: &str) -> &Pick {
        self.picks.get(role).unwrap_or_else(|| panic!("art for {role} was not picked"))
    }
    pub fn has(&self, role: &str) -> bool {
        self.picks.contains_key(role)
    }
}

fn color_words(c: Color) -> &'static [&'static str] {
    match c {
        Color::Yellow => &["yellow", "gold"],
        Color::Grey => &["grey", "silver"],
        Color::Red => &["red", "fire"],
        Color::Purple => &["purple"],
        Color::Green => &["green"],
        Color::Blue => &["blue"],
        Color::Orange => &["orange"],
        Color::Pink => &["pink"],
        Color::Black => &["black"],
        Color::White => &["white"],
        Color::Brown => &["brown"],
    }
}

fn sprite_score(s: &SpriteAsset, style: &str, theme: &str, color: Option<Color>) -> i32 {
    let mut score = 0;
    if s.style.iter().any(|x| x == style) {
        score += 100;
    }
    // made for this theme (a space laser for a space shooter's bombs)
    if s.style.iter().any(|x| x == theme) {
        score += 10;
    }
    if let Some(c) = color {
        if color_words(c).iter().any(|w| s.id.contains(w)) {
            score += 20;
        } else if s.tint {
            score += 10;
        }
    }
    if s.frames.len() > 1 {
        score += 5;
    }
    score
}

struct Painter<'a> {
    lib: &'a Library,
    cfg: &'a Config,
    style: &'static str,
    theme: &'static str,
    plan: ArtPlan,
}

impl<'a> Painter<'a> {
    fn best_sprite(&self, kind: &str, color: Option<Color>) -> Option<&'a SpriteAsset> {
        let mut best: Option<(&SpriteAsset, i32)> = None;
        for s in self.lib.sprites_for(kind) {
            let sc = sprite_score(s, self.style, self.theme, color);
            if best.map_or(true, |(_, b)| sc > b) {
                best = Some((s, sc));
            }
        }
        best.map(|(s, _)| s)
    }

    fn tint(&mut self, role: &str, asset_id: &str, tintable: bool, color: Option<Color>) -> [f32; 3] {
        let Some(c) = color else { return [1.0; 3] };
        if color_words(c).iter().any(|w| asset_id.contains(w)) {
            return [1.0; 3];
        }
        if !tintable {
            self.plan.why.push(Why { piece: format!("{role} tinted {} (its art isn't made for tinting)", c.name()), rule: "art".into(), words: c.name().into() });
        }
        self.cfg.color(c)
    }

    /// Install the best sprite for `kind` as sprite `name`, under `role`.
    fn sprite(&mut self, role: &str, kind: &str, name: &str, color: Option<Color>, words: &str) -> Result<(), String> {
        let s = self.best_sprite(kind, color).ok_or_else(|| format!("the asset library has no 2D art for \"{kind}\""))?;
        self.install_sprite(role, s, name, color, words)
    }

    fn install_sprite(&mut self, role: &str, s: &SpriteAsset, name: &str, color: Option<Color>, words: &str) -> Result<(), String> {
        let inst = self.lib.install_sprite(&s.id, name).map_err(|e| format!("{e:#}"))?;
        let (w, h) = self.lib.sprite_size(&s.id).map_err(|e| format!("{e:#}"))?;
        let frames = if inst.names.len() > 1 { inst.names.clone() } else { vec![] };
        let fps = if s.fps > 0.0 { s.fps } else { self.cfg.art.enemy_fps };
        let styled = s.style.iter().any(|x| x == self.style);
        let rule = if styled { format!("art/{}", self.style) } else { format!("art/{} (no {} art for this kind; any style)", self.style, self.style) };
        self.plan.why.push(Why { piece: format!("{role} drawn with {} ({})", s.id, s.pack), rule, words: words.to_string() });
        let color = self.tint(role, &s.id, s.tint, color);
        self.plan.install.merge(inst);
        self.plan.picks.insert(role.to_string(), Pick { asset: s.id.clone(), name: name.to_string(), frames, fps, aspect: w as f32 / h.max(1) as f32, depth_ratio: 1.0, model: false, color });
        Ok(())
    }

    fn model(&mut self, role: &str, kind: &str, name: &str, color: Option<Color>, words: &str) -> Result<(), String> {
        let mut k = kind.to_string();
        let mut rule = format!("art/{}", self.style);
        if self.lib.models_for(&k).is_empty() {
            if let Some(f) = self.cfg.art.model_fallback.get(kind) {
                rule = format!("art/{} + rules.toml art.model_fallback.{kind} = {f}", self.style);
                k = f.clone();
            }
        }
        let m = self.lib.models_for(&k).into_iter().next().ok_or_else(|| format!("the asset library has no 3D model for \"{kind}\""))?;
        let inst = self.lib.install_model(&m.id, name).map_err(|e| format!("{e:#}"))?;
        let dir = self.lib.pack_dir(&m.pack);
        let tex = m.texture.as_ref().map(|t| dir.join(t));
        let imported = crate::models::import(&dir.join(&m.file), &m.groups, tex.as_deref()).map_err(|e| format!("model {}: {e:#}", m.id))?;
        let s = imported.size;
        self.plan.why.push(Why { piece: format!("{role} modelled with {} ({})", m.id, m.pack), rule, words: words.to_string() });
        let color = match color {
            Some(c) => self.cfg.color(c),
            None => [1.0; 3],
        };
        self.plan.install.merge(inst);
        self.plan.picks.insert(
            role.to_string(),
            Pick { asset: m.id.clone(), name: name.to_string(), frames: vec![], fps: 0.0, aspect: s.x / s.y.max(1e-3), depth_ratio: s.z / s.y.max(1e-3), model: true, color },
        );
        Ok(())
    }

    fn thing(&mut self, depth: bool, role: &str, kind: &str, name: &str, color: Option<Color>, words: &str) -> Result<(), String> {
        if depth {
            self.model(role, kind, name, color, words)
        } else {
            self.sprite(role, kind, name, color, words)
        }
    }
}

/// Pick and install the art for a design.
pub fn paint(d: &Design, cfg: &Config, lib: &Library) -> Result<ArtPlan, String> {
    let style = d.variant("art");
    let style = if style.is_empty() { "pixel" } else { style };
    let mut p = Painter { lib, cfg, style, theme: d.theme.name(), plan: ArtPlan::default() };

    // the player
    let pkind = d.player.name();
    if d.depth {
        p.model("player", pkind, "hero", d.player_color, pkind)?;
    } else {
        let styled = lib.sprites_for(pkind).iter().any(|s| s.style.iter().any(|x| x == style));
        let kind = if !styled && !d.player_stated && d.genre != Genre::Shooter { cfg.art.player_fallback.as_str() } else { pkind };
        if kind != pkind {
            p.plan.why.push(Why { piece: format!("player drawn as a {kind} (no {style} {pkind})"), rule: "rules.toml art.player_fallback".into(), words: String::new() });
        }
        p.sprite("player", kind, "hero", d.player_color, pkind)?;
    }
    for e in &d.enemies {
        p.thing(d.depth, &format!("enemy:{}", e.kind.name()), e.kind.name(), e.kind.name(), e.color, e.kind.name())?;
    }
    for k in &d.pickups {
        p.thing(d.depth, &format!("pickup:{}", k.kind.name()), k.kind.name(), k.kind.name(), k.color, k.kind.name())?;
    }
    for h in &d.hazards {
        p.thing(d.depth, &format!("hazard:{}", h.kind.name()), h.kind.name(), h.kind.name(), None, h.kind.name())?;
    }
    if let Some(x) = d.exit {
        p.thing(d.depth, "exit", x.name(), x.name(), None, x.name())?;
    }
    // projectiles are sprites in 2D and 3D
    if d.player_shoots {
        p.sprite("shot", "shot", "shot", None, "")?;
    }
    if d.enemies.iter().any(|e| e.shoots) {
        p.sprite("bomb", "bomb", "bomb", None, "")?;
    }
    // decor
    let decor = cfg.art.decor.get(d.theme.name()).cloned().unwrap_or_default();
    for kind in &decor {
        if d.genre == Genre::Shooter {
            break;
        }
        let role = format!("decor:{kind}");
        let has = if d.depth { !lib.models_for(kind).is_empty() } else { !lib.sprites_for(kind).is_empty() };
        if has {
            p.thing(d.depth, &role, kind, kind, None, d.theme.name())?;
        }
    }
    if !d.depth && d.genre == Genre::Platformer && clouds(d) {
        p.sprite("cloud", "cloud", "cloud", None, "")?;
    }
    // tiles (2D ground, platforms, walls, floor)
    if !d.depth && d.genre != Genre::Shooter {
        let theme = cfg.art.tile_theme.get(d.theme.name()).cloned().unwrap_or_else(|| "forest".into());
        let sets = lib.tilesets_for(&theme);
        let set = sets.iter().find(|t| t.style.iter().any(|x| x == style)).or(sets.first()).copied();
        let set = match set {
            Some(s) => s,
            None => lib.tilesets.first().ok_or("the asset library has no tilesets")?,
        };
        let inst = lib.install_tileset(&set.id, "tiles", cfg.art.tile_size).map_err(|e| format!("{e:#}"))?;
        p.plan.install.merge(inst);
        p.plan.why.push(Why { piece: format!("ground, platforms and walls tiled with {} ({})", set.id, set.pack), rule: format!("art/{style} + rules.toml art.tile_theme.{}", d.theme.name()), words: d.theme.name().into() });
        p.plan.tiles = Some(Tiles { ground: "tiles_ground".into(), platform: "tiles_platform".into(), wall: "tiles_wall".into() });
    }
    Ok(p.plan)
}

/// Outdoor themes get clouds in the sky.
pub fn clouds(d: &Design) -> bool {
    !matches!(d.theme, Theme::Cave | Theme::Castle | Theme::Lava | Theme::Space | Theme::Ocean)
}

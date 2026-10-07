//! The API game code is written against. A game is a Rust type implementing [`Game`]; the
//! engine calls `init` once (and again after every restart), `update` every frame and
//! `on_collision` for every pair of things that touched this frame.
//!
//! Games keep their own state (score, lives, timers) as typed fields on their struct.
//! Everything in the world (positions, velocities, sprites, text) goes through [`Ctx`].

use crate::sound::SoundCmd;
use crate::spec::BodyKind;
use crate::world::{Body, Tags, Transform, Visual, World};
pub use glam::Vec3;

pub trait Game {
    fn init(&mut self, _cx: &mut Ctx) {}
    fn update(&mut self, _cx: &mut Ctx, _dt: f32) {}
    fn on_collision(&mut self, _cx: &mut Ctx, _a: &str, _b: &str) {}
    /// Whether the round is still going. Checks and `DROID_SIM` read this.
    fn status(&self) -> Status {
        Status::Playing
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Playing,
    Won,
    Lost,
}

impl Status {
    pub fn name(self) -> &'static str {
        match self {
            Status::Playing => "playing",
            Status::Won => "won",
            Status::Lost => "lost",
        }
    }
}

/// Access to the running world for game code.
pub struct Ctx<'a> {
    pub(crate) w: &'a mut World,
}

pub fn vec2(x: f32, y: f32) -> Vec3 {
    Vec3::new(x, y, 0.0)
}

pub fn vec3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

impl<'a> Ctx<'a> {
    pub fn new(w: &'a mut World) -> Ctx<'a> {
        Ctx { w }
    }

    // ---- time and input ----
    pub fn time(&self) -> f32 {
        self.w.time
    }
    pub fn key(&self, k: &str) -> bool {
        self.w.input.down.contains(k)
    }
    pub fn key_pressed(&self, k: &str) -> bool {
        self.w.input.pressed.contains(k)
    }
    /// -1..1 from A/D or the left/right arrows.
    pub fn axis_x(&self) -> f32 {
        let d = &self.w.input.down;
        (d.contains("d") || d.contains("right")) as i32 as f32 - (d.contains("a") || d.contains("left")) as i32 as f32
    }
    /// -1..1 from S/W or the down/up arrows.
    pub fn axis_y(&self) -> f32 {
        let d = &self.w.input.down;
        (d.contains("w") || d.contains("up")) as i32 as f32 - (d.contains("s") || d.contains("down")) as i32 as f32
    }
    /// Loudness of the microphone, 0..1, smoothed: quiet ~0, normal speech ~0.3, a shout 0.8+.
    /// 0 without a microphone. Headless (`DROID_SIM`) it is 0, or the `DROID_VOICE` schedule
    /// (`0:0,3:0.9,6:0`: seconds:level steps, or one constant). See [`crate::voice`].
    pub fn voice(&self) -> f32 {
        crate::voice::level(self.w.time)
    }
    /// A random number in `lo..hi`. With `DROID_SEED=<n>` the numbers come from that seed (the
    /// same run twice gives the same game: headless checks); otherwise from the system.
    pub fn random(&self, lo: f32, hi: f32) -> f32 {
        if hi <= lo {
            lo
        } else {
            lo + crate::seeded::next_f32() * (hi - lo)
        }
    }

    // ---- reading entities ----
    pub fn exists(&self, n: &str) -> bool {
        self.w.entity(n).is_some()
    }
    pub fn pos(&self, n: &str) -> Vec3 {
        self.w.entity(n).and_then(|e| self.w.ecs.get::<&Transform>(e).ok().map(|t| t.pos)).unwrap_or_default()
    }
    pub fn vel(&self, n: &str) -> Vec3 {
        self.w.entity(n).and_then(|e| self.w.ecs.get::<&Body>(e).ok().map(|b| b.vel)).unwrap_or_default()
    }
    /// The entity's size (its scale: a sprite or box is this many units across).
    pub fn scale(&self, n: &str) -> Vec3 {
        self.w.entity(n).and_then(|e| self.w.ecs.get::<&Transform>(e).ok().map(|t| t.scale)).unwrap_or(Vec3::ONE)
    }
    pub fn grounded(&self, n: &str) -> bool {
        self.w.entity(n).and_then(|e| self.w.ecs.get::<&Body>(e).ok().map(|b| b.grounded)).unwrap_or(false)
    }
    pub fn has_tag(&self, n: &str, tag: &str) -> bool {
        self.w.entity(n).and_then(|e| self.w.ecs.get::<&Tags>(e).ok().map(|t| t.0.iter().any(|x| x == tag))).unwrap_or(false)
    }
    /// Names of every entity with `tag`, sorted (stable iteration order).
    pub fn find_tag(&self, tag: &str) -> Vec<String> {
        let mut v: Vec<String> = self
            .w
            .ecs
            .query::<(&crate::world::Name, &Tags)>()
            .iter()
            .filter(|(_, (_, t))| t.0.iter().any(|x| x == tag))
            .map(|(_, (n, _))| n.0.clone())
            .collect();
        v.sort();
        v
    }
    pub fn count_tag(&self, tag: &str) -> usize {
        self.w.ecs.query::<&Tags>().iter().filter(|(_, t)| t.0.iter().any(|x| x == tag)).count()
    }

    // ---- changing entities ----
    fn with<C: hecs::Component>(&mut self, n: &str, f: impl FnOnce(&mut C)) {
        if let Some(e) = self.w.entity(n) {
            if let Ok(mut c) = self.w.ecs.get::<&mut C>(e) {
                f(&mut c);
            }
        }
    }
    pub fn set_pos(&mut self, n: &str, p: Vec3) {
        self.with::<Transform>(n, |t| t.pos = p);
    }
    pub fn move_by(&mut self, n: &str, d: Vec3) {
        self.with::<Transform>(n, |t| t.pos += d);
    }
    pub fn set_vel(&mut self, n: &str, v: Vec3) {
        self.with::<Body>(n, |b| b.vel = v);
    }
    pub fn set_vel_x(&mut self, n: &str, x: f32) {
        self.with::<Body>(n, |b| b.vel.x = x);
    }
    pub fn set_vel_y(&mut self, n: &str, y: f32) {
        self.with::<Body>(n, |b| b.vel.y = y);
    }
    pub fn set_vel_z(&mut self, n: &str, z: f32) {
        self.with::<Body>(n, |b| b.vel.z = z);
    }
    pub fn set_scale(&mut self, n: &str, s: Vec3) {
        self.with::<Transform>(n, |t| t.scale = s);
    }
    /// Euler rotation in degrees (x, y, z); in 2D only z matters (counter-clockwise).
    pub fn set_rotation(&mut self, n: &str, degrees: Vec3) {
        self.with::<Transform>(n, |t| t.rot = degrees);
    }
    pub fn set_color(&mut self, n: &str, c: Vec3) {
        self.with::<Visual>(n, |v| v.color = c);
    }
    pub fn set_flip_x(&mut self, n: &str, f: bool) {
        self.with::<Visual>(n, |v| v.flip_x = f);
    }
    pub fn set_solid(&mut self, n: &str, s: bool) {
        self.with::<Body>(n, |b| b.solid = s);
    }
    pub fn set_sprite(&mut self, n: &str, sprite: &str) {
        let idx = self.w.sprite_index(sprite);
        self.with::<Visual>(n, |v| {
            if idx.is_some() {
                v.sprite = idx;
                v.shape = crate::spec::Shape::Plane;
            }
        });
    }
    pub fn add_tag(&mut self, n: &str, tag: &str) {
        let tag = tag.to_string();
        self.with::<Tags>(n, |t| {
            if !t.0.contains(&tag) {
                t.0.push(tag)
            }
        });
    }
    /// Cycle through sprite frames at `fps`.
    pub fn animate_sprite(&mut self, n: &str, frames: &[&str], fps: f32) {
        let idx: Vec<usize> = frames.iter().filter_map(|f| self.w.sprite_index(f)).collect();
        if let Some(e) = self.w.entity(n) {
            let _ = self.w.ecs.insert_one(e, crate::anim::SpriteAnim { frames: idx, fps, t: 0.0, looping: true });
        }
    }
    /// Quick squash-and-stretch that springs back.
    pub fn squash(&mut self, n: &str, amount: f32) {
        let Some(e) = self.w.entity(n) else { return };
        let base = self.w.ecs.get::<&Transform>(e).map(|t| t.scale).unwrap_or(Vec3::ONE);
        let a = amount.clamp(0.0, 0.9);
        self.with::<Transform>(n, |t| t.scale = Vec3::new(base.x * (1.0 + a), base.y * (1.0 - a), base.z * (1.0 + a)));
        let tween = crate::anim::Tween { kind: crate::anim::TweenKind::Scale, from: self.pos_scale(n), to: base, t: 0.0, dur: 0.25 };
        let _ = self.w.ecs.insert_one(e, crate::anim::Tweens(vec![tween]));
    }
    fn pos_scale(&self, n: &str) -> Vec3 {
        self.w.entity(n).and_then(|e| self.w.ecs.get::<&Transform>(e).ok().map(|t| t.scale)).unwrap_or(Vec3::ONE)
    }

    // ---- spawning ----
    /// Spawn a sprite entity; returns its actual (unique) name.
    pub fn spawn_sprite(&mut self, name: &str, tags: &[&str], sprite: &str, pos: Vec3, scale: Vec3, body: BodyKind) -> String {
        let spec = crate::spec::EntitySpec {
            name: name.to_string(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            shape: crate::spec::Shape::Plane,
            sprite: Some(sprite.to_string()),
            model: None,
            position: pos.into(),
            rotation: [0.0; 3],
            scale: scale.into(),
            color: [1.0; 3],
            physics: crate::spec::PhysicsSpec { body, ..Default::default() },
        };
        let e = self.w.spawn_spec(&spec);
        self.w.name_of(e)
    }
    /// Spawn a plain shape ("box", "sphere" or "plane") in a colour; returns its actual name.
    pub fn spawn_shape(&mut self, name: &str, tags: &[&str], shape: &str, pos: Vec3, scale: Vec3, color: Vec3, body: BodyKind) -> String {
        let shape = match shape {
            "sphere" => crate::spec::Shape::Sphere,
            "plane" => crate::spec::Shape::Plane,
            _ => crate::spec::Shape::Box,
        };
        let spec = crate::spec::EntitySpec {
            name: name.to_string(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            shape,
            sprite: None,
            model: None,
            position: pos.into(),
            rotation: [0.0; 3],
            scale: scale.into(),
            color: color.into(),
            physics: crate::spec::PhysicsSpec { body, ..Default::default() },
        };
        let e = self.w.spawn_spec(&spec);
        self.w.name_of(e)
    }
    /// One-shot model animation: "jump", "attack", "hit", "spin", "wave" or "nod".
    pub fn play_anim(&mut self, n: &str, anim: &str) {
        let anim = anim.to_string();
        self.with::<crate::anim::Pose>(n, |p| p.oneshot = Some((anim, 0.0)));
    }
    pub fn remove_text(&mut self, id: &str) {
        self.w.texts.retain(|t| t.id != id);
    }
    pub fn destroy(&mut self, n: &str) {
        self.w.despawn(n);
    }
    /// Spawn again every entity of the scene that has `tag` (e.g. the next wave).
    /// Returns how many were spawned.
    pub fn respawn_tag(&mut self, tag: &str) -> usize {
        let t: Vec<crate::spec::EntitySpec> = self.w.templates.iter().filter(|e| e.tags.iter().any(|x| x == tag)).cloned().collect();
        for e in &t {
            self.w.spawn_spec(e);
        }
        t.len()
    }
    /// Spawn again the scene entity that was authored as `name` (its kind, art and start
    /// position). Returns the new entity's name, or None when the scene has no such entity.
    pub fn respawn(&mut self, name: &str) -> Option<String> {
        let spec = self.w.templates.iter().find(|e| e.name == name).cloned()?;
        let e = self.w.spawn_spec(&spec);
        Some(self.w.name_of(e))
    }
    pub fn destroy_tag(&mut self, tag: &str) {
        for n in self.find_tag(tag) {
            self.w.despawn(&n);
        }
    }

    // ---- screen, sound, game ----
    pub fn text(&mut self, id: &str, s: &str) {
        self.w.set_text(id, s);
    }
    pub fn text_at(&mut self, id: &str, s: &str, x: f32, y: f32, size: f32) {
        self.w.set_text(id, s);
        if let Some(t) = self.w.texts.iter_mut().find(|t| t.id == id) {
            t.x = x;
            t.y = y;
            t.size = size;
        }
    }
    /// A filled rectangle on the screen: `x`, `y` (top left), `w`, `h` in fractions of the
    /// screen, drawn under the texts; the same `id` moves and recolours it.
    pub fn rect_at(&mut self, id: &str, x: f32, y: f32, w: f32, h: f32, color: Vec3) {
        let r = crate::world::UiRect { id: id.to_string(), x, y, w: w.max(0.0), h: h.max(0.0), color: color.into() };
        match self.w.rects.iter_mut().find(|r| r.id == id) {
            Some(old) => *old = r,
            None => self.w.rects.push(r),
        }
    }
    pub fn remove_rect(&mut self, id: &str) {
        self.w.rects.retain(|r| r.id != id);
    }
    pub fn text_color(&mut self, id: &str, c: Vec3) {
        if let Some(t) = self.w.texts.iter_mut().find(|t| t.id == id) {
            t.color = c.into();
        }
    }
    pub fn play_sound(&mut self, name: &str) {
        self.w.sounds.push(SoundCmd::Effect(name.to_string()));
    }
    pub fn play_music(&mut self, bpm: f32, notes: &str, wave: &str) {
        self.w.sounds.push(SoundCmd::Music { bpm, notes: notes.to_string(), wave: wave.to_string(), volume: 0.4 });
    }
    /// Rebuild the scene from its files and call `init` again with a fresh game.
    pub fn restart(&mut self) {
        self.w.restart_requested = true;
    }
}

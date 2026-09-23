//! Animation: procedural part-group animation for 3D models (walk / idle / jump / one-shots),
//! sprite frame sequences, and simple tweens. No keyframes or skinning: model parts are grouped
//! into rigid "bones" (body, head, arm_l, arm_r, leg_l, leg_r, tail, ...) and posed each frame.

use crate::spec::BodyKind;
use crate::world::{Body, Transform, Visual, World};
use glam::Vec3;
use std::collections::HashMap;

/// Pose of one part group: rotation (degrees, about the group's pivot) and translation offset.
#[derive(Clone, Copy, Debug, Default)]
pub struct GroupPose {
    pub rot: Vec3,
    pub offset: Vec3,
}

#[derive(Clone, Debug)]
pub enum AnimMode {
    /// Engine picks walk / idle / air from movement.
    Auto,
    /// A named looping animation: "walk", "idle", "none".
    Named(String),
}

/// Per-entity animation state for model entities.
#[derive(Clone, Debug)]
pub struct Pose {
    pub groups: HashMap<String, GroupPose>,
    /// Manual overrides from `set_part`, added on top of the procedural pose.
    pub manual: HashMap<String, GroupPose>,
    pub mode: AnimMode,
    pub phase: f32,
    /// (name, elapsed seconds) of a one-shot such as "jump" or "attack".
    pub oneshot: Option<(String, f32)>,
    /// Turn the entity to face its horizontal velocity (3D).
    pub face_velocity: bool,
    pub prev_pos: Vec3,
}

impl Default for Pose {
    fn default() -> Self {
        Pose { groups: HashMap::new(), manual: HashMap::new(), mode: AnimMode::Auto, phase: 0.0, oneshot: None, face_velocity: true, prev_pos: Vec3::ZERO }
    }
}

/// Sprite frame cycling.
#[derive(Clone, Debug)]
pub struct SpriteAnim {
    pub frames: Vec<usize>,
    pub fps: f32,
    pub t: f32,
    pub looping: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TweenKind {
    Pos,
    Scale,
    Rot,
    Color,
}

#[derive(Clone, Debug)]
pub struct Tween {
    pub kind: TweenKind,
    pub from: Vec3,
    pub to: Vec3,
    pub t: f32,
    pub dur: f32,
}

#[derive(Clone, Debug, Default)]
pub struct Tweens(pub Vec<Tween>);

fn ease(t: f32) -> f32 {
    // smoothstep
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

const ONESHOT_LEN: f32 = 0.45;

pub fn step(world: &mut World, dt: f32) {
    let three_d = world.view == crate::spec::View::ThreeD;

    // ---- model part-group poses ----
    for (_, (t, b, v, p)) in world.ecs.query_mut::<(&mut Transform, Option<&Body>, &Visual, &mut Pose)>() {
        if v.model.is_none() {
            continue;
        }
        // Velocity: from body if it has one, else from position delta (kinematic via set_pos/move_by).
        let vel = match b {
            Some(b) if b.kind == BodyKind::Dynamic || (b.kind == BodyKind::Kinematic && b.vel.length_squared() > 1e-4) => b.vel,
            _ => (t.pos - p.prev_pos) / dt.max(1e-4),
        };
        p.prev_pos = t.pos;
        let horiz = Vec3::new(vel.x, 0.0, if three_d { vel.z } else { 0.0 });
        let speed = horiz.length();
        let grounded = b.map_or(true, |b| b.kind != BodyKind::Dynamic || b.grounded);
        let airborne = !grounded && b.map_or(false, |b| b.kind == BodyKind::Dynamic);

        if p.face_velocity && three_d && speed > 0.5 {
            let target = horiz.x.atan2(horiz.z).to_degrees();
            let cur = t.rot.y;
            let mut d = (target - cur + 540.0) % 360.0 - 180.0;
            d = d.clamp(-720.0 * dt, 720.0 * dt);
            t.rot.y = cur + d;
        }

        let mode = match &p.mode {
            AnimMode::Named(n) => n.clone(),
            AnimMode::Auto => {
                if airborne {
                    "air".into()
                } else if speed > 0.4 {
                    "walk".into()
                } else {
                    "idle".into()
                }
            }
        };
        let mut g: HashMap<String, GroupPose> = HashMap::new();
        let mut set = |name: &str, rot: Vec3, off: Vec3| {
            g.insert(name.to_string(), GroupPose { rot, offset: off });
        };
        match mode.as_str() {
            "walk" => {
                p.phase += dt * (6.0 + speed * 1.2);
                let s = p.phase.sin();
                let swing = 35.0 * s;
                set("leg_l", Vec3::new(swing, 0.0, 0.0), Vec3::ZERO);
                set("leg_r", Vec3::new(-swing, 0.0, 0.0), Vec3::ZERO);
                set("arm_l", Vec3::new(-swing * 0.8, 0.0, 0.0), Vec3::ZERO);
                set("arm_r", Vec3::new(swing * 0.8, 0.0, 0.0), Vec3::ZERO);
                let bob = (p.phase * 2.0).sin().abs() * 0.05;
                set("body", Vec3::new(0.0, 0.0, 3.0 * s), Vec3::new(0.0, bob, 0.0));
                set("head", Vec3::new(3.0 * (p.phase * 2.0).sin(), 0.0, -2.0 * s), Vec3::new(0.0, bob, 0.0));
                set("tail", Vec3::new(0.0, 25.0 * s, 0.0), Vec3::ZERO);
            }
            "air" => {
                set("leg_l", Vec3::new(-25.0, 0.0, 0.0), Vec3::ZERO);
                set("leg_r", Vec3::new(20.0, 0.0, 0.0), Vec3::ZERO);
                set("arm_l", Vec3::new(-150.0, 0.0, 0.0), Vec3::ZERO);
                set("arm_r", Vec3::new(-150.0, 0.0, 0.0), Vec3::ZERO);
                set("body", Vec3::ZERO, Vec3::new(0.0, 0.05, 0.0));
            }
            "none" => {}
            _ => {
                // idle: breathing
                p.phase += dt * 2.0;
                let br = (p.phase).sin();
                set("body", Vec3::ZERO, Vec3::new(0.0, 0.015 * br, 0.0));
                set("head", Vec3::new(2.0 * br, 0.0, 0.0), Vec3::new(0.0, 0.02 * br, 0.0));
                set("arm_l", Vec3::new(0.0, 0.0, -3.0 * br), Vec3::ZERO);
                set("arm_r", Vec3::new(0.0, 0.0, 3.0 * br), Vec3::ZERO);
                set("tail", Vec3::new(0.0, 10.0 * br, 0.0), Vec3::ZERO);
            }
        }

        // One-shots layered on top.
        if let Some((name, elapsed)) = p.oneshot.as_mut() {
            *elapsed += dt;
            let k = (*elapsed / ONESHOT_LEN).clamp(0.0, 1.0);
            let pulse = (k * std::f32::consts::PI).sin(); // 0 -> 1 -> 0
            let add = |g: &mut HashMap<String, GroupPose>, name: &str, rot: Vec3, off: Vec3| {
                let e = g.entry(name.to_string()).or_default();
                e.rot += rot;
                e.offset += off;
            };
            match name.as_str() {
                "jump" => {
                    add(&mut g, "arm_l", Vec3::new(-160.0 * pulse, 0.0, 0.0), Vec3::ZERO);
                    add(&mut g, "arm_r", Vec3::new(-160.0 * pulse, 0.0, 0.0), Vec3::ZERO);
                    add(&mut g, "body", Vec3::ZERO, Vec3::new(0.0, 0.08 * pulse, 0.0));
                }
                "attack" => {
                    add(&mut g, "arm_r", Vec3::new(-120.0 * pulse, 0.0, 0.0), Vec3::ZERO);
                    add(&mut g, "body", Vec3::new(0.0, -15.0 * pulse, 0.0), Vec3::ZERO);
                }
                "hit" => {
                    add(&mut g, "body", Vec3::new(-15.0 * pulse, 0.0, 12.0 * (k * 20.0).sin()), Vec3::ZERO);
                    add(&mut g, "head", Vec3::new(-20.0 * pulse, 0.0, 0.0), Vec3::ZERO);
                }
                "spin" => {
                    add(&mut g, "body", Vec3::new(0.0, 360.0 * ease(k), 0.0), Vec3::ZERO);
                    add(&mut g, "head", Vec3::new(0.0, 360.0 * ease(k), 0.0), Vec3::ZERO);
                }
                "wave" => {
                    add(&mut g, "arm_r", Vec3::new(-160.0 * pulse.max(0.3), 0.0, 25.0 * (k * 25.0).sin()), Vec3::ZERO);
                }
                "nod" => {
                    add(&mut g, "head", Vec3::new(25.0 * (k * 12.0).sin() * (1.0 - k), 0.0, 0.0), Vec3::ZERO);
                }
                _ => {}
            }
            if k >= 1.0 {
                p.oneshot = None;
            }
        }

        // Manual overrides.
        for (name, m) in &p.manual {
            let e = g.entry(name.clone()).or_default();
            e.rot += m.rot;
            e.offset += m.offset;
        }
        p.groups = g;
    }

    // ---- sprite frame animation ----
    for (_, (v, a)) in world.ecs.query_mut::<(&mut Visual, &mut SpriteAnim)>() {
        if a.frames.is_empty() || a.fps <= 0.0 {
            continue;
        }
        a.t += dt * a.fps;
        let n = a.frames.len();
        let i = a.t as usize;
        let idx = if a.looping { i % n } else { i.min(n - 1) };
        v.sprite = Some(a.frames[idx]);
    }

    // ---- tweens ----
    for (_, (t, v, tw)) in world.ecs.query_mut::<(&mut Transform, &mut Visual, &mut Tweens)>() {
        for tween in tw.0.iter_mut() {
            tween.t += dt;
            let k = ease(tween.t / tween.dur.max(1e-4));
            let val = tween.from.lerp(tween.to, k);
            match tween.kind {
                TweenKind::Pos => t.pos = val,
                TweenKind::Scale => t.scale = val,
                TweenKind::Rot => t.rot = val,
                TweenKind::Color => v.color = val,
            }
        }
        tw.0.retain(|tw| tw.t < tw.dur);
    }
}

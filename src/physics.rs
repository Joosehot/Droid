//! Minimal AABB physics: gravity, integration, overlap resolution, collision events.

use crate::spec::BodyKind;
use crate::world::{Body, Transform, World};
use glam::Vec3;
use hecs::Entity;

struct Aabb {
    min: Vec3,
    max: Vec3,
}

fn aabb(t: &Transform) -> Aabb {
    let half = t.scale.abs() * 0.5;
    Aabb { min: t.pos - half, max: t.pos + half }
}

fn overlap(a: &Aabb, b: &Aabb) -> Option<Vec3> {
    let dx = (a.max.x.min(b.max.x)) - (a.min.x.max(b.min.x));
    let dy = (a.max.y.min(b.max.y)) - (a.min.y.max(b.min.y));
    let dz = (a.max.z.min(b.max.z)) - (a.min.z.max(b.min.z));
    if dx <= 0.0 || dy <= 0.0 || dz <= 0.0 {
        return None;
    }
    Some(Vec3::new(dx, dy, dz))
}

pub fn step(world: &mut World, dt: f32) {
    world.collisions.clear();
    let gravity = world.gravity;
    let two_d = world.view == crate::spec::View::TwoD;

    // Integrate.
    for (_, (t, b)) in world.ecs.query_mut::<(&mut Transform, &mut Body)>() {
        match b.kind {
            BodyKind::Dynamic => {
                b.vel += gravity * dt;
                b.grounded = false;
                t.pos += b.vel * dt;
            }
            BodyKind::Kinematic => {
                t.pos += b.vel * dt;
            }
            _ => {}
        }
        if two_d {
            t.pos.z = 0.0;
            b.vel.z = 0.0;
        }
    }

    // Gather colliders.
    let mut items: Vec<(Entity, Aabb, BodyKind, bool)> = Vec::new();
    for (e, (t, b)) in world.ecs.query::<(&Transform, &Body)>().iter() {
        if b.kind == BodyKind::None {
            continue;
        }
        let mut bb = aabb(t);
        if two_d {
            // Treat everything as overlapping in z so thin/flat shapes still collide.
            bb.min.z = -1.0;
            bb.max.z = 1.0;
        }
        items.push((e, bb, b.kind, b.solid));
    }

    let mut pairs: Vec<(Entity, Entity)> = Vec::new();
    for i in 0..items.len() {
        for j in (i + 1)..items.len() {
            let (ea, ba, ka, sa) = &items[i];
            let (eb, bb, kb, sb) = &items[j];
            if *ka != BodyKind::Dynamic && *kb != BodyKind::Dynamic && *ka != BodyKind::Kinematic && *kb != BodyKind::Kinematic {
                continue; // static-static
            }
            let Some(pen) = overlap(ba, bb) else { continue };
            pairs.push((*ea, *eb));
            if !(*sa && *sb) {
                continue; // trigger: no resolution
            }
            resolve(world, *ea, *eb, *ka, *kb, pen, two_d);
        }
    }
    world.collisions = pairs;
}

fn resolve(world: &mut World, ea: Entity, eb: Entity, ka: BodyKind, kb: BodyKind, pen: Vec3, two_d: bool) {
    let (pa, pb) = {
        let ta = world.ecs.get::<&Transform>(ea).map(|t| t.pos).unwrap_or_default();
        let tb = world.ecs.get::<&Transform>(eb).map(|t| t.pos).unwrap_or_default();
        (ta, tb)
    };
    // Pick the axis of least penetration.
    let mut axis = 0;
    let mut min = pen.x;
    if pen.y < min {
        axis = 1;
        min = pen.y;
    }
    if !two_d && pen.z < min {
        axis = 2;
        min = pen.z;
    }
    let mut n = Vec3::ZERO;
    n[axis] = if pa[axis] < pb[axis] { -1.0 } else { 1.0 };
    let push = n * min;

    let movable = |k: BodyKind| k == BodyKind::Dynamic;
    let (wa, wb) = match (movable(ka), movable(kb)) {
        (true, true) => (0.5, 0.5),
        (true, false) => (1.0, 0.0),
        (false, true) => (0.0, 1.0),
        (false, false) => (0.0, 0.0),
    };

    if wa > 0.0 {
        if let Ok((t, b)) = world.ecs.query_one_mut::<(&mut Transform, &mut Body)>(ea) {
            t.pos += push * wa;
            apply_velocity(b, n, axis);
        }
    }
    if wb > 0.0 {
        if let Ok((t, b)) = world.ecs.query_one_mut::<(&mut Transform, &mut Body)>(eb) {
            t.pos -= push * wb;
            apply_velocity(b, -n, axis);
        }
    }
}

fn apply_velocity(b: &mut Body, n: Vec3, axis: usize) {
    // Only cancel velocity moving into the contact.
    let into = b.vel[axis] * n[axis];
    if into < 0.0 {
        b.vel[axis] = -b.vel[axis] * b.bounciness;
        if b.vel[axis].abs() < 0.05 {
            b.vel[axis] = 0.0;
        }
    }
    if n.y > 0.5 {
        b.grounded = true;
    }
}

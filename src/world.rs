//! Runtime world: ECS storage plus input, camera, UI text and frame events.

use crate::sound::SoundCmd;
use crate::spec::{BodyKind, CameraSpec, EntitySpec, GameSpec, ModelSpec, Shape, SpriteSpec, TextSpec, View};
use glam::Vec3;
use hecs::Entity;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug)]
pub struct Name(pub String);

#[derive(Clone, Debug, Default)]
pub struct Tags(pub Vec<String>);

#[derive(Clone, Copy, Debug)]
pub struct Transform {
    pub pos: Vec3,
    /// Euler degrees.
    pub rot: Vec3,
    pub scale: Vec3,
}

#[derive(Clone, Debug)]
pub struct Visual {
    pub shape: Shape,
    pub color: Vec3,
    /// Index into `World::sprites`, if drawn as a sprite.
    pub sprite: Option<usize>,
    /// Index into `World::models`, if drawn as a procedural 3D model.
    pub model: Option<usize>,
    pub flip_x: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Body {
    pub kind: BodyKind,
    pub vel: Vec3,
    pub bounciness: f32,
    pub solid: bool,
    pub grounded: bool,
}

#[derive(Default)]
pub struct Input {
    pub down: HashSet<String>,
    pub pressed: HashSet<String>,
    pub released: HashSet<String>,
    /// Mouse in normalised device coords (-1..1, y up).
    pub mouse_ndc: [f32; 2],
    pub mouse_down: bool,
    pub mouse_pressed: bool,
}

impl Input {
    pub fn end_frame(&mut self) {
        self.pressed.clear();
        self.released.clear();
        self.mouse_pressed = false;
    }
}

#[derive(Clone, Debug)]
pub struct Camera {
    pub position: Vec3,
    pub look_at: Vec3,
    pub zoom: f32,
    pub follow: Option<String>,
    /// 3D follow: eye offset relative to the followed entity.
    pub follow_offset: Vec3,
}

impl Camera {
    fn from_spec(c: &CameraSpec) -> Self {
        let position = Vec3::from(c.position);
        let look_at = Vec3::from(c.look_at);
        Self { position, look_at, zoom: c.zoom.max(0.1), follow: c.follow.clone(), follow_offset: position - look_at }
    }
}

pub struct World {
    pub ecs: hecs::World,
    pub by_name: HashMap<String, Entity>,
    pub view: View,
    pub camera: Camera,
    pub background: Vec3,
    pub gravity: Vec3,
    pub input: Input,
    pub time: f32,
    pub dt: f32,
    pub texts: Vec<TextSpec>,
    /// Collision pairs found by physics this frame.
    pub collisions: Vec<(Entity, Entity)>,
    pub restart_requested: bool,
    pub spawn_counter: u64,
    /// Script-visible key/value store (persists across frames, cleared on restart).
    pub vars: HashMap<String, rhai::Dynamic>,
    /// Last script error, shown on screen.
    pub error: Option<String>,
    pub aspect: f32,
    /// Sprite definitions in a stable order (index = sprite id).
    pub sprites: Vec<(String, SpriteSpec)>,
    /// Bumped whenever `sprites` changes so the renderer rebuilds its atlas.
    pub sprite_gen: u64,
    pub models: Vec<(String, ModelSpec)>,
    pub model_gen: u64,
    /// Sounds requested by the script this frame; drained by the app.
    pub sounds: Vec<SoundCmd>,
}

impl World {
    pub fn from_spec(spec: &GameSpec) -> Self {
        let mut w = World {
            ecs: hecs::World::new(),
            by_name: HashMap::new(),
            view: spec.view,
            camera: Camera::from_spec(&spec.camera),
            background: Vec3::from(spec.background),
            gravity: Vec3::from(spec.gravity),
            input: Input::default(),
            time: 0.0,
            dt: 0.0,
            texts: spec.texts.clone(),
            collisions: Vec::new(),
            restart_requested: false,
            spawn_counter: 0,
            vars: HashMap::new(),
            error: None,
            aspect: 16.0 / 9.0,
            sprites: spec.sprites.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            sprite_gen: next_gen(),
            models: spec.models.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            model_gen: next_gen(),
            sounds: Vec::new(),
        };
        for e in &spec.entities {
            w.spawn_spec(e);
        }
        w
    }

    pub fn spawn_spec(&mut self, e: &EntitySpec) -> Entity {
        let name = self.unique_name(&e.name);
        let ent = self.ecs.spawn((
            Name(name.clone()),
            Tags(e.tags.clone()),
            Transform { pos: Vec3::from(e.position), rot: Vec3::from(e.rotation), scale: Vec3::from(e.scale) },
            Visual {
                shape: if e.sprite.is_some() { Shape::Plane } else { e.shape },
                color: Vec3::from(e.color),
                sprite: e.sprite.as_deref().and_then(|n| self.sprite_index(n)),
                model: e.model.as_deref().and_then(|n| self.model_index(n)),
                flip_x: false,
            },
            Body {
                kind: e.physics.body,
                vel: Vec3::from(e.physics.velocity),
                bounciness: e.physics.bounciness,
                solid: e.physics.solid,
                grounded: false,
            },
        ));
        self.by_name.insert(name, ent);
        ent
    }

    /// Returns `base` if unused, otherwise `base_N`.
    pub fn unique_name(&mut self, base: &str) -> String {
        if !self.by_name.contains_key(base) {
            return base.to_string();
        }
        loop {
            self.spawn_counter += 1;
            let candidate = format!("{base}_{}", self.spawn_counter);
            if !self.by_name.contains_key(&candidate) {
                return candidate;
            }
        }
    }

    /// Hot-swap sprites and models (art-only reload): re-resolve every entity's sprite/model index by name.
    pub fn replace_art(&mut self, spec: &GameSpec) {
        let old_sprites: Vec<String> = self.sprites.iter().map(|(n, _)| n.clone()).collect();
        let old_models: Vec<String> = self.models.iter().map(|(n, _)| n.clone()).collect();
        self.sprites = spec.sprites.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        self.models = spec.models.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        self.sprite_gen = next_gen();
        self.model_gen = next_gen();
        let sprites = self.sprites.clone();
        let models = self.models.clone();
        for (_, v) in self.ecs.query_mut::<&mut Visual>() {
            v.sprite = v.sprite.and_then(|i| old_sprites.get(i)).and_then(|n| sprites.iter().position(|(m, _)| m == n));
            v.model = v.model.and_then(|i| old_models.get(i)).and_then(|n| models.iter().position(|(m, _)| m == n));
        }
    }

    pub fn sprite_index(&self, name: &str) -> Option<usize> {
        self.sprites.iter().position(|(n, _)| n == name)
    }

    pub fn model_index(&self, name: &str) -> Option<usize> {
        self.models.iter().position(|(n, _)| n == name)
    }

    pub fn entity(&self, name: &str) -> Option<Entity> {
        self.by_name.get(name).copied()
    }

    pub fn name_of(&self, e: Entity) -> String {
        self.ecs.get::<&Name>(e).map(|n| n.0.clone()).unwrap_or_default()
    }

    pub fn despawn(&mut self, name: &str) -> bool {
        if let Some(e) = self.by_name.remove(name) {
            let _ = self.ecs.despawn(e);
            true
        } else {
            false
        }
    }

    pub fn set_text(&mut self, id: &str, text: &str) {
        if let Some(t) = self.texts.iter_mut().find(|t| t.id == id) {
            t.text = text.to_string();
        } else {
            self.texts.push(TextSpec {
                id: id.to_string(),
                text: text.to_string(),
                x: 0.02,
                y: 0.02 + 0.06 * self.texts.len() as f32,
                size: 24.0,
                color: [1.0; 3],
            });
        }
    }

    /// Update camera to follow its target, if any.
    pub fn update_camera(&mut self) {
        let Some(target) = self.camera.follow.clone() else { return };
        let Some(e) = self.entity(&target) else { return };
        let Ok(t) = self.ecs.get::<&Transform>(e) else { return };
        let p = t.pos;
        match self.view {
            View::TwoD => {
                self.camera.position.x = p.x;
                self.camera.position.y = p.y;
            }
            View::ThreeD => {
                self.camera.look_at = p;
                self.camera.position = p + self.camera.follow_offset;
            }
        }
    }

    /// Mouse position converted to world XY on the z=0 plane (2D view only; 3D returns camera look_at XY).
    pub fn mouse_world(&self) -> Vec3 {
        match self.view {
            View::TwoD => {
                let h = self.camera.zoom;
                let w = h * self.aspect;
                Vec3::new(
                    self.camera.position.x + self.input.mouse_ndc[0] * w * 0.5,
                    self.camera.position.y + self.input.mouse_ndc[1] * h * 0.5,
                    0.0,
                )
            }
            View::ThreeD => self.camera.look_at,
        }
    }
}

fn next_gen() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static GEN: AtomicU64 = AtomicU64::new(1);
    GEN.fetch_add(1, Ordering::Relaxed)
}

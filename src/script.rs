//! Rhai scripting host. The AI-written script defines `init()`, `update(dt)` and
//! `on_collision(a, b)`; this module exposes the engine API to it.

use crate::spec::{BodyKind, EntitySpec, PhysicsSpec, Shape};
use crate::world::{Body, Tags, Transform, Visual, World};
use glam::Vec3;
use rhai::{Array, Dynamic, Engine, EvalAltResult, Scope, AST};
use std::cell::RefCell;
use std::rc::Rc;

pub type Shared = Rc<RefCell<World>>;

pub struct ScriptHost {
    engine: Engine,
    ast: Option<AST>,
    scope: Scope<'static>,
    broken: bool,
}

fn num(d: &Dynamic) -> f32 {
    if let Some(f) = d.clone().try_cast::<f32>() {
        f
    } else if let Some(i) = d.clone().try_cast::<i64>() {
        i as f32
    } else {
        0.0
    }
}

fn to_vec3(d: &Dynamic) -> Vec3 {
    if let Some(v) = d.clone().try_cast::<Vec3>() {
        v
    } else if let Some(arr) = d.clone().try_cast::<Array>() {
        let g = |i: usize| arr.get(i).map(num).unwrap_or(0.0);
        Vec3::new(g(0), g(1), g(2))
    } else {
        Vec3::ZERO
    }
}

fn shape_of(s: &str) -> Shape {
    match s.to_ascii_lowercase().as_str() {
        "sphere" | "ball" | "circle" => Shape::Sphere,
        "plane" | "quad" | "rect" => Shape::Plane,
        _ => Shape::Box,
    }
}

fn body_of(s: &str) -> BodyKind {
    match s.to_ascii_lowercase().as_str() {
        "static" => BodyKind::Static,
        "kinematic" => BodyKind::Kinematic,
        "dynamic" => BodyKind::Dynamic,
        _ => BodyKind::None,
    }
}

fn err(msg: impl Into<String>) -> Box<EvalAltResult> {
    Box::new(EvalAltResult::ErrorRuntime(msg.into().into(), rhai::Position::NONE))
}

impl ScriptHost {
    pub fn new(world: Shared) -> Self {
        let mut engine = Engine::new();
        engine.set_max_expr_depths(0, 0);
        engine.set_max_operations(0);
        register_vec3(&mut engine);
        register_api(&mut engine, world);
        ScriptHost { engine, ast: None, scope: Scope::new(), broken: false }
    }

    /// Compile-check a script without running it. Returns the error text on failure.
    pub fn check(src: &str) -> Result<(), String> {
        let mut engine = Engine::new();
        register_vec3(&mut engine);
        let w: Shared = Rc::new(RefCell::new(World::from_spec(&crate::spec::GameSpec::empty())));
        register_api(&mut engine, w);
        engine.compile(src).map(|_| ()).map_err(|e| e.to_string())
    }

    /// Compile and run top-level statements plus `init()`.
    pub fn load(&mut self, src: &str) -> Result<(), String> {
        self.broken = false;
        self.scope = Scope::new();
        let ast = self.engine.compile(src).map_err(|e| format!("compile error: {e}"))?;
        self.engine.run_ast_with_scope(&mut self.scope, &ast).map_err(|e| format!("script error: {e}"))?;
        self.ast = Some(ast);
        self.call("init", ())
    }

    fn call<A: rhai::FuncArgs>(&mut self, name: &str, args: A) -> Result<(), String> {
        let Some(ast) = &self.ast else { return Ok(()) };
        let opts = rhai::CallFnOptions::new().eval_ast(false).rewind_scope(true);
        match self.engine.call_fn_with_options::<Dynamic>(opts, &mut self.scope, ast, name, args) {
            Ok(_) => Ok(()),
            Err(e) => match *e {
                EvalAltResult::ErrorFunctionNotFound(ref f, _) if f.starts_with(name) => Ok(()),
                _ => Err(format!("in {name}(): {e}")),
            },
        }
    }

    pub fn update(&mut self, dt: f32) -> Result<(), String> {
        if self.broken {
            return Ok(());
        }
        let r = self.call("update", (dt,));
        if r.is_err() {
            self.broken = true;
        }
        r
    }

    pub fn on_collision(&mut self, a: &str, b: &str) -> Result<(), String> {
        if self.broken {
            return Ok(());
        }
        let r = self.call("on_collision", (a.to_string(), b.to_string()));
        if r.is_err() {
            self.broken = true;
        }
        r
    }
}

fn register_vec3(engine: &mut Engine) {
    engine
        .register_type_with_name::<Vec3>("Vec3")
        .register_fn("vec3", |x: Dynamic, y: Dynamic, z: Dynamic| Vec3::new(num(&x), num(&y), num(&z)))
        .register_fn("vec2", |x: Dynamic, y: Dynamic| Vec3::new(num(&x), num(&y), 0.0))
        .register_get_set("x", |v: &mut Vec3| v.x, |v: &mut Vec3, x: f32| v.x = x)
        .register_set("x", |v: &mut Vec3, x: i64| v.x = x as f32)
        .register_get_set("y", |v: &mut Vec3| v.y, |v: &mut Vec3, y: f32| v.y = y)
        .register_set("y", |v: &mut Vec3, y: i64| v.y = y as f32)
        .register_get_set("z", |v: &mut Vec3| v.z, |v: &mut Vec3, z: f32| v.z = z)
        .register_set("z", |v: &mut Vec3, z: i64| v.z = z as f32)
        .register_fn("+", |a: Vec3, b: Vec3| a + b)
        .register_fn("-", |a: Vec3, b: Vec3| a - b)
        .register_fn("-", |a: Vec3| -a)
        .register_fn("*", |a: Vec3, b: Dynamic| a * num(&b))
        .register_fn("*", |a: Dynamic, b: Vec3| b * num(&a))
        .register_fn("/", |a: Vec3, b: Dynamic| a / num(&b))
        .register_fn("==", |a: Vec3, b: Vec3| a == b)
        .register_fn("length", |a: Vec3| a.length())
        .register_fn("normalize", |a: Vec3| a.normalize_or_zero())
        .register_fn("dot", |a: Vec3, b: Vec3| a.dot(b))
        .register_fn("lerp", |a: Vec3, b: Vec3, t: Dynamic| a.lerp(b, num(&t)))
        .register_fn("to_string", |a: Vec3| format!("({:.2}, {:.2}, {:.2})", a.x, a.y, a.z))
        .register_fn("to_debug", |a: Vec3| format!("({:.2}, {:.2}, {:.2})", a.x, a.y, a.z));
}

fn register_api(engine: &mut Engine, world: Shared) {
    macro_rules! w {
        () => {
            world.clone()
        };
    }

    // ---- input ----
    let ww = w!();
    engine.register_fn("key", move |k: &str| ww.borrow().input.down.contains(&k.to_ascii_lowercase()));
    let ww = w!();
    engine.register_fn("key_pressed", move |k: &str| ww.borrow().input.pressed.contains(&k.to_ascii_lowercase()));
    let ww = w!();
    engine.register_fn("key_released", move |k: &str| ww.borrow().input.released.contains(&k.to_ascii_lowercase()));
    let ww = w!();
    engine.register_fn("mouse_down", move || ww.borrow().input.mouse_down);
    let ww = w!();
    engine.register_fn("mouse_pressed", move || ww.borrow().input.mouse_pressed);
    let ww = w!();
    engine.register_fn("mouse_pos", move || ww.borrow().mouse_world());
    // Horizontal axis: -1 (a/left) .. 1 (d/right).
    let ww = w!();
    engine.register_fn("axis_x", move || {
        let i = &ww.borrow().input.down;
        let mut v = 0.0f32;
        if i.contains("a") || i.contains("left") { v -= 1.0 }
        if i.contains("d") || i.contains("right") { v += 1.0 }
        v
    });
    let ww = w!();
    engine.register_fn("axis_y", move || {
        let i = &ww.borrow().input.down;
        let mut v = 0.0f32;
        if i.contains("s") || i.contains("down") { v -= 1.0 }
        if i.contains("w") || i.contains("up") { v += 1.0 }
        v
    });

    // ---- time / util ----
    let ww = w!();
    engine.register_fn("time", move || ww.borrow().time);
    let ww = w!();
    engine.register_fn("delta", move || ww.borrow().dt);
    engine.register_fn("rand", |a: Dynamic, b: Dynamic| {
        let (a, b) = (num(&a), num(&b));
        if b <= a { a } else { a + rand::random::<f32>() * (b - a) }
    });
    engine.register_fn("rand_int", |a: i64, b: i64| if b < a { a } else { a + (rand::random::<u64>() % ((b - a + 1) as u64)) as i64 });
    engine.register_fn("clamp", |v: Dynamic, lo: Dynamic, hi: Dynamic| num(&v).clamp(num(&lo), num(&hi)));
    engine.register_fn("abs", |v: Dynamic| num(&v).abs());
    engine.register_fn("sqrt", |v: Dynamic| num(&v).sqrt());
    engine.register_fn("sin", |v: Dynamic| num(&v).sin());
    engine.register_fn("cos", |v: Dynamic| num(&v).cos());
    engine.register_fn("atan2", |y: Dynamic, x: Dynamic| num(&y).atan2(num(&x)));
    engine.register_fn("floor", |v: Dynamic| num(&v).floor() as i64);
    engine.register_fn("round", |v: Dynamic| num(&v).round() as i64);
    engine.register_fn("min", |a: Dynamic, b: Dynamic| num(&a).min(num(&b)));
    engine.register_fn("max", |a: Dynamic, b: Dynamic| num(&a).max(num(&b)));
    engine.register_fn("to_float", |v: Dynamic| num(&v));
    engine.register_fn("log", |s: &str| log::info!("[script] {s}"));
    engine.register_fn("print", |s: &str| println!("[script] {s}"));

    // ---- state store ----
    let ww = w!();
    engine.register_fn("set", move |k: &str, v: Dynamic| {
        ww.borrow_mut().vars.insert(k.to_string(), v);
    });
    let ww = w!();
    engine.register_fn("get", move |k: &str| ww.borrow().vars.get(k).cloned().unwrap_or(Dynamic::UNIT));
    let ww = w!();
    engine.register_fn("get_or", move |k: &str, d: Dynamic| ww.borrow().vars.get(k).cloned().unwrap_or(d));
    let ww = w!();
    engine.register_fn("add", move |k: &str, d: Dynamic| {
        let mut w = ww.borrow_mut();
        let cur = w.vars.get(k).cloned().unwrap_or(Dynamic::from(0i64));
        let nv: Dynamic = if cur.is::<i64>() && d.is::<i64>() {
            Dynamic::from(cur.as_int().unwrap() + d.as_int().unwrap())
        } else {
            Dynamic::from(num(&cur) + num(&d))
        };
        w.vars.insert(k.to_string(), nv.clone());
        nv
    });

    // ---- entity queries ----
    let ww = w!();
    engine.register_fn("exists", move |n: &str| ww.borrow().entity(n).is_some());
    let ww = w!();
    engine.register_fn("pos", move |n: &str| -> Result<Vec3, Box<EvalAltResult>> {
        let w = ww.borrow();
        let e = w.entity(n).ok_or_else(|| err(format!("no entity '{n}'")))?;
        Ok(w.ecs.get::<&Transform>(e).map(|t| t.pos).unwrap_or_default())
    });
    let ww = w!();
    engine.register_fn("vel", move |n: &str| {
        let w = ww.borrow();
        w.entity(n).and_then(|e| w.ecs.get::<&Body>(e).ok().map(|b| b.vel)).unwrap_or_default()
    });
    let ww = w!();
    engine.register_fn("scale_of", move |n: &str| {
        let w = ww.borrow();
        w.entity(n).and_then(|e| w.ecs.get::<&Transform>(e).ok().map(|t| t.scale)).unwrap_or(Vec3::ONE)
    });
    let ww = w!();
    engine.register_fn("rotation_of", move |n: &str| {
        let w = ww.borrow();
        w.entity(n).and_then(|e| w.ecs.get::<&Transform>(e).ok().map(|t| t.rot)).unwrap_or_default()
    });
    let ww = w!();
    engine.register_fn("grounded", move |n: &str| {
        let w = ww.borrow();
        w.entity(n).and_then(|e| w.ecs.get::<&Body>(e).ok().map(|b| b.grounded)).unwrap_or(false)
    });
    let ww = w!();
    engine.register_fn("has_tag", move |n: &str, tag: &str| {
        let w = ww.borrow();
        w.entity(n).and_then(|e| w.ecs.get::<&Tags>(e).ok().map(|t| t.0.iter().any(|x| x == tag))).unwrap_or(false)
    });
    let ww = w!();
    engine.register_fn("find_tag", move |tag: &str| -> Array {
        let w = ww.borrow();
        let mut out: Vec<(String, Dynamic)> = w
            .ecs
            .query::<(&crate::world::Name, &Tags)>()
            .iter()
            .filter(|(_, (_, t))| t.0.iter().any(|x| x == tag))
            .map(|(_, (n, _))| (n.0.clone(), Dynamic::from(n.0.clone())))
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out.into_iter().map(|(_, d)| d).collect()
    });
    let ww = w!();
    engine.register_fn("count_tag", move |tag: &str| -> i64 {
        let w = ww.borrow();
        let c = w.ecs.query::<&Tags>().iter().filter(|(_, t)| t.0.iter().any(|x| x == tag)).count();
        c as i64
    });
    let ww = w!();
    engine.register_fn("distance", move |a: &str, b: &str| {
        let w = ww.borrow();
        let p = |n: &str| w.entity(n).and_then(|e| w.ecs.get::<&Transform>(e).ok().map(|t| t.pos));
        match (p(a), p(b)) {
            (Some(a), Some(b)) => a.distance(b),
            _ => f32::INFINITY,
        }
    });
    let ww = w!();
    engine.register_fn("nearest_tag", move |from: &str, tag: &str| -> Dynamic {
        let w = ww.borrow();
        let Some(origin) = w.entity(from).and_then(|e| w.ecs.get::<&Transform>(e).ok().map(|t| t.pos)) else {
            return Dynamic::UNIT;
        };
        let mut best: Option<(f32, String)> = None;
        for (_, (n, t, tr)) in w.ecs.query::<(&crate::world::Name, &Tags, &Transform)>().iter() {
            if n.0 == from || !t.0.iter().any(|x| x == tag) {
                continue;
            }
            let d = tr.pos.distance(origin);
            if best.as_ref().map_or(true, |(bd, _)| d < *bd) {
                best = Some((d, n.0.clone()));
            }
        }
        best.map(|(_, n)| Dynamic::from(n)).unwrap_or(Dynamic::UNIT)
    });

    // ---- entity mutation ----
    macro_rules! with_entity {
        ($ww:expr, $n:expr, $comp:ty, $f:expr) => {{
            let w = $ww.borrow_mut();
            if let Some(e) = w.entity($n) {
                if let Ok(c) = w.ecs.get::<&mut $comp>(e) {
                    let mut c = c;
                    let f: &dyn Fn(&mut $comp) = &$f;
                    f(&mut c);
                }
            }
        }};
    }
    let ww = w!();
    engine.register_fn("set_pos", move |n: &str, p: Dynamic| with_entity!(ww, n, Transform, |t| t.pos = to_vec3(&p)));
    let ww = w!();
    engine.register_fn("move_by", move |n: &str, d: Dynamic| with_entity!(ww, n, Transform, |t| t.pos += to_vec3(&d)));
    let ww = w!();
    engine.register_fn("set_vel", move |n: &str, v: Dynamic| with_entity!(ww, n, Body, |b| b.vel = to_vec3(&v)));
    let ww = w!();
    engine.register_fn("add_vel", move |n: &str, v: Dynamic| with_entity!(ww, n, Body, |b| b.vel += to_vec3(&v)));
    let ww = w!();
    engine.register_fn("set_vel_x", move |n: &str, v: Dynamic| with_entity!(ww, n, Body, |b| b.vel.x = num(&v)));
    let ww = w!();
    engine.register_fn("set_vel_y", move |n: &str, v: Dynamic| with_entity!(ww, n, Body, |b| b.vel.y = num(&v)));
    let ww = w!();
    engine.register_fn("set_vel_z", move |n: &str, v: Dynamic| with_entity!(ww, n, Body, |b| b.vel.z = num(&v)));
    let ww = w!();
    engine.register_fn("set_scale", move |n: &str, s: Dynamic| with_entity!(ww, n, Transform, |t| t.scale = to_vec3(&s)));
    let ww = w!();
    engine.register_fn("set_rotation", move |n: &str, r: Dynamic| with_entity!(ww, n, Transform, |t| t.rot = to_vec3(&r)));
    let ww = w!();
    engine.register_fn("rotate", move |n: &str, r: Dynamic| with_entity!(ww, n, Transform, |t| t.rot += to_vec3(&r)));
    let ww = w!();
    engine.register_fn("set_color", move |n: &str, c: Dynamic| with_entity!(ww, n, Visual, |v| v.color = to_vec3(&c)));
    let ww = w!();
    engine.register_fn("set_body", move |n: &str, k: &str| with_entity!(ww, n, Body, |b| b.kind = body_of(k)));
    let ww = w!();
    engine.register_fn("set_solid", move |n: &str, s: bool| with_entity!(ww, n, Body, |b| b.solid = s));
    let ww = w!();
    engine.register_fn("add_tag", move |n: &str, tag: &str| {
        let tag = tag.to_string();
        with_entity!(ww, n, Tags, |t| if !t.0.contains(&tag) { t.0.push(tag.clone()) })
    });
    let ww = w!();
    engine.register_fn("remove_tag", move |n: &str, tag: &str| with_entity!(ww, n, Tags, |t| t.0.retain(|x| x != tag)));
    let ww = w!();
    engine.register_fn("look_at_2d", move |n: &str, target: Dynamic| {
        let target = to_vec3(&target);
        let w = ww.borrow_mut();
        if let Some(e) = w.entity(n) {
            if let Ok(mut t) = w.ecs.get::<&mut Transform>(e) {
                let d = target - t.pos;
                t.rot.z = d.y.atan2(d.x).to_degrees();
            }
        }
    });

    // ---- spawn / destroy ----
    let ww = w!();
    engine.register_fn("spawn", move |name: &str, shape: &str, pos: Dynamic, scale: Dynamic, color: Dynamic, body: &str| -> String {
        let spec = EntitySpec {
            name: name.to_string(),
            tags: vec![],
            shape: shape_of(shape),
            sprite: None,
            position: to_vec3(&pos).into(),
            rotation: [0.0; 3],
            scale: to_vec3(&scale).into(),
            color: to_vec3(&color).into(),
            physics: PhysicsSpec { body: body_of(body), ..Default::default() },
        };
        let mut w = ww.borrow_mut();
        let e = w.spawn_spec(&spec);
        w.name_of(e)
    });
    let ww = w!();
    engine.register_fn("spawn_tagged", move |name: &str, tags: Array, shape: &str, pos: Dynamic, scale: Dynamic, color: Dynamic, body: &str| -> String {
        let spec = EntitySpec {
            name: name.to_string(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            shape: shape_of(shape),
            sprite: None,
            position: to_vec3(&pos).into(),
            rotation: [0.0; 3],
            scale: to_vec3(&scale).into(),
            color: to_vec3(&color).into(),
            physics: PhysicsSpec { body: body_of(body), ..Default::default() },
        };
        let mut w = ww.borrow_mut();
        let e = w.spawn_spec(&spec);
        w.name_of(e)
    });
    let ww = w!();
    engine.register_fn("spawn_sprite", move |name: &str, tags: Array, sprite: &str, pos: Dynamic, scale: Dynamic, body: &str| -> String {
        let spec = EntitySpec {
            name: name.to_string(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            shape: Shape::Plane,
            sprite: Some(sprite.to_string()),
            position: to_vec3(&pos).into(),
            rotation: [0.0; 3],
            scale: to_vec3(&scale).into(),
            color: [1.0; 3],
            physics: PhysicsSpec { body: body_of(body), ..Default::default() },
        };
        let mut w = ww.borrow_mut();
        let e = w.spawn_spec(&spec);
        w.name_of(e)
    });
    let ww = w!();
    engine.register_fn("set_sprite", move |n: &str, sprite: &str| {
        let w = ww.borrow_mut();
        let idx = w.sprite_index(sprite);
        if let Some(e) = w.entity(n) {
            if let Ok(mut v) = w.ecs.get::<&mut Visual>(e) {
                v.sprite = idx;
                if idx.is_some() {
                    v.shape = Shape::Plane;
                }
            }
        }
    });
    let ww = w!();
    engine.register_fn("set_flip_x", move |n: &str, f: bool| with_entity!(ww, n, Visual, |v| v.flip_x = f));
    let ww = w!();
    engine.register_fn("destroy", move |n: &str| ww.borrow_mut().despawn(n));
    let ww = w!();
    engine.register_fn("destroy_tag", move |tag: &str| -> i64 {
        let mut w = ww.borrow_mut();
        let names: Vec<String> = w
            .ecs
            .query::<(&crate::world::Name, &Tags)>()
            .iter()
            .filter(|(_, (_, t))| t.0.iter().any(|x| x == tag))
            .map(|(_, (n, _))| n.0.clone())
            .collect();
        let c = names.len() as i64;
        for n in names {
            w.despawn(&n);
        }
        c
    });

    // ---- ui / camera / game ----
    let ww = w!();
    engine.register_fn("text", move |id: &str, s: Dynamic| ww.borrow_mut().set_text(id, &s.to_string()));
    let ww = w!();
    engine.register_fn("text_at", move |id: &str, s: Dynamic, x: Dynamic, y: Dynamic, size: Dynamic| {
        let mut w = ww.borrow_mut();
        w.set_text(id, &s.to_string());
        if let Some(t) = w.texts.iter_mut().find(|t| t.id == id) {
            t.x = num(&x);
            t.y = num(&y);
            t.size = num(&size);
        }
    });
    let ww = w!();
    engine.register_fn("text_color", move |id: &str, c: Dynamic| {
        let mut w = ww.borrow_mut();
        if let Some(t) = w.texts.iter_mut().find(|t| t.id == id) {
            t.color = to_vec3(&c).into();
        }
    });
    let ww = w!();
    engine.register_fn("remove_text", move |id: &str| ww.borrow_mut().texts.retain(|t| t.id != id));
    let ww = w!();
    engine.register_fn("camera_set", move |pos: Dynamic, look: Dynamic| {
        let mut w = ww.borrow_mut();
        w.camera.position = to_vec3(&pos);
        w.camera.look_at = to_vec3(&look);
        w.camera.follow_offset = w.camera.position - w.camera.look_at;
    });
    let ww = w!();
    engine.register_fn("camera_follow", move |n: &str| {
        let mut w = ww.borrow_mut();
        w.camera.follow = if n.is_empty() { None } else { Some(n.to_string()) };
    });
    let ww = w!();
    engine.register_fn("camera_zoom", move |z: Dynamic| ww.borrow_mut().camera.zoom = num(&z).max(0.1));
    let ww = w!();
    engine.register_fn("camera_pos", move || ww.borrow().camera.position);
    let ww = w!();
    engine.register_fn("set_gravity", move |g: Dynamic| ww.borrow_mut().gravity = to_vec3(&g));
    let ww = w!();
    engine.register_fn("set_background", move |c: Dynamic| ww.borrow_mut().background = to_vec3(&c));
    let ww = w!();
    engine.register_fn("restart", move || ww.borrow_mut().restart_requested = true);
}

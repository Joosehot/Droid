//! Level geometry: where the ground, platforms, walls, enemies, pickups, hazards and the exit
//! go. Driven by the design's numbers (rules.toml) and a PRNG seeded from the sentence and the
//! chosen variants, so the same sentence always builds the same level. The builder also keeps
//! what the checks need: walkable surfaces (platformer), solid blocks (arena), the spawn point.

use super::config::Config;
use super::design::{Design, Why};
use super::paint::{ArtPlan, Pick};
use super::request::*;
use super::rng::Rng;
use crate::spec::{BodyKind, CameraSpec, EntitySpec, PhysicsSpec, Shape, TextSpec, View};
use std::collections::BTreeMap;

/// A walkable top (platformer): from x0 to x1 at height `top`.
#[derive(Clone, Debug, PartialEq)]
pub struct Surface {
    pub x0: f32,
    pub x1: f32,
    pub top: f32,
    /// A pit floor with hazards on it: never part of a path.
    pub hazard: bool,
}

/// An axis-aligned rectangle on the play plane (x, y) in 2D or (x, z) in 3D.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl Rect {
    pub fn centered(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x0: x - w / 2.0, y0: y - h / 2.0, x1: x + w / 2.0, y1: y + h / 2.0 }
    }
    pub fn grow(self, r: f32) -> Rect {
        Rect { x0: self.x0 - r, y0: self.y0 - r, x1: self.x1 + r, y1: self.y1 + r }
    }
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x0 && x <= self.x1 && y >= self.y0 && y <= self.y1
    }
}

#[derive(Clone, Debug)]
pub struct Level {
    pub view: View,
    pub gravity: [f32; 3],
    pub background: [f32; 3],
    pub camera: CameraSpec,
    pub entities: Vec<EntitySpec>,
    pub texts: Vec<TextSpec>,
    /// The player's start (entity centre).
    pub spawn: [f32; 3],
    pub player_size: [f32; 3],
    /// Platformer: walkable tops, for the jump-graph check.
    pub surfaces: Vec<Surface>,
    /// Arena: solid blocks on the play plane, for the flood fill.
    pub blocks: Vec<Rect>,
    /// Arena: the inside of the walls.
    pub bounds: Rect,
    /// Platformer: falling below this loses a life.
    pub kill_y: f32,
    /// Patrol distance per enemy kind (what fits the ground it stands on).
    pub patrol: BTreeMap<EnemyKind, f32>,
    /// Shooter: the player moves within +-this.
    pub move_limit: f32,
    pub text_color: [f32; 3],
    pub why: Vec<Why>,
}

/// Unique entity names: slime_1, slime_2, ...
#[derive(Default)]
struct Namer(BTreeMap<String, u32>);

impl Namer {
    fn next(&mut self, base: &str) -> String {
        let n = self.0.entry(base.to_string()).or_insert(0);
        *n += 1;
        format!("{base}_{n}")
    }
}

struct E(EntitySpec);

impl E {
    fn new(name: impl Into<String>, pos: [f32; 3], scale: [f32; 3]) -> E {
        E(EntitySpec {
            name: name.into(),
            tags: vec![],
            shape: Shape::Box,
            sprite: None,
            model: None,
            position: pos.map(round3),
            rotation: [0.0; 3],
            scale: scale.map(round3),
            color: [1.0; 3],
            physics: PhysicsSpec::default(),
        })
    }
    fn tags(mut self, t: &[&str]) -> E {
        self.0.tags = t.iter().map(|s| s.to_string()).collect();
        self
    }
    fn sprite(mut self, s: &str) -> E {
        self.0.sprite = Some(s.to_string());
        self
    }
    fn art(mut self, p: &Pick) -> E {
        if p.model {
            self.0.model = Some(p.name.clone());
        } else {
            self.0.sprite = Some(p.name.clone());
        }
        self.0.color = p.color.map(round3);
        self
    }
    fn color(mut self, c: [f32; 3]) -> E {
        self.0.color = c.map(round3);
        self
    }
    fn shape(mut self, s: Shape) -> E {
        self.0.shape = s;
        self
    }
    fn body(mut self, b: BodyKind, solid: bool) -> E {
        self.0.physics = PhysicsSpec { body: b, velocity: [0.0; 3], bounciness: 0.0, solid };
        self
    }
}

/// Round to 3 decimals so files are short and identical everywhere.
pub fn round3(x: f32) -> f32 {
    let r = (x * 1000.0).round() / 1000.0;
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

/// Theme colours with the palette applied: (background, ground, wall, decor, text, art shade).
pub struct Colors {
    pub background: [f32; 3],
    pub ground: [f32; 3],
    pub wall: [f32; 3],
    pub decor: [f32; 3],
    pub text: [f32; 3],
    /// Multiplier for tiles and decor art.
    pub shade: [f32; 3],
}

pub fn colors(d: &Design, cfg: &Config) -> Colors {
    let th = cfg.themes[d.theme.name()];
    let p = &cfg.palette;
    let grey = cfg.colors["grey"];
    let map = |c: [f32; 3]| -> [f32; 3] {
        match d.variant("palette") {
            "dark" => c.map(|x| x * (p.dark_factor + 0.4)),
            "muted" => [0, 1, 2].map(|i| c[i] + (grey[i] - c[i]) * p.muted_mix),
            _ => c,
        }
    };
    let background = match d.variant("palette") {
        "dark" => th.background.map(|x| x * p.dark_factor),
        _ => map(th.background),
    };
    Colors { background, ground: map(th.ground), wall: map(th.wall), decor: map(th.decor), text: th.text, shade: map([1.0; 3]) }
}

fn sized(p: &Pick, h: f32) -> [f32; 3] {
    let w = h * p.aspect.clamp(0.4, 2.5);
    [w, h, if p.model { (h * p.depth_ratio).max(0.3) } else { 1.0 }]
}

pub fn build(d: &Design, cfg: &Config, art: &ArtPlan) -> Result<Level, String> {
    let mut rng = Rng::new(d.seed);
    let col = colors(d, cfg);
    let mut lv = Level {
        view: if d.depth { View::ThreeD } else { View::TwoD },
        gravity: [0.0; 3],
        background: col.background.map(round3),
        camera: CameraSpec::default(),
        entities: vec![],
        texts: vec![],
        spawn: [0.0; 3],
        player_size: [1.0; 3],
        surfaces: vec![],
        blocks: vec![],
        bounds: Rect { x0: 0.0, y0: 0.0, x1: 0.0, y1: 0.0 },
        kill_y: cfg.layout.platformer.kill_y,
        patrol: BTreeMap::new(),
        move_limit: 0.0,
        text_color: col.text,
        why: vec![],
    };
    match d.genre {
        Genre::Platformer => platformer(d, cfg, art, &col, &mut rng, &mut lv)?,
        Genre::Arena => arena(d, cfg, art, &col, &mut rng, &mut lv)?,
        Genre::Shooter => shooter(d, cfg, art, &col, &mut rng, &mut lv)?,
    }
    let h = &cfg.hud;
    let help = match d.genre {
        Genre::Platformer => format!("ARROWS/AD MOVE  SPACE JUMP{}  R RESTART", if d.player_shoots { "  X SHOOT" } else { "" }),
        Genre::Arena => format!("ARROWS/WASD MOVE{}{}  R RESTART", if d.can_jump { "  SPACE JUMP" } else { "" }, if d.player_shoots { if d.can_jump { "  X SHOOT" } else { "  SPACE SHOOT" } } else { "" }),
        Genre::Shooter => "ARROWS/AD MOVE  SPACE FIRE  R RESTART".to_string(),
    };
    lv.texts.push(TextSpec { id: "help".into(), text: help, x: h.help[0], y: h.help[1], size: h.help[2], color: col.text.map(|c| round3(c * h.help_dim)) });
    Ok(lv)
}

// ---------------------------------------------------------------- platformer

/// Jump physics: (apex height, horizontal reach on flat ground).
pub fn jump_reach(d: &Design) -> (f32, f32) {
    let h = d.jump * d.jump / (2.0 * d.gravity);
    let air = 2.0 * d.jump / d.gravity;
    (h, d.speed * air)
}

/// Horizontal distance covered by a jump that lands `dh` higher (negative = lower).
pub fn reach_at(d: &Design, dh: f32) -> f32 {
    let disc = d.jump * d.jump - 2.0 * d.gravity * dh;
    if disc < 0.0 {
        return 0.0;
    }
    d.speed * (d.jump + disc.sqrt()) / d.gravity
}

fn platformer(d: &Design, cfg: &Config, art: &ArtPlan, col: &Colors, rng: &mut Rng, lv: &mut Level) -> Result<(), String> {
    let lay = &cfg.layout.platformer;
    let tile = cfg.art.tile_size;
    let tiles = art.tiles.as_ref().ok_or("platformer without tiles")?;
    let qf = |x: f32| ((x / tile).floor().max(1.0)) * tile;
    let qr = |x: f32| ((x / tile).round().max(1.0)) * tile;
    let half = |x: f32| (x * 2.0).round() / 2.0;
    let (jump_h, reach) = jump_reach(d);
    let gap_max = (lay.gap_fraction * reach).max(lay.gap_min);
    let rise_max = lay.rise_fraction * jump_h;
    let length = lay.length * d.size;
    let layout = d.variant("layout");
    lv.gravity = [0.0, -round3(d.gravity), 0.0];
    lv.kill_y = lay.kill_y;

    // ground segments and the gaps between them
    let mut segs: Vec<Surface> = vec![Surface { x0: 0.0, x1: qr(lay.first_segment), top: 0.0, hazard: false }];
    let mut floating: Vec<Surface> = vec![];
    let mut wide = false;
    while segs.last().unwrap().x1 < length {
        let prev = segs.last().unwrap().clone();
        wide = layout == "stacked" && !wide;
        let gap = if wide { qf(rng.range(gap_max * 1.4, gap_max * 2.0)) } else { qf(rng.range(lay.gap_min, gap_max)) };
        let top = match layout {
            "steps" => {
                let up = if prev.top <= 0.0 { true } else if prev.top >= lay.max_height { false } else { rng.chance(0.6) };
                let dh = half(rng.range(lay.step_min, rise_max)).max(0.5).min(half(rise_max - 0.25).max(0.5));
                (prev.top + if up { dh } else { -dh }).clamp(0.0, lay.max_height)
            }
            "stacked" => (prev.top + half(rng.range(-0.5, 0.5))).clamp(0.0, lay.max_height),
            _ => 0.0,
        };
        let len = qr(rng.range(lay.segment_min, lay.segment_max));
        let x0 = prev.x1 + gap;
        if wide {
            // a floating platform halfway, so the wide gap is two jumps
            let pw = qr(rng.range(lay.platform_width[0], lay.platform_width[1])).max(gap - 2.0 * (gap_max - tile)).min(gap - 2.0 * tile);
            let ptop = prev.top.max(top) + half(rng.range(0.5, rise_max * 0.8)).min(half(rise_max - 0.25));
            let cx = prev.x1 + gap / 2.0;
            floating.push(Surface { x0: cx - pw / 2.0, x1: cx + pw / 2.0, top: ptop, hazard: false });
        }
        segs.push(Surface { x0, x1: x0 + len, top, hazard: false });
    }
    // the last segment holds the exit
    let last = segs.len() - 1;
    segs[last].x1 = segs[last].x1.max(segs[last].x0 + qr(lay.exit_margin * 2.0 + 2.0));
    let end_x = segs[last].x1;

    let player = art.get("player");
    let ph = cfg.art.player_height;
    lv.player_size = sized(player, ph);
    lv.spawn = [lay.spawn_x, segs[0].top + ph / 2.0 + 0.05, 0.0].map(round3);
    let spawn_x = lv.spawn[0];
    let mut names = Namer::default();
    let mut ents: Vec<EntitySpec> = vec![];

    // pits with hazards (hazards "in_gaps")
    let mut hazards_left: Vec<(HazardKind, u32)> = d.hazards.iter().map(|h| (h.kind, h.count)).collect();
    let mut pits: Vec<Surface> = vec![];
    if d.is("hazards", "in_gaps") && !hazards_left.is_empty() {
        let gaps: Vec<(f32, f32, f32)> = segs.windows(2).map(|w| (w[0].x1, w[1].x0, w[0].top.min(w[1].top))).collect();
        let want: u32 = hazards_left.iter().map(|h| h.1).sum();
        let n = (want as usize).min(gaps.len());
        let mut hk = 0;
        for i in 0..n {
            let g = gaps[(i * gaps.len() + gaps.len() / 2) / n.max(1) % gaps.len()];
            if pits.iter().any(|p| p.x0 == g.0) {
                continue;
            }
            while hk < hazards_left.len() && hazards_left[hk].1 == 0 {
                hk += 1;
            }
            let Some((kind, left)) = hazards_left.get_mut(hk) else { break };
            *left -= 1;
            let kind = *kind;
            let floor = g.2 - lay.pit_depth;
            pits.push(Surface { x0: g.0, x1: g.1, top: floor, hazard: true });
            let hz = art.get(&format!("hazard:{}", kind.name()));
            let size = cfg.hazards[kind.name()].size;
            let w = g.1 - g.0;
            match kind {
                HazardKind::Spike => {
                    let k = (w / size[0]).floor().max(1.0) as usize;
                    for j in 0..k {
                        let x = g.0 + (w - k as f32 * size[0]) / 2.0 + (j as f32 + 0.5) * size[0];
                        ents.push(E::new(names.next(kind.name()), [x, floor + size[1] / 2.0, 0.0], [size[0], size[1], 1.0]).tags(&[kind.name(), "hazard"]).art(hz).body(BodyKind::Static, false).0);
                    }
                }
                HazardKind::Lava => {
                    ents.push(E::new(names.next(kind.name()), [(g.0 + g.1) / 2.0, floor + size[1] / 2.0, 0.0], [w, size[1], 1.0]).tags(&[kind.name(), "hazard"]).art(hz).body(BodyKind::Static, false).0);
                }
                HazardKind::Saw => {
                    ents.push(E::new(names.next(kind.name()), [(g.0 + g.1) / 2.0, floor + size[1] / 2.0, 0.0], [size[0].min(w), size[1], 1.0]).tags(&[kind.name(), "hazard"]).art(hz).body(BodyKind::Static, false).0);
                }
            }
        }
        if hazards_left.iter().any(|h| h.1 > 0) {
            lv.why.push(Why { piece: format!("{} hazards did not fit in the gaps; they stand on the floor", hazards_left.iter().map(|h| h.1).sum::<u32>()), rule: d.rv("hazards"), words: String::new() });
        }
    }

    // ground and platforms
    for s in segs.iter().chain(pits.iter()) {
        let h = s.top - lay.ground_bottom;
        ents.push(E::new(names.next("ground"), [(s.x0 + s.x1) / 2.0, (s.top + lay.ground_bottom) / 2.0, 0.0], [s.x1 - s.x0, h, 1.0]).tags(&["ground"]).sprite(&tiles.ground).color(col.shade).body(BodyKind::Static, true).0);
    }
    for s in &floating {
        let t = lay.platform_thickness;
        ents.push(E::new(names.next("platform"), [(s.x0 + s.x1) / 2.0, s.top - t / 2.0, 0.0], [s.x1 - s.x0, t, 1.0]).tags(&["platform"]).sprite(&tiles.platform).color(col.shade).body(BodyKind::Static, true).0);
    }

    // x ranges already used on the ground (hazards, enemies, the exit)
    let exit_x = end_x - lay.exit_margin;
    let mut used: Vec<(f32, f32)> = vec![(exit_x - 2.0, exit_x + 2.0), (spawn_x - cfg.checks.spawn_clear, spawn_x + cfg.checks.spawn_clear)];
    let free = |used: &Vec<(f32, f32)>, a: f32, b: f32| used.iter().all(|u| b < u.0 || a > u.1);

    // hazards on the floor
    for (kind, count) in hazards_left.iter().filter(|h| h.1 > 0) {
        let size = cfg.hazards[kind.name()].size;
        let hz = art.get(&format!("hazard:{}", kind.name()));
        let mut slots = vec![];
        for s in &segs {
            let mut x = s.x0 + lay.hazard_margin + size[0] / 2.0;
            while x + size[0] / 2.0 <= s.x1 - lay.hazard_margin {
                slots.push((x, s.top));
                x += size[0] + lay.hazard_margin;
            }
        }
        slots.retain(|(x, _)| free(&used, x - size[0] / 2.0, x + size[0] / 2.0));
        for (x, top) in spread(&slots, *count as usize) {
            used.push((x - size[0] / 2.0 - 0.5, x + size[0] / 2.0 + 0.5));
            ents.push(E::new(names.next(kind.name()), [x, top + size[1] / 2.0, 0.0], [size[0], size[1], 1.0]).tags(&[kind.name(), "hazard"]).art(hz).body(BodyKind::Static, false).0);
        }
    }

    // enemies
    for e in &d.enemies {
        let pick = art.get(&format!("enemy:{}", e.kind.name()));
        let size = cfg.enemies[e.kind.name()].size;
        let sc = sized(pick, size[1]);
        let want = cfg.enemy_rules.patrol_distance;
        if e.flying {
            // over gaps and ground, hovering
            let mut slots: Vec<(f32, f32)> = segs.windows(2).map(|w| ((w[0].x1 + w[1].x0) / 2.0, w[0].top.max(w[1].top))).collect();
            slots.extend(segs.iter().map(|s| ((s.x0 + s.x1) / 2.0, s.top)));
            slots.sort_by(|a, b| a.0.total_cmp(&b.0));
            slots.retain(|(x, _)| (x - spawn_x).abs() > cfg.checks.spawn_clear + sc[0] && (x - exit_x).abs() > 2.0);
            for (x, top) in spread(&slots, e.count as usize) {
                let y = top + cfg.enemy_rules.fly_height;
                ents.push(E::new(names.next(e.kind.name()), [x, y, 0.0], sc).tags(&[e.kind.name(), "enemy"]).art(pick).body(BodyKind::Kinematic, false).0);
            }
        } else {
            // walkers: the middle of a ground segment, room to patrol
            let mut slots: Vec<(f32, f32, f32)> = vec![];
            for s in &segs {
                let len = s.x1 - s.x0;
                let parts = if len >= 4.0 * (want + sc[0]) { 2 } else { 1 };
                for k in 0..parts {
                    let a = s.x0 + len * k as f32 / parts as f32;
                    let b = s.x0 + len * (k + 1) as f32 / parts as f32;
                    let room = (b - a) / 2.0 - sc[0] / 2.0 - 0.5;
                    let x = (a + b) / 2.0;
                    if room >= 1.0 && free(&used, x - 0.5, x + 0.5) && (x - spawn_x).abs() > cfg.checks.spawn_clear + room.min(want) + sc[0] {
                        slots.push((x, s.top, room));
                    }
                }
            }
            if slots.is_empty() {
                return Err(format!("no ground has room for {}s to walk on", e.kind.name()));
            }
            let chosen = spread(&slots.iter().map(|s| (s.0, (s.1, s.2))).collect::<Vec<_>>(), e.count as usize);
            let d_fit = chosen.iter().map(|c| c.1 .1).fold(want, f32::min);
            lv.patrol.insert(e.kind, round3(d_fit));
            for (x, (top, _)) in chosen {
                used.push((x - 0.5, x + 0.5));
                ents.push(E::new(names.next(e.kind.name()), [x, top + sc[1] / 2.0 + 0.02, 0.0], sc).tags(&[e.kind.name(), "enemy"]).art(pick).body(BodyKind::Dynamic, true).0);
            }
        }
    }

    // pickups
    let hazard_xs: Vec<f32> = ents.iter().filter(|e| e.tags.iter().any(|t| t == "hazard")).map(|e| e.position[0]).collect();
    let walk: Vec<Surface> = segs.iter().chain(floating.iter()).cloned().collect();
    let mut cands: Vec<(f32, f32)> = vec![];
    let lift = |x: f32, y: f32| if hazard_xs.iter().any(|h| (h - x).abs() < 1.2) { y + jump_h * 0.6 } else { y };
    match d.variant("pickups") {
        "rewarding" => {
            for s in &floating {
                let mut x = s.x0 + 0.5;
                while x <= s.x1 - 0.5 {
                    cands.push((x, s.top + lay.coin_height));
                    x += lay.coin_spacing;
                }
            }
            for w in segs.windows(2) {
                let (a, b) = (w[0].x1, w[1].x0);
                let top = w[0].top.max(w[1].top);
                cands.push(((a + b) / 2.0, top + jump_h * 0.8));
            }
            for s in &segs {
                let x = (s.x0 + s.x1) / 2.0;
                cands.push((x, s.top + jump_h * 0.85));
                cands.push((x + lay.coin_spacing, s.top + lay.coin_height));
            }
        }
        "scattered" => {
            let total: f32 = walk.iter().map(|s| s.x1 - s.x0).sum();
            for _ in 0..(d.pickup_total() * 3).max(8) {
                let mut r = rng.range(0.0, total);
                for s in &walk {
                    let l = s.x1 - s.x0;
                    if r <= l {
                        let x = s.x0 + r.clamp(0.5, (l - 0.5).max(0.5));
                        cands.push((x, s.top + lay.coin_height + rng.range(0.0, jump_h * 0.7)));
                        break;
                    }
                    r -= l;
                }
            }
        }
        _ => {
            // a trail: rows along the ground, arcs over the gaps
            for s in &walk {
                let mut x = s.x0 + 1.0;
                while x <= s.x1 - 1.0 {
                    cands.push((x, s.top + lay.coin_height));
                    x += lay.coin_spacing;
                }
            }
            for w in segs.windows(2) {
                let (a, b) = (w[0].x1, w[1].x0);
                let top = w[0].top.max(w[1].top);
                let m = (a + b) / 2.0;
                cands.push((m - lay.coin_spacing * 0.8, top + jump_h * 0.5));
                cands.push((m, top + jump_h * 0.7));
                cands.push((m + lay.coin_spacing * 0.8, top + jump_h * 0.5));
            }
        }
    }
    cands.retain(|(x, _)| (x - spawn_x).abs() > 1.0 && (x - exit_x).abs() > 1.2);
    cands = cands.into_iter().map(|(x, y)| (x, lift(x, y))).collect();
    cands.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let total = d.pickup_total() as usize;
    let mut extra = 1.0;
    while cands.len() < total {
        // not enough places: a second row above
        let more: Vec<(f32, f32)> = cands.iter().map(|(x, y)| (*x, y + extra)).collect();
        cands.extend(more);
        extra += 1.0;
        cands.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    }
    let picked = spread(&cands.iter().map(|c| (c.0, c.1)).collect::<Vec<_>>(), total);
    let mut kinds: Vec<&super::design::PickupPlan> = vec![];
    for p in &d.pickups {
        for _ in 0..p.count {
            kinds.push(p);
        }
    }
    // interleave kinds deterministically along the level
    let n = kinds.len();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|i| (i * 7919) % n.max(1));
    for (k, (x, y)) in picked.into_iter().enumerate() {
        let p = kinds[order[k]];
        let pick = art.get(&format!("pickup:{}", p.kind.name()));
        let sc = sized(pick, cfg.pickups[p.kind.name()].size);
        ents.push(E::new(names.next(p.kind.name()), [x, y, 0.0], sc).tags(&[p.kind.name(), "pickup"]).art(pick).body(BodyKind::Static, false).0);
    }

    // the exit
    if let Some(x) = d.exit {
        let pick = art.get("exit");
        let sc = sized(pick, cfg.art.exit_height);
        let top = segs[last].top;
        ents.push(E::new(x.name(), [exit_x, top + sc[1] / 2.0, 0.0], sc).tags(&[x.name(), "exit"]).art(pick).body(BodyKind::Static, false).0);
    }

    // decor behind everything
    let mut decor: Vec<EntitySpec> = vec![];
    let kinds: Vec<(String, &Pick)> = art.picks.iter().filter(|(k, _)| k.starts_with("decor:")).map(|(k, p)| (k[6..].to_string(), p)).collect();
    if !kinds.is_empty() {
        for s in &segs {
            let mut x = s.x0 + lay.decor_every / 2.0;
            while x < s.x1 - 1.0 {
                let (kind, pick) = &kinds[rng.int(0, kinds.len() as i32 - 1) as usize];
                let h = cfg.art.decor_height.get(kind).copied().unwrap_or(1.0) * rng.range(0.85, 1.15);
                let sc = sized(pick, h);
                decor.push(E::new(names.next(kind), [x + rng.range(-1.0, 1.0), s.top + sc[1] / 2.0 - 0.05, -0.5], sc).art(pick).color(mul(pick.color, col.shade)).0);
                x += lay.decor_every;
            }
        }
    }
    if let Some(cloud) = art.picks.get("cloud") {
        for _ in 0..(lay.clouds as f32 * d.size).round() as u32 {
            let h = cfg.art.decor_height.get("cloud").copied().unwrap_or(1.4) * rng.range(0.7, 1.3);
            let sc = sized(cloud, h);
            decor.push(E::new(names.next("cloud"), [rng.range(-6.0, end_x + 6.0), rng.range(lay.cloud_height[0], lay.cloud_height[1]), -0.8], sc).art(cloud).color(mul(cloud.color, col.shade)).0);
        }
    }

    let player_e = E::new("player", lv.spawn, lv.player_size).tags(&["player"]).art(player).body(BodyKind::Dynamic, true).0;
    lv.entities = decor.into_iter().chain(ents).chain(std::iter::once(player_e)).collect();
    lv.surfaces = segs.into_iter().chain(floating).chain(pits).collect();
    lv.camera = CameraSpec {
        position: [lv.spawn[0], lv.spawn[1], 20.0],
        look_at: [0.0; 3],
        zoom: lay.camera_zoom,
        follow: Some("player".into()),
        offset: [if d.is("camera", "lookahead") { lay.lookahead } else { 0.0 }, lay.camera_rise, 0.0],
    };
    Ok(())
}

fn mul(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] * b[0], a[1] * b[1], a[2] * b[2]]
}

/// `n` items spread evenly over `slots` (in order); slots are reused when there are too few.
fn spread<T: Clone>(slots: &[(f32, T)], n: usize) -> Vec<(f32, T)> {
    if slots.is_empty() || n == 0 {
        return vec![];
    }
    (0..n).map(|i| slots[((2 * i + 1) * slots.len() / (2 * n)).min(slots.len() - 1) % slots.len()].clone()).collect()
}

// ---------------------------------------------------------------- arena

fn arena(d: &Design, cfg: &Config, art: &ArtPlan, col: &Colors, rng: &mut Rng, lv: &mut Level) -> Result<(), String> {
    let lay = &cfg.layout.arena;
    let w = (lay.width * d.size).round();
    let h = (lay.height * d.size).round();
    let t = lay.wall_thickness;
    let depth = d.depth;
    lv.bounds = Rect { x0: -w / 2.0, y0: -h / 2.0, x1: w / 2.0, y1: h / 2.0 };
    lv.gravity = if depth { [0.0, -round3(d.gravity), 0.0] } else { [0.0; 3] };
    let mut names = Namer::default();
    let mut ents: Vec<EntitySpec> = vec![];
    // plane (a, b) -> world position at height y
    let at = |a: f32, b: f32, y: f32| -> [f32; 3] {
        if depth {
            [a, y, b]
        } else {
            [a, b, 0.0]
        }
    };
    let tiles = art.tiles.as_ref();
    let wall_h = lay.wall_height_3d;
    let solid = |ents: &mut Vec<EntitySpec>, names: &mut Namer, r: Rect, base: &str, height: f32| {
        let (cw, ch) = (r.x1 - r.x0, r.y1 - r.y0);
        let (cx, cy) = ((r.x0 + r.x1) / 2.0, (r.y0 + r.y1) / 2.0);
        let e = if depth {
            E::new(names.next(base), [cx, height / 2.0, cy], [cw, height, ch]).color(col.wall)
        } else {
            E::new(names.next(base), [cx, cy, 0.0], [cw, ch, 1.0]).sprite(&tiles.expect("2d arena has tiles").wall).color(col.shade)
        };
        ents.push(e.tags(&["wall"]).body(BodyKind::Static, true).0);
    };
    // floor
    if depth {
        ents.push(E::new("floor", [0.0, -0.5, 0.0], [w + 2.0 * t, 1.0, h + 2.0 * t]).tags(&["ground"]).color(col.ground).body(BodyKind::Static, true).0);
    } else {
        // a plain floor in the theme's ground colour, so tiled walls and pillars stand out
        ents.push(E::new("floor", [0.0, 0.0, -1.0], [w, h, 1.0]).color(col.ground).0);
        // outside the walls: the theme's wall colour, not the sky
        lv.background = col.wall.map(round3);
    }
    // outer walls
    for r in [
        Rect { x0: -w / 2.0 - t, y0: h / 2.0, x1: w / 2.0 + t, y1: h / 2.0 + t },
        Rect { x0: -w / 2.0 - t, y0: -h / 2.0 - t, x1: w / 2.0 + t, y1: -h / 2.0 },
        Rect { x0: -w / 2.0 - t, y0: -h / 2.0, x1: -w / 2.0, y1: h / 2.0 },
        Rect { x0: w / 2.0, y0: -h / 2.0, x1: w / 2.0 + t, y1: h / 2.0 },
    ] {
        solid(&mut ents, &mut names, r, "wall", wall_h);
        lv.blocks.push(r);
    }
    // spawn
    let spawn = if d.is("layout", "rooms") { (-w / 4.0, -h / 4.0) } else { (0.0, 0.0) };
    // inner structure
    match d.variant("layout") {
        "pillars" => {
            let ps = lay.pillar_size;
            let pick = art.picks.iter().find(|(k, _)| k.starts_with("decor:")).map(|(_, p)| p.clone());
            let mut x = -w / 2.0 + lay.pillar_every;
            while x < w / 2.0 - lay.pillar_every * 0.5 {
                let mut y = -h / 2.0 + lay.pillar_every * 0.8;
                while y < h / 2.0 - lay.pillar_every * 0.5 {
                    let near = ((x - spawn.0).powi(2) + (y - spawn.1).powi(2)).sqrt() < cfg.checks.spawn_clear * 0.6;
                    if !near && rng.chance(lay.pillar_keep) {
                        let r = Rect::centered(x, y, ps, ps);
                        lv.blocks.push(r);
                        if depth {
                            let ph = lay.pillar_height_3d;
                            let mut e = E::new(names.next("pillar"), [x, ph / 2.0, y], [ps, ph, ps]).tags(&["wall"]).body(BodyKind::Static, true);
                            e = match &pick {
                                Some(p) => e.art(p),
                                None => e.color(col.wall),
                            };
                            ents.push(e.0);
                        } else {
                            solid(&mut ents, &mut names, r, "pillar", wall_h);
                        }
                    }
                    y += lay.pillar_every;
                }
                x += lay.pillar_every;
            }
        }
        "rooms" => {
            let door = lay.room_door;
            let th = t;
            // a vertical wall at x=0 with doors at y = +-h/4, a horizontal one at y=0 with doors at x = +-w/4
            let cuts = |len: f32| -> Vec<(f32, f32)> {
                let (a, b) = (-len / 4.0, len / 4.0);
                vec![(-len / 2.0, a - door / 2.0), (a + door / 2.0, b - door / 2.0), (b + door / 2.0, len / 2.0)]
            };
            for (y0, y1) in cuts(h) {
                let r = Rect { x0: -th / 2.0, y0, x1: th / 2.0, y1 };
                solid(&mut ents, &mut names, r, "wall", wall_h);
                lv.blocks.push(r);
            }
            for (x0, x1) in cuts(w) {
                let r = Rect { x0, y0: -th / 2.0, x1, y1: th / 2.0 };
                solid(&mut ents, &mut names, r, "wall", wall_h);
                lv.blocks.push(r);
            }
        }
        _ => {}
    }

    // things on free spots
    let mut taken: Vec<(f32, f32, f32)> = vec![(spawn.0, spawn.1, 1.5)];
    let blocks = lv.blocks.clone();
    let spot = |rng: &mut Rng, taken: &mut Vec<(f32, f32, f32)>, r: f32, min_spawn: f32| -> Option<(f32, f32)> {
        for _ in 0..400 {
            let x = rng.range(-w / 2.0 + r + 0.3, w / 2.0 - r - 0.3);
            let y = rng.range(-h / 2.0 + r + 0.3, h / 2.0 - r - 0.3);
            if blocks.iter().any(|b| b.grow(r + 0.3).contains(x, y)) {
                continue;
            }
            if ((x - spawn.0).powi(2) + (y - spawn.1).powi(2)).sqrt() < min_spawn {
                continue;
            }
            if taken.iter().any(|(tx, ty, tr)| ((x - tx).powi(2) + (y - ty).powi(2)).sqrt() < r + tr + 0.3) {
                continue;
            }
            let p = (round3(x), round3(y));
            taken.push((p.0, p.1, r));
            return Some(p);
        }
        None
    };
    let player = art.get("player");
    let ph = if depth { cfg.art.model_height } else { cfg.art.player_height };
    lv.player_size = sized(player, ph);
    lv.spawn = at(spawn.0, spawn.1, if depth { ph / 2.0 + 0.05 } else { 0.0 }).map(round3);

    for hz in &d.hazards {
        let pick = art.get(&format!("hazard:{}", hz.kind.name()));
        let size = cfg.hazards[hz.kind.name()].size;
        // seen from above a hazard is square
        let sc = if depth { [size[0], size[1], size[0]] } else { [size[0], size[0], 1.0] };
        for _ in 0..hz.count {
            let (a, b) = spot(rng, &mut taken, size[0].max(sc[1]) / 2.0 + 0.2, cfg.checks.spawn_clear).ok_or("no free floor left for the hazards")?;
            ents.push(E::new(names.next(hz.kind.name()), at(a, b, sc[1] / 2.0), sc).tags(&[hz.kind.name(), "hazard"]).art(pick).body(BodyKind::Static, false).0);
        }
    }
    for e in &d.enemies {
        let pick = art.get(&format!("enemy:{}", e.kind.name()));
        let size = cfg.enemies[e.kind.name()].size;
        let sc = sized(pick, if depth { size[1].max(0.8) * cfg.art.model_height / cfg.art.player_height } else { size[1] });
        for _ in 0..e.count {
            let (a, b) = spot(rng, &mut taken, sc[0].max(sc[1]) / 2.0, cfg.checks.spawn_clear).ok_or_else(|| format!("no room for {} {}s away from the player", e.count, e.kind.name()))?;
            let y = if depth { sc[1] / 2.0 + if e.flying { 0.6 } else { 0.05 } } else { 0.0 };
            let body = if e.flying { (BodyKind::Kinematic, false) } else { (BodyKind::Dynamic, true) };
            ents.push(E::new(names.next(e.kind.name()), at(a, b, y), sc).tags(&[e.kind.name(), "enemy"]).art(pick).body(body.0, body.1).0);
        }
        if e.behavior == Behavior::Patrol {
            lv.patrol.insert(e.kind, cfg.enemy_rules.patrol_distance);
        }
    }
    let rewarding = d.is("pickups", "rewarding");
    for p in &d.pickups {
        let pick = art.get(&format!("pickup:{}", p.kind.name()));
        let s = cfg.pickups[p.kind.name()].size;
        let sc = sized(pick, s);
        let mut spots = vec![];
        let tries = if rewarding { p.count * 3 } else { p.count };
        for _ in 0..tries {
            if let Some(sp) = spot(rng, &mut taken, sc[0].max(sc[1]) / 2.0, 2.0) {
                spots.push(sp);
            }
        }
        if rewarding {
            // the far corners, near the walls, away from the start
            spots.sort_by(|a, b| {
                let da = (a.0 - spawn.0).powi(2) + (a.1 - spawn.1).powi(2);
                let db = (b.0 - spawn.0).powi(2) + (b.1 - spawn.1).powi(2);
                db.total_cmp(&da).then(a.0.total_cmp(&b.0))
            });
        }
        if spots.len() < p.count as usize {
            return Err(format!("no room for {} {}s", p.count, p.kind.name()));
        }
        for (a, b) in spots.into_iter().take(p.count as usize) {
            ents.push(E::new(names.next(p.kind.name()), at(a, b, if depth { sc[1] / 2.0 + 0.3 } else { 0.0 }), sc).tags(&[p.kind.name(), "pickup"]).art(pick).body(BodyKind::Static, false).0);
        }
    }

    // decor outside the walls
    let kinds: Vec<(String, &Pick)> = art.picks.iter().filter(|(k, _)| k.starts_with("decor:")).map(|(k, p)| (k[6..].to_string(), p)).collect();
    if !kinds.is_empty() {
        for i in 0..lay.decor {
            let (kind, pick) = &kinds[i as usize % kinds.len()];
            let dh = cfg.art.decor_height.get(kind).copied().unwrap_or(1.0) * rng.range(0.8, 1.2);
            let sc = sized(pick, dh);
            let side = i % 4;
            let off = t + 1.0 + rng.range(0.0, 2.5);
            let (a, b) = match side {
                0 => (rng.range(-w / 2.0, w / 2.0), h / 2.0 + off),
                1 => (rng.range(-w / 2.0, w / 2.0), -h / 2.0 - off),
                2 => (-w / 2.0 - off, rng.range(-h / 2.0, h / 2.0)),
                _ => (w / 2.0 + off, rng.range(-h / 2.0, h / 2.0)),
            };
            ents.push(E::new(names.next(kind), at(a, b, if depth { sc[1] / 2.0 } else { 0.0 }), sc).art(pick).color(mul(pick.color, col.shade)).0);
        }
    }

    ents.push(E::new("player", lv.spawn, lv.player_size).tags(&["player"]).art(player).body(BodyKind::Dynamic, true).0);
    lv.entities = ents;
    lv.camera = if depth {
        let off = if d.is("camera", "high_cam") { lay.high_camera } else { lay.depth_camera };
        CameraSpec { position: [lv.spawn[0] + off[0], lv.spawn[1] + off[1], lv.spawn[2] + off[2]], look_at: lv.spawn, zoom: 20.0, follow: Some("player".into()), offset: [0.0; 3] }
    } else if d.is("camera", "follow") {
        CameraSpec { position: [lv.spawn[0], lv.spawn[1], 20.0], look_at: [0.0; 3], zoom: lay.follow_zoom, follow: Some("player".into()), offset: [0.0; 3] }
    } else {
        CameraSpec { position: [0.0, 0.0, 20.0], look_at: [0.0; 3], zoom: (lay.camera_zoom * d.size).max(h + 2.0 * t + 1.0), follow: None, offset: [0.0; 3] }
    };
    Ok(())
}

// ---------------------------------------------------------------- shooter

fn shooter(d: &Design, cfg: &Config, art: &ArtPlan, col: &Colors, rng: &mut Rng, lv: &mut Level) -> Result<(), String> {
    let lay = &cfg.layout.shooter;
    let hw = lay.half_width;
    let mut names = Namer::default();
    let mut ents: Vec<EntitySpec> = vec![];
    lv.gravity = [0.0; 3];
    // stars
    for _ in 0..lay.stars {
        let s = rng.range(lay.star_size[0], lay.star_size[1]);
        ents.push(E::new(names.next("star"), [rng.range(-hw * 1.6, hw * 1.6), rng.range(lay.player_y - 2.0, lay.top_y + 1.5), -1.0], [s, s, s]).shape(Shape::Sphere).color(col.decor).0);
    }
    // the fleet, top down
    let (sx, sy) = (lay.alien_spacing[0], lay.alien_spacing[1]);
    let mut row = 0;
    for e in &d.enemies {
        let pick = art.get(&format!("enemy:{}", e.kind.name()));
        let size = cfg.enemies[e.kind.name()].size;
        let sc = sized(pick, size[1]);
        let rows = e.rows.max(1);
        let cols = e.count.div_ceil(rows);
        if cols as f32 * sx > 2.0 * hw {
            return Err(format!("{} columns of {}s don't fit across the screen; ask for more rows", cols, e.kind.name()));
        }
        let mut left = e.count;
        for r in 0..rows {
            let n = left.min(cols);
            let y = lay.top_y - sy * (row + 1) as f32;
            for c in 0..n {
                let x = (c as f32 - (n as f32 - 1.0) / 2.0) * sx;
                ents.push(E::new(names.next(e.kind.name()), [x, y, 0.0], sc).tags(&[e.kind.name(), "enemy"]).art(pick).body(BodyKind::Kinematic, false).0);
            }
            left -= n;
            row += 1;
            let _ = r;
        }
    }
    if lay.top_y - sy * row as f32 <= lay.lose_y + 2.0 {
        return Err(format!("{row} rows of aliens start too close to the bottom"));
    }
    // shields
    if d.is("layout", "shields") {
        let b = lay.shield_block;
        let green = cfg.colors["green"];
        for i in 0..lay.shield_count {
            let cx = if lay.shield_count == 1 { 0.0 } else { -hw * 0.7 + 1.4 * hw * i as f32 / (lay.shield_count - 1) as f32 };
            for gx in 0..4 {
                for gy in 0..2 {
                    let x = cx + (gx as f32 - 1.5) * b;
                    let y = lay.shield_y + (gy as f32 - 0.5) * b;
                    ents.push(E::new(names.next("shield"), [x, y, 0.0], [b * 0.95, b * 0.95, 1.0]).tags(&["shield"]).color(green).body(BodyKind::Static, false).0);
                }
            }
        }
    }
    // pickups the sentence asked for: a row the ship sweeps
    for p in &d.pickups {
        let pick = art.get(&format!("pickup:{}", p.kind.name()));
        let sc = sized(pick, cfg.pickups[p.kind.name()].size);
        for i in 0..p.count {
            let x = -hw * 0.8 + 1.6 * hw * (i as f32 + 0.5) / p.count as f32;
            ents.push(E::new(names.next(p.kind.name()), [x, lay.player_y, 0.0], sc).tags(&[p.kind.name(), "pickup"]).art(pick).body(BodyKind::Static, false).0);
        }
    }
    let player = art.get("player");
    lv.player_size = sized(player, cfg.art.ship_height);
    lv.spawn = [0.0, lay.player_y, 0.0];
    lv.move_limit = round3(hw - lv.player_size[0]);
    ents.push(E::new("player", lv.spawn, lv.player_size).tags(&["player"]).art(player).body(BodyKind::Kinematic, true).0);
    lv.entities = ents;
    lv.kill_y = lay.lose_y;
    lv.camera = CameraSpec { position: [0.0, 0.0, 20.0], look_at: [0.0; 3], zoom: lay.camera_zoom, follow: None, offset: [0.0; 3] };
    Ok(())
}

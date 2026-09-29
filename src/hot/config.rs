//! `rules.toml`: every number the engine uses. Validated at load so a typo fails immediately.

use anyhow::{anyhow, bail, Context};
use serde::Deserialize;
use std::collections::BTreeMap;

pub const DEFAULT_RULES: &str = include_str!("../../rules.toml");

pub type Axes = [f32; 3];

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub search: Search,
    pub profile: Profile,
    pub tuning: Tuning,
    pub variants: BTreeMap<String, BTreeMap<String, VariantCfg>>,
    pub judges: BTreeMap<String, f32>,
    pub controls: BTreeMap<String, ControlCfg>,
    pub abilities: Abilities,
    pub layout: Layouts,
    pub enemies: BTreeMap<String, EnemyCfg>,
    pub enemy_rules: EnemyRules,
    pub pickups: BTreeMap<String, PickupCfg>,
    pub hazards: BTreeMap<String, HazardCfg>,
    pub counts: Counts,
    pub ramp: Ramp,
    pub checks: Checks,
    pub colors: BTreeMap<String, [f32; 3]>,
    pub themes: BTreeMap<String, ThemeCfg>,
    pub palette: PaletteCfg,
    pub default_theme: BTreeMap<String, String>,
    pub default_player: BTreeMap<String, String>,
    pub default_enemy: BTreeMap<String, String>,
    pub default_pickup: BTreeMap<String, String>,
    pub music: BTreeMap<String, MusicCfg>,
    pub art: ArtCfg,
    pub hud: HudCfg,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct HudCfg {
    pub help: [f32; 3],
    pub help_dim: f32,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ArtCfg {
    pub tile_size: f32,
    pub player_height: f32,
    pub ship_height: f32,
    pub exit_height: f32,
    pub shot_size: f32,
    pub decor_height: BTreeMap<String, f32>,
    pub model_height: f32,
    pub player_fallback: String,
    pub walk_fps: f32,
    pub enemy_fps: f32,
    pub tile_theme: BTreeMap<String, String>,
    pub decor: BTreeMap<String, Vec<String>>,
    pub model_fallback: BTreeMap<String, String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Search {
    pub beam_width: usize,
    pub per_shape: usize,
    pub finalists: usize,
    pub evidence_bonus: f32,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub default: Axes,
    pub shift: BTreeMap<String, Axes>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Tuning {
    pub pace_speed_gain: f32,
    pub challenge_count_gain: f32,
    pub challenge_speed_gain: f32,
    pub simplicity_count_cut: f32,
    pub edit_speed_step: f32,
    pub edit_count_step: f32,
    pub edit_size_step: f32,
    pub many_count: u32,
    pub few_count: u32,
}

#[derive(Deserialize, Debug, Clone, Copy)]
#[serde(deny_unknown_fields)]
pub struct VariantCfg {
    pub weight: f32,
    pub axes: Axes,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ControlCfg {
    #[serde(default)]
    pub run: f32,
    #[serde(default)]
    pub speed: f32,
    #[serde(default)]
    pub accel: f32,
    #[serde(default)]
    pub jump: f32,
    #[serde(default)]
    pub gravity: f32,
    #[serde(default)]
    pub variable_jump: bool,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Abilities {
    pub double_jump_factor: f32,
    pub dash_speed: f32,
    pub dash_time: f32,
    pub dash_cooldown: f32,
    pub player_bullet_speed: f32,
    pub player_fire_cooldown: f32,
    pub invulnerable_time: f32,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Layouts {
    pub platformer: PlatformerLayout,
    pub arena: ArenaLayout,
    pub shooter: ShooterLayout,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct PlatformerLayout {
    pub length: f32,
    pub kill_y: f32,
    pub segment_min: f32,
    pub segment_max: f32,
    pub gap_fraction: f32,
    pub rise_fraction: f32,
    pub platform_width: [f32; 2],
    pub spawn_x: f32,
    pub exit_margin: f32,
    pub camera_zoom: f32,
    pub lookahead: f32,
    pub camera_rise: f32,
    pub ground_bottom: f32,
    pub first_segment: f32,
    pub gap_min: f32,
    pub step_min: f32,
    pub max_height: f32,
    pub platform_thickness: f32,
    pub platform_gap: f32,
    pub pit_depth: f32,
    pub hazard_margin: f32,
    pub coin_height: f32,
    pub coin_spacing: f32,
    pub decor_every: f32,
    pub clouds: u32,
    pub cloud_height: [f32; 2],
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ArenaLayout {
    pub width: f32,
    pub height: f32,
    pub pillar_every: f32,
    pub pillar_size: f32,
    pub room_door: f32,
    pub wall_height_3d: f32,
    pub camera_zoom: f32,
    pub follow_zoom: f32,
    pub pillar_keep: f32,
    pub pillar_height_3d: f32,
    pub wall_thickness: f32,
    pub clear_cell: f32,
    pub decor: u32,
    pub depth_camera: [f32; 3],
    pub high_camera: [f32; 3],
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ShooterLayout {
    pub half_width: f32,
    pub player_y: f32,
    pub top_y: f32,
    pub lose_y: f32,
    pub alien_spacing: [f32; 2],
    pub alien_drop: f32,
    pub camera_zoom: f32,
    pub stars: u32,
    pub shield_count: u32,
    pub shield_y: f32,
    pub shield_block: f32,
    pub star_size: [f32; 2],
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct EnemyCfg {
    pub speed: f32,
    pub size: [f32; 2],
    pub behavior: String,
    pub flying: bool,
    pub score: u32,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct EnemyRules {
    pub chase_range: f32,
    pub fly_amplitude: f32,
    pub fly_frequency: f32,
    pub wander_turn: f32,
    pub shoot_interval: f32,
    pub shoot_speed: f32,
    pub stomp_bounce: f32,
    pub patrol_distance: f32,
    pub fly_height: f32,
    pub shoot_range: f32,
    pub wave_every: f32,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct PickupCfg {
    pub value: u32,
    pub size: f32,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct HazardCfg {
    pub size: [f32; 2],
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Counts {
    pub platformer_enemies: u32,
    pub arena_enemies: u32,
    pub shooter_rows: u32,
    pub shooter_columns: u32,
    pub platformer_pickups: u32,
    pub arena_pickups: u32,
    pub hazards: u32,
    pub lives: u32,
    pub survive_seconds: u32,
    pub waves: u32,
    pub min_enemies: u32,
    pub max_enemies: u32,
    pub max_pickups: u32,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Ramp {
    pub waves_speed_step: f32,
    pub speedup_per_second: f32,
    pub speedup_fleet: f32,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Checks {
    pub spawn_clear: f32,
    pub max_chase_ratio: f32,
    pub reach_margin: f32,
    pub sim_seconds: f32,
    pub max_entities: usize,
}

#[derive(Deserialize, Debug, Clone, Copy)]
#[serde(deny_unknown_fields)]
pub struct ThemeCfg {
    pub background: [f32; 3],
    pub ground: [f32; 3],
    pub platform: [f32; 3],
    pub wall: [f32; 3],
    pub decor: [f32; 3],
    pub text: [f32; 3],
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct PaletteCfg {
    pub dark_factor: f32,
    pub muted_mix: f32,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct MusicCfg {
    pub bpm: f32,
    pub wave: String,
    pub notes: String,
}

impl Config {
    pub fn load(path: Option<&std::path::Path>) -> anyhow::Result<Config> {
        let (src, name) = match path {
            Some(p) => (std::fs::read_to_string(p).with_context(|| format!("cannot read {}", p.display()))?, p.display().to_string()),
            None => (DEFAULT_RULES.to_string(), "built-in rules.toml".to_string()),
        };
        let cfg: Config = toml::from_str(&src).map_err(|e| anyhow!("{name}: {e}"))?;
        cfg.validate().map_err(|e| anyhow!("{name}: {e}"))?;
        Ok(cfg)
    }

    /// Every rule variant, kind and theme the code refers to must have its numbers here.
    fn validate(&self) -> anyhow::Result<()> {
        self.check_judges()?;
        for rule in crate::hot::rules::all() {
            let table = self.variants.get(rule.name()).ok_or_else(|| anyhow!("missing [variants.{}]", rule.name()))?;
            for id in rule.all_variants() {
                if !table.contains_key(*id) {
                    bail!("missing variants.{}.{id}", rule.name());
                }
            }
            for id in table.keys() {
                if !rule.all_variants().contains(&id.as_str()) {
                    bail!("variants.{}.{id} is not a variant of that rule (known: {})", rule.name(), rule.all_variants().join(", "));
                }
            }
        }
        for key in self.variants.keys() {
            if !crate::hot::rules::all().iter().any(|r| r.name() == key) {
                bail!("[variants.{key}] does not match any rule");
            }
        }
        use crate::hot::request::*;
        for k in EnemyKind::ALL {
            if !self.enemies.contains_key(k.name()) {
                bail!("missing [enemies.{}]", k.name());
            }
            let b = &self.enemies[k.name()].behavior;
            if Behavior::ALL.iter().all(|x| x.name() != b) {
                bail!("enemies.{}.behavior = {b:?} is not a behaviour", k.name());
            }
        }
        for k in PickupKind::ALL {
            if !self.pickups.contains_key(k.name()) {
                bail!("missing [pickups.{}]", k.name());
            }
        }
        for k in HazardKind::ALL {
            if !self.hazards.contains_key(k.name()) {
                bail!("missing [hazards.{}]", k.name());
            }
        }
        for k in Theme::ALL {
            if !self.themes.contains_key(k.name()) {
                bail!("missing [themes.{}]", k.name());
            }
        }
        for k in Color::ALL {
            if !self.colors.contains_key(k.name()) {
                bail!("missing colors.{}", k.name());
            }
        }
        for m in Mod::ALL {
            if !self.profile.shift.contains_key(m.name()) {
                bail!("missing profile.shift.{}", m.name());
            }
        }
        for id in ["snappy", "floaty", "direct", "momentum", "slide", "drift"] {
            if !self.controls.contains_key(id) {
                bail!("missing [controls.{id}]");
            }
        }
        for id in ["bounce", "drive", "calm"] {
            if !self.music.contains_key(id) {
                bail!("missing [music.{id}]");
            }
        }
        for g in Genre::ALL {
            let t = self.default_theme.get(g.name()).ok_or_else(|| anyhow!("missing default_theme.{}", g.name()))?;
            if Theme::ALL.iter().all(|x| x.name() != t) {
                bail!("default_theme.{} = {t:?} is not a theme", g.name());
            }
            let p = self.default_player.get(g.name()).ok_or_else(|| anyhow!("missing default_player.{}", g.name()))?;
            if PlayerKind::ALL.iter().all(|x| x.name() != p) {
                bail!("default_player.{} = {p:?} is not a player kind", g.name());
            }
            let e = self.default_enemy.get(g.name()).ok_or_else(|| anyhow!("missing default_enemy.{}", g.name()))?;
            if EnemyKind::ALL.iter().all(|x| x.name() != e) {
                bail!("default_enemy.{} = {e:?} is not an enemy kind", g.name());
            }
        }
        for (k, v) in &self.default_pickup {
            if PickupKind::ALL.iter().all(|x| x.name() != v) {
                bail!("default_pickup.{k} = {v:?} is not a pickup kind");
            }
        }
        for t in Theme::ALL {
            if !self.art.tile_theme.contains_key(t.name()) {
                bail!("missing art.tile_theme.{}", t.name());
            }
            if !self.art.decor.contains_key(t.name()) {
                bail!("missing art.decor.{}", t.name());
            }
            for d in &self.art.decor[t.name()] {
                if !self.art.decor_height.contains_key(d) {
                    bail!("art.decor.{} names {d:?}, which has no art.decor_height", t.name());
                }
            }
        }
        for (k, v) in [("tile_size", self.art.tile_size), ("player_height", self.art.player_height), ("model_height", self.art.model_height), ("shot_size", self.art.shot_size)] {
            if !(v > 0.0) {
                bail!("art.{k} must be above 0");
            }
        }
        if self.search.beam_width == 0 || self.search.finalists == 0 || self.search.per_shape == 0 {
            bail!("search.beam_width, search.per_shape and search.finalists must be at least 1");
        }
        Ok(())
    }

    pub fn variant(&self, rule: &str, id: &str) -> VariantCfg {
        self.variants[rule][id]
    }

    pub fn judge(&self, name: &str) -> f32 {
        *self.judges.get(name).unwrap_or_else(|| panic!("judge {name} missing from rules.toml (validated list out of date)"))
    }

    pub fn color(&self, c: crate::hot::request::Color) -> [f32; 3] {
        self.colors[c.name()]
    }
}

/// Judge names used by the code, checked at load.
pub const JUDGES: &[&str] = &[
    "chase_needs_direct_control",
    "rewarding_pickups_on_stacked",
    "trail_pickups_on_flat_run",
    "hazard_gaps_need_snappy",
    "one_hit_with_shooting_enemies",
    "lives_with_waves",
    "open_arena_in_depth",
    "lookahead_with_fast_controls",
    "calm_music_with_aggressive",
    "full_juice_in_depth",
];

impl Config {
    pub fn check_judges(&self) -> anyhow::Result<()> {
        for j in JUDGES {
            if !self.judges.contains_key(*j) {
                bail!("missing judges.{j}");
            }
        }
        for k in self.judges.keys() {
            if !JUDGES.contains(&k.as_str()) {
                bail!("judges.{k} is not a judge the engine knows (known: {})", JUDGES.join(", "));
            }
        }
        Ok(())
    }
}

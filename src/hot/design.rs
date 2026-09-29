//! A finalist's choices plus the request, turned into numbers: who is in the game, how many,
//! how fast, how it is won and lost. Every number comes from rules.toml, scaled by the profile
//! ([challenge, pace, simplicity]) and the sentence's edits. Every decision records the rule,
//! variant and words behind it for why.txt.

use super::config::Config;
use super::request::*;
use super::rules::{self, Chosen};
use super::search::Candidate;

/// One line of why.txt: a piece of the game, the rule/variant (or rules.toml entry) that
/// decided it, and the words of the sentence behind it ("" = nobody asked; the rule chose).
#[derive(Clone, Debug, PartialEq)]
pub struct Why {
    pub piece: String,
    pub rule: String,
    pub words: String,
}

#[derive(Clone, Debug)]
pub struct EnemyPlan {
    pub kind: EnemyKind,
    pub count: u32,
    /// Shooter fleet rows of this kind.
    pub rows: u32,
    pub behavior: Behavior,
    pub speed: f32,
    /// Flies (kinematic, passes through the level) instead of walking on it.
    pub flying: bool,
    pub shoots: bool,
    pub color: Option<Color>,
    pub score: u32,
}

#[derive(Clone, Debug)]
pub struct PickupPlan {
    pub kind: PickupKind,
    pub count: u32,
    pub value: u32,
    pub color: Option<Color>,
}

#[derive(Clone, Debug)]
pub struct HazardPlan {
    pub kind: HazardKind,
    pub count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Goal {
    CollectAll,
    ReachExit,
    Survive,
    DestroyAll,
}

#[derive(Clone, Debug)]
pub struct Design {
    pub sentence: String,
    pub genre: Genre,
    pub depth: bool,
    pub chosen: Chosen,
    pub profile: [f32; 3],
    pub theme: Theme,
    pub night: bool,
    pub player: PlayerKind,
    /// The sentence named the player's kind (otherwise the art may use a generic hero).
    pub player_stated: bool,
    pub player_color: Option<Color>,
    /// Run speed (platformer) or move speed (arena, shooter).
    pub speed: f32,
    pub smooth: bool,
    pub jump: f32,
    pub gravity: f32,
    pub variable_jump: bool,
    pub double_jump: bool,
    pub can_jump: bool,
    pub player_shoots: bool,
    pub enemies: Vec<EnemyPlan>,
    pub pickups: Vec<PickupPlan>,
    pub hazards: Vec<HazardPlan>,
    pub exit: Option<ExitKind>,
    pub goal: Goal,
    /// 1 = one hit loses.
    pub lives: u32,
    pub survive: u32,
    /// Waves to clear (1 = no waves).
    pub waves: u32,
    /// Level size multiplier ("bigger level").
    pub size: f32,
    pub seed: u64,
    pub why: Vec<Why>,
}

impl Design {
    pub fn is(&self, rule: &str, variant: &str) -> bool {
        self.chosen.get(rule).copied() == Some(variant)
    }
    pub fn variant(&self, rule: &str) -> &'static str {
        self.chosen.get(rule).copied().unwrap_or("")
    }
    fn note(&mut self, piece: impl Into<String>, rule: impl Into<String>, words: impl Into<String>) {
        self.why.push(Why { piece: piece.into(), rule: rule.into(), words: words.into() });
    }
    /// The rule/variant tag for why.txt.
    pub fn rv(&self, rule: &str) -> String {
        format!("{rule}/{}", self.variant(rule))
    }
    pub fn full_juice(&self) -> bool {
        self.is("juice", "full")
    }
    pub fn enemy_total(&self) -> u32 {
        self.enemies.iter().map(|e| e.count).sum()
    }
    pub fn pickup_total(&self) -> u32 {
        self.pickups.iter().map(|p| p.count).sum()
    }
}

fn step_pow(step: f32, n: i32) -> f32 {
    step.powi(n)
}

pub fn amount(cfg: &Config, a: Amount) -> u32 {
    match a {
        Amount::N(n) => n,
        Amount::Many => cfg.tuning.many_count,
        Amount::Few => cfg.tuning.few_count,
    }
}

/// Turn a finalist into a design. Errors are honest failures ("0 coins can't be collected").
pub fn design(req: &Request, cfg: &Config, cand: &Candidate) -> Result<Design, String> {
    let chosen = cand.chosen.clone();
    let profile = rules::profile(req, cfg);
    let genre = rules::genre_of(&chosen);
    let depth = rules::depth(&chosen);
    let [challenge, pace, simplicity] = profile;
    let t = &cfg.tuning;
    let pace_mult = 1.0 + (pace - 0.5) * t.pace_speed_gain;
    let count_mult = (1.0 + (challenge - 0.5) * t.challenge_count_gain) * (1.0 - (simplicity - 0.5) * t.simplicity_count_cut);
    let enemy_speed_mult = (1.0 + (challenge - 0.5) * t.challenge_speed_gain) * pace_mult * step_pow(t.edit_speed_step, req.edit_steps(Target::Enemies, Change::Faster, Change::Slower));
    let player_speed_mult = pace_mult * step_pow(t.edit_speed_step, req.edit_steps(Target::Player, Change::Faster, Change::Slower));
    let size = step_pow(t.edit_size_step, req.edit_steps(Target::Level, Change::Bigger, Change::Smaller));
    let enemy_count_edit = step_pow(t.edit_count_step, req.edit_steps(Target::Enemies, Change::More, Change::Fewer));
    let pickup_count_edit = step_pow(t.edit_count_step, req.edit_steps(Target::Pickups, Change::More, Change::Fewer));
    let hazard_count_edit = step_pow(t.edit_count_step, req.edit_steps(Target::Hazards, Change::More, Change::Fewer));

    let seed_text = format!("{}|{}", req.sentence.to_lowercase(), chosen.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(","));
    let mut d = Design {
        sentence: req.sentence.clone(),
        genre,
        depth,
        chosen: chosen.clone(),
        profile,
        theme: Theme::Forest,
        night: chosen.get("palette") == Some(&"dark"),
        player: PlayerKind::Knight,
        player_stated: req.player.is_some(),
        player_color: req.player_color.as_ref().map(|c| c.v),
        speed: 0.0,
        smooth: false,
        jump: 0.0,
        gravity: 0.0,
        variable_jump: false,
        double_jump: false,
        can_jump: false,
        player_shoots: false,
        enemies: vec![],
        pickups: vec![],
        hazards: vec![],
        exit: None,
        goal: Goal::CollectAll,
        lives: 1,
        survive: 0,
        waves: 1,
        size,
        seed: super::rng::fnv(&seed_text),
        why: vec![],
    };
    for s in &cand.steps {
        d.note(format!("{}: {}", s.rule, s.variant), format!("{}/{}", s.rule, s.variant), s.evidence.clone().unwrap_or_default());
    }
    if profile != cfg.profile.default {
        let words: Vec<String> = req.mods.iter().map(|m| m.words.clone()).collect();
        d.note(format!("profile [challenge, pace, simplicity] = [{:.2}, {:.2}, {:.2}]", profile[0], profile[1], profile[2]), "profile.shift", words.join(", "));
    }

    // theme
    let key = if depth && genre == Genre::Arena { "arena" } else { genre.name() };
    d.theme = match &req.theme {
        Some(th) => {
            d.note(format!("theme {}", th.v.name()), "theme", th.words.clone());
            th.v
        }
        None => {
            let th = Theme::from_name(&cfg.default_theme[genre.name()]).expect("validated");
            d.note(format!("theme {}", th.name()), format!("rules.toml default_theme.{}", genre.name()), "");
            th
        }
    };

    // player
    d.player = match &req.player {
        Some(p) => p.v,
        None => {
            let k = if depth { cfg.default_player.get("arena_depth").unwrap_or(&cfg.default_player[key]) } else { &cfg.default_player[genre.name()] };
            PlayerKind::from_name(k).expect("validated")
        }
    };
    d.note(format!("player {}", d.player.name()), if d.player_stated { "player".to_string() } else { format!("rules.toml default_player.{}", genre.name()) }, req.player.as_ref().map(|p| p.words.clone()).unwrap_or_default());
    if genre != Genre::Shooter && d.player == PlayerKind::Ship {
        return Err(format!("\"{}\": a ship belongs in a shooter (\"make a space shooter ...\")", req.player.as_ref().unwrap().words));
    }

    // controls
    let controls = d.variant("controls");
    let c = cfg.controls[controls].clone();
    d.smooth = controls == "momentum" || controls == "drift" || controls == "floaty";
    match genre {
        Genre::Platformer => {
            d.speed = c.run * player_speed_mult;
            d.jump = c.jump;
            d.gravity = c.gravity;
            d.variable_jump = c.variable_jump;
            d.can_jump = true;
            d.double_jump = req.has_ability(Ability::DoubleJump).is_some();
        }
        _ => {
            d.speed = c.speed * player_speed_mult;
            d.can_jump = depth && req.abilities.iter().any(|a| matches!(a.v, Ability::Jump | Ability::DoubleJump));
            d.double_jump = depth && req.has_ability(Ability::DoubleJump).is_some();
            d.jump = cfg.controls["floaty"].jump;
            d.gravity = cfg.controls["floaty"].gravity;
        }
    }
    let rv = d.rv("controls");
    d.note(format!("player speed {:.2} units/s", d.speed), rv.clone(), req.edit_words(Target::Player));
    if d.double_jump {
        d.note("double jump", "abilities.double_jump_factor", req.has_ability(Ability::DoubleJump).unwrap().words.clone());
    }

    // player shooting: asked for, a shooter, or "shoot the zombies"
    let shoot_words = req.has_ability(Ability::Shoot).map(|a| a.words.clone()).or_else(|| req.goal(|g| *g == GoalReq::DestroyAll).map(|g| g.words.clone()));
    d.player_shoots = genre == Genre::Shooter || shoot_words.is_some() || d.is("goal", "destroy_all");
    if d.player_shoots {
        d.note("the player shoots", if genre == Genre::Shooter { "genre/shooter".to_string() } else { "abilities.player_bullet_speed".to_string() }, shoot_words.unwrap_or_default());
    }

    // enemies
    if rules::has_enemies(req) {
        let mut reqs: Vec<(EnemyKind, Option<&EnemyReq>)> = req.enemies.iter().map(|e| (e.kind.v, Some(e))).collect();
        if reqs.is_empty() {
            let k = EnemyKind::from_name(&cfg.default_enemy[genre.name()]).expect("validated");
            reqs.push((k, None));
        }
        let default_total = match genre {
            Genre::Platformer => cfg.counts.platformer_enemies,
            Genre::Arena => cfg.counts.arena_enemies,
            Genre::Shooter => cfg.counts.shooter_rows * cfg.counts.shooter_columns,
        };
        let generic_count = req.generic_enemies.as_ref().and_then(|g| g.v.map(|a| (amount(cfg, a), g.words.clone())));
        let unspecified = reqs.iter().filter(|(_, e)| e.and_then(|e| e.count.as_ref()).is_none()).count().max(1) as f32;
        let shoot_all = req.generic_shoots.as_ref().map(|s| s.words.clone());
        let mut rows_left = cfg.counts.shooter_rows;
        let n_kinds = reqs.len() as u32;
        for (i, (kind, e)) in reqs.iter().enumerate() {
            let kcfg = &cfg.enemies[kind.name()];
            let (behavior, bwords, brule) = behavior_for(req, cfg, &chosen, genre, *kind, *e);
            let mut count = match (e.and_then(|e| e.count.as_ref()), &generic_count) {
                (Some(c), _) => amount(cfg, c.v),
                (None, Some((n, _))) => ((*n as f32 / unspecified).round() as u32).max(1),
                (None, None) => ((default_total as f32 * count_mult * enemy_count_edit / unspecified).round() as u32).max(cfg.counts.min_enemies),
            };
            let count_words = e.and_then(|e| e.count.as_ref()).map(|c| c.words.clone()).or_else(|| generic_count.as_ref().map(|g| g.1.clone())).unwrap_or_default();
            let mut rows = 0;
            if genre == Genre::Shooter {
                // the fleet: rows x columns of each kind
                rows = match e.and_then(|e| e.rows.as_ref()) {
                    Some(r) => r.v.max(1),
                    None => {
                        let left_kinds = n_kinds - i as u32;
                        let r = (rows_left as f32 / left_kinds as f32).ceil().max(1.0) as u32;
                        rows_left = rows_left.saturating_sub(r);
                        r
                    }
                };
                let explicit = e.and_then(|e| e.count.as_ref()).is_some() || generic_count.is_some();
                let columns = if explicit { count.div_ceil(rows).max(1) } else { ((cfg.counts.shooter_columns as f32 * count_mult.sqrt() * enemy_count_edit).round() as u32).max(2) };
                if !explicit {
                    count = rows * columns;
                } else {
                    count = count.max(1);
                }
            }
            count = count.clamp(cfg.counts.min_enemies, cfg.counts.max_enemies);
            let flying = matches!(behavior, Behavior::Fly) || (kcfg.flying && behavior == Behavior::Chase) || genre == Genre::Shooter;
            let mut speed = kcfg.speed * enemy_speed_mult;
            let hard = req.mods.iter().any(|m| m.v == Mod::Hard);
            let cap = d.speed * cfg.checks.max_chase_ratio * if hard { 1.2 } else { 1.0 };
            if behavior == Behavior::Chase && speed > cap {
                d.note(format!("{} speed capped at {:.2} (a chaser stays slower than the player)", kind.name(), cap), "checks.max_chase_ratio", "");
                speed = cap;
            }
            let shoots = genre == Genre::Shooter || e.and_then(|e| e.shoots.as_ref()).is_some() || shoot_all.is_some();
            let plan = EnemyPlan { kind: *kind, count, rows, behavior, speed, flying, shoots, color: e.and_then(|e| e.color.as_ref()).map(|c| c.v), score: kcfg.score };
            let kind_words = e.map(|e| e.kind.words.clone()).filter(|k| !bwords.contains(k.as_str()) && !count_words.contains(k.as_str())).unwrap_or_default();
            let words = [kind_words, count_words, bwords].iter().filter(|w| !w.is_empty()).cloned().collect::<Vec<_>>().join(", ");
            let rule = if e.is_none() { format!("rules.toml default_enemy.{} + {brule}", genre.name()) } else { brule };
            d.note(format!("{} x{} {} at {:.2} units/s{}", kind.name(), count, behavior.name(), speed, if shoots { ", shooting" } else { "" }), rule, words);
            d.enemies.push(plan);
        }
    }

    // pickups
    if rules::has_pickups(req, &chosen) {
        let mut reqs: Vec<(PickupKind, Option<&PickupReq>)> = req.pickups.iter().map(|p| (p.kind.v, Some(p))).collect();
        if reqs.is_empty() {
            let k = cfg.default_pickup.get(genre.name()).and_then(|k| PickupKind::from_name(k)).unwrap_or(PickupKind::Coin);
            reqs.push((k, None));
        }
        let default_total = match genre {
            Genre::Platformer => cfg.counts.platformer_pickups,
            _ => cfg.counts.arena_pickups,
        };
        let generic_count = req.generic_pickups.as_ref().and_then(|g| g.v.map(|a| (amount(cfg, a), g.words.clone())));
        let unspecified = reqs.iter().filter(|(_, p)| p.and_then(|p| p.count.as_ref()).is_none()).count().max(1) as f32;
        for (kind, p) in reqs {
            let count = match (p.and_then(|p| p.count.as_ref()), &generic_count) {
                (Some(c), _) => amount(cfg, c.v),
                (None, Some((n, _))) => ((*n as f32 / unspecified).round() as u32).max(1),
                (None, None) => ((default_total as f32 * size * pickup_count_edit / unspecified).round() as u32).max(1),
            };
            if count == 0 {
                return Err(format!("\"{}\": 0 {}s can't be collected", p.map(|p| p.kind.words.clone()).unwrap_or_default(), kind.name()));
            }
            let count = count.min(cfg.counts.max_pickups);
            let words = p.map(|p| [Some(p.kind.words.clone()), p.count.as_ref().map(|c| c.words.clone())].into_iter().flatten().collect::<Vec<_>>().join(", ")).unwrap_or_default();
            let rule = if p.is_none() { format!("rules.toml default_pickup.{}", genre.name()) } else { d.rv("pickups") };
            d.note(format!("{} x{}", kind.name(), count), rule, words);
            d.pickups.push(PickupPlan { kind, count, value: cfg.pickups[kind.name()].value, color: p.and_then(|p| p.color.as_ref()).map(|c| c.v) });
        }
    }

    // hazards
    for h in &req.hazards {
        let count = match &h.count {
            Some(c) => amount(cfg, c.v),
            None => ((cfg.counts.hazards as f32 * count_mult * hazard_count_edit * size).round() as u32).max(1),
        };
        d.note(format!("{} x{}", h.kind.v.name(), count), d.rv("hazards"), [Some(h.kind.words.clone()), h.count.as_ref().map(|c| c.words.clone())].into_iter().flatten().collect::<Vec<_>>().join(", "));
        d.hazards.push(HazardPlan { kind: h.kind.v, count });
    }

    // goal
    d.goal = match d.variant("goal") {
        "collect_all" => Goal::CollectAll,
        "reach_exit" => Goal::ReachExit,
        "survive" => Goal::Survive,
        _ => Goal::DestroyAll,
    };
    if genre == Genre::Platformer && (d.goal == Goal::ReachExit || req.exit.is_some()) {
        d.exit = Some(req.exit.as_ref().map(|e| e.v).unwrap_or(ExitKind::Flag));
        d.note(format!("exit {}", d.exit.unwrap().name()), d.rv("goal"), req.exit.as_ref().map(|e| e.words.clone()).unwrap_or_default());
    } else if let Some(e) = &req.exit {
        return Err(format!("\"{}\": exits are for platformers; a {} is won by its goal", e.words, genre.name()));
    }
    if d.goal == Goal::CollectAll && d.pickups.is_empty() {
        return Err("nothing to collect: name some pickups (coins, gems, stars ...)".into());
    }
    if d.goal == Goal::Survive {
        let (secs, words) = match req.goal(|g| matches!(g, GoalReq::Survive(Some(_)))) {
            Some(g) => match g.v {
                GoalReq::Survive(Some(s)) => (s, g.words.clone()),
                _ => unreachable!(),
            },
            None => (cfg.counts.survive_seconds, String::new()),
        };
        if secs == 0 {
            return Err(format!("\"{words}\": surviving 0 seconds is no game"));
        }
        d.survive = secs;
        d.note(format!("survive {secs} seconds"), d.rv("goal"), words);
    }
    if d.is("ramp", "waves") {
        d.waves = cfg.counts.waves;
        if d.goal == Goal::Survive {
            d.note(format!("a new wave every {} seconds", cfg.enemy_rules.wave_every), d.rv("ramp"), "");
        } else {
            d.note(format!("{} waves", d.waves), d.rv("ramp"), "");
        }
    }

    // health
    d.lives = if d.is("health", "lives") { req.lives.as_ref().map(|l| l.v).unwrap_or(cfg.counts.lives) } else { 1 };
    if d.chosen.contains_key("health") {
        d.note(if d.lives > 1 { format!("{} lives", d.lives) } else { "one hit loses".into() }, d.rv("health"), req.lives.as_ref().map(|l| l.words.clone()).unwrap_or_default());
    }
    if size != 1.0 {
        d.note(format!("level size x{size:.2}"), "tuning.edit_size_step", req.edit_words(Target::Level));
    }
    Ok(d)
}

/// How one enemy kind behaves: stated words win, then the enemy_behavior variant, then the
/// genre's limits (a platformer has no wandering, an arena no marching, a shooter only a fleet).
fn behavior_for(req: &Request, cfg: &Config, chosen: &Chosen, genre: Genre, kind: EnemyKind, e: Option<&EnemyReq>) -> (Behavior, String, String) {
    let stated = e.and_then(|e| e.behaviors.last()).or_else(|| req.generic_behaviors.last());
    let kcfg = &cfg.enemies[kind.name()];
    let variant = chosen.get("enemy_behavior").copied().unwrap_or("by_kind");
    let (mut b, words, mut rule) = match stated {
        Some(s) => (s.v, s.words.clone(), "enemy behaviour (stated)".to_string()),
        None => {
            let by_kind = Behavior::from_name(&kcfg.behavior).expect("validated");
            let b = match variant {
                "aggressive" if genre != Genre::Shooter => Behavior::Chase,
                "calm" if kcfg.flying => Behavior::Fly,
                "calm" if genre == Genre::Arena => Behavior::Wander,
                "calm" => Behavior::Patrol,
                _ => by_kind,
            };
            (b, String::new(), format!("enemy_behavior/{variant}"))
        }
    };
    let mapped = match (genre, b) {
        (Genre::Shooter, _) => Behavior::March,
        (Genre::Platformer, Behavior::Wander | Behavior::March) => Behavior::Patrol,
        (Genre::Arena, Behavior::Fly | Behavior::March) => Behavior::Wander,
        (_, b) => b,
    };
    if mapped != b {
        rule = format!("{rule}, {} becomes {} in a {}", b.name(), mapped.name(), genre.name());
        b = mapped;
    }
    (b, words, rule)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hot::rules::test_util::{cfg, req};
    use crate::hot::rules::Ctx;

    fn best(s: &str) -> Design {
        let c = cfg();
        let r = req(s);
        let cx = Ctx { req: &r, cfg: &c, profile: rules::profile(&r, &c) };
        let o = crate::hot::search::search(&cx).unwrap();
        design(&r, &c, &o.finalists[0]).unwrap()
    }

    #[test]
    fn numbers_come_from_the_sentence_and_rules() {
        let d = best("make a platformer with 20 coins and slimes");
        assert_eq!(d.pickups[0].count, 20);
        assert_eq!(d.enemies[0].kind, EnemyKind::Slime);
        assert_eq!(d.enemies[0].behavior, Behavior::Patrol);
        assert!(d.speed > 0.0 && d.jump > 0.0);
        assert!(d.why.iter().any(|w| w.words.contains("20 coins")));
    }

    #[test]
    fn chasers_are_capped_below_the_player() {
        let d = best("make a very fast arena where zombies chase the player, make the enemies faster, make the enemies faster, make the enemies faster");
        let z = &d.enemies[0];
        assert!(z.speed <= d.speed * cfg().checks.max_chase_ratio + 1e-4, "{} vs {}", z.speed, d.speed);
    }

    #[test]
    fn shooter_fleet_is_rows_by_columns() {
        let d = best("make a space shooter with 3 rows of aliens that shoot back");
        assert_eq!(d.enemies[0].rows, 3);
        assert_eq!(d.enemies[0].count % 3, 0);
        assert!(d.enemies[0].shoots && d.player_shoots);
        assert_eq!(d.goal, Goal::DestroyAll);
    }

    #[test]
    fn survive_takes_the_stated_seconds() {
        let d = best("make a top-down arena where zombies chase the player, survive 60 seconds");
        assert_eq!(d.goal, Goal::Survive);
        assert_eq!(d.survive, 60);
        assert_eq!(d.enemies[0].behavior, Behavior::Chase);
    }
}

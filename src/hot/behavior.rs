//! behavior.txt: the game's behaviour as sentences in Senne's game language (DROID_DESIGN.md
//! section 3). Speeds, ranges, counts and texts are filled in from the design (rules.toml and
//! the profile) and the level (what fits it). Senne turns these sentences into src/main.rs.

use super::config::Config;
use super::design::{Design, Goal};
use super::level::Level;
use super::paint::ArtPlan;
use super::request::*;

/// A number as Senne reads it: at most 2 decimals, no trailing zeros.
pub fn num(x: f32) -> String {
    let s = format!("{:.2}", x);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".into()
    } else {
        s.to_string()
    }
}

fn hex(c: [f32; 3]) -> String {
    let b = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", b(c[0]), b(c[1]), b(c[2]))
}

fn plural(s: &str) -> String {
    format!("{s}s")
}

/// "a" or "an".
fn a(s: &str) -> String {
    if s.starts_with(['a', 'e', 'i', 'o', 'u']) {
        format!("an {s}")
    } else {
        format!("a {s}")
    }
}

/// One sentence plus the rule/variant that asked for it (for why.txt).
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub text: String,
    pub rule: String,
}

struct Out(Vec<Line>);

impl Out {
    fn push(&mut self, rule: impl Into<String>, text: impl Into<String>) {
        self.0.push(Line { text: text.into(), rule: rule.into() });
    }
}

/// How the score counts: pickups one by one ("COINS 3 / 14") or points.
fn count_mode(d: &Design) -> bool {
    d.pickups.len() == 1 && matches!(d.goal, Goal::CollectAll | Goal::ReachExit) && !d.player_shoots
}

/// The thing word for "no <things> are left" over all pickups / enemies.
fn pickups_word(d: &Design) -> String {
    if d.pickups.len() == 1 {
        plural(d.pickups[0].kind.name())
    } else {
        "pickups".into()
    }
}

fn enemies_word(d: &Design) -> String {
    if d.enemies.len() == 1 {
        plural(d.enemies[0].kind.name())
    } else {
        "enemies".into()
    }
}

pub fn write(d: &Design, cfg: &Config, lv: &Level, art: &ArtPlan) -> Vec<Line> {
    let mut o = Out(vec![]);
    let hurt = if d.lives > 1 { format!("the player loses a life, safe for {} seconds", num(cfg.abilities.invulnerable_time)) } else { "the player loses".to_string() };
    let full = d.full_juice();
    let count = count_mode(d);
    let depth = d.depth;
    let ground = if depth { " across the ground" } else { "" };

    // ---- at the start: lives, music, HUD
    let mut start: Vec<String> = vec![];
    if d.lives > 1 {
        start.push(format!("the player has {} lives", d.lives));
    }
    let m = &cfg.music[d.variant("music")];
    start.push(format!("play the music \"{}\" at {} beats per minute as a {} wave", m.notes, num(m.bpm), m.wave));
    let mut hud: Vec<String> = vec![];
    if !d.pickups.is_empty() {
        if count {
            let p = &d.pickups[0];
            hud.push(format!("{} {{score}} / {}", plural(p.kind.name()).to_uppercase(), p.count));
        } else {
            hud.push("SCORE {score}".into());
        }
    } else if d.player_shoots || d.goal == Goal::DestroyAll {
        hud.push("SCORE {score}".into());
    }
    if d.lives > 1 {
        hud.push("LIVES {lives}".into());
    }
    if d.goal == Goal::Survive {
        hud.push(format!("TIME {{countdown {}}}", d.survive));
    }
    if d.waves > 1 {
        hud.push("WAVE {wave}".into());
    }
    if !count && d.goal == Goal::CollectAll && d.pickups.len() == 1 {
        let k = d.pickups[0].kind.name();
        hud.push(format!("{} LEFT {{left {k}}}", plural(k).to_uppercase()));
    }
    let color = hex(lv.text_color);
    if d.is("hud", "top_bar") {
        if !hud.is_empty() {
            start.push(format!("show \"{}\" at the top in \"{color}\"", hud.join("   ")));
        }
    } else {
        let places = ["at the top left", "at the top right", "at the top", "at the bottom"];
        for (i, h) in hud.iter().enumerate() {
            start.push(format!("show \"{h}\" {} in \"{color}\"", places[i.min(places.len() - 1)]));
        }
    }
    o.push(format!("{} + {} + {}", d.rv("music"), d.rv("hud"), if d.lives > 1 { d.rv("health") } else { "goal".into() }), format!("At the start, {}.", join_and(&start)));

    // ---- the player
    let smooth = if d.smooth { ", smoothly" } else { "" };
    match d.genre {
        Genre::Platformer => {
            o.push(d.rv("controls"), format!("Move the player sideways with the arrow keys at {} units per second and face where it moves{smooth}.", num(d.speed)));
        }
        Genre::Arena => {
            o.push(d.rv("controls"), format!("Move the player{ground} with the arrow keys at {} units per second and face where it moves{smooth}.", num(d.speed)));
        }
        Genre::Shooter => {
            o.push(d.rv("controls"), format!("Move the player sideways with the arrow keys at {} units per second within {} units{smooth}.", num(d.speed), num(lv.move_limit)));
        }
    }
    if d.can_jump {
        let mut j = format!("The player jumps with space at {} units per second", num(d.jump));
        if d.double_jump {
            j.push_str(&format!(", twice at {} units per second", num(d.jump * cfg.abilities.double_jump_factor)));
        }
        if d.variable_jump {
            j.push_str(", higher while held");
        }
        j.push_str(", with the jump sound.");
        o.push(d.rv("controls"), j);
    }
    if d.player_shoots {
        let (dir, key, sound) = match d.genre {
            Genre::Shooter => ("up", "space", "laser"),
            Genre::Platformer => ("forward", "x", "shoot"),
            Genre::Arena if d.can_jump => ("forward", "x", "shoot"),
            Genre::Arena => ("forward", "space", "shoot"),
        };
        o.push(
            "abilities.player_bullet_speed",
            format!(
                "The player shoots {dir}{ground} with {key} at {} units per second every {} seconds, using \"{}\", with the {sound} sound.",
                num(cfg.abilities.player_bullet_speed),
                num(cfg.abilities.player_fire_cooldown),
                art.get("shot").name
            ),
        );
    }
    let player = art.get("player");
    if full && !depth && player.frames.len() > 1 {
        o.push(d.rv("juice"), format!("Animate the player with {} at {} frames per second while it walks.", frames(&player.frames), num(cfg.art.walk_fps)));
    }

    // ---- enemies
    let r = &cfg.enemy_rules;
    for e in &d.enemies {
        let k = e.kind.name();
        let line = match e.behavior {
            Behavior::Patrol => format!("Each {k} patrols {} units to each side at {} units per second.", num(*lv.patrol.get(&e.kind).unwrap_or(&r.patrol_distance)), num(e.speed)),
            Behavior::Chase if d.genre == Genre::Platformer => format!("Each {k} moves toward the player sideways at {} units per second within {} units.", num(e.speed), num(r.chase_range)),
            Behavior::Chase => {
                let mut s = format!("Each {k} moves toward the player{ground} at {} units per second", num(e.speed));
                if d.is("ramp", "speedup") {
                    s.push_str(&format!(", speeding up by {} percent per second", num(cfg.ramp.speedup_per_second * 100.0)));
                }
                s + "."
            }
            Behavior::Fly => format!("Each {k} flies in a wave {} units high at {} cycles per second.", num(r.fly_amplitude), num(r.fly_frequency)),
            Behavior::Wander => format!("Each {k} wanders{ground} at {} units per second, turning every {} seconds.", num(e.speed), num(r.wander_turn)),
            Behavior::March => {
                let lay = &cfg.layout.shooter;
                let mut s = format!("Each {k} marches sideways at {} units per second within {} units, stepping down {} units", num(e.speed), num(lay.half_width), num(lay.alien_drop));
                if !d.is("ramp", "steady") {
                    s.push_str(&format!(", up to {} times as fewer are left", num(cfg.ramp.speedup_fleet)));
                }
                if d.waves > 1 {
                    s.push_str(&format!(", {} percent faster each wave", num((cfg.ramp.waves_speed_step - 1.0) * 100.0)));
                }
                s + "."
            }
        };
        let rule = if d.chosen.contains_key("enemy_behavior") { d.rv("enemy_behavior") } else { format!("genre/{}", d.genre.name()) };
        o.push(format!("{rule} ({k}: {})", e.behavior.name()), line);
        let pick = art.get(&format!("enemy:{k}"));
        if full && !depth && pick.frames.len() > 1 {
            o.push(d.rv("juice"), format!("Animate each {k} with {} at {} frames per second.", frames(&pick.frames), num(pick.fps)));
        }
    }
    // enemies that shoot
    let aimed = d.is("enemy_attack", "aimed");
    let bomb = art.picks.get("bomb").map(|p| p.name.clone());
    if let Some(bomb) = &bomb {
        let dir = if aimed { "at the player" } else { "down" };
        if d.genre == Genre::Shooter {
            let who = if d.enemies.len() == 1 { d.enemies[0].kind.name().to_string() } else { "enemy".into() };
            o.push(d.rv("enemy_attack"), format!("Every {} seconds, a random {who} shoots {dir} at {} units per second, using \"{bomb}\".", num(r.shoot_interval), num(r.shoot_speed)));
        } else {
            for e in d.enemies.iter().filter(|e| e.shoots) {
                o.push(
                    d.rv("enemy_attack"),
                    format!("Each {} shoots {dir} at {} units per second every {} seconds within {} units, using \"{bomb}\".", e.kind.name(), num(r.shoot_speed), num(r.shoot_interval), num(r.shoot_range)),
                );
            }
        }
    }
    // pickups that animate
    for p in &d.pickups {
        let pick = art.get(&format!("pickup:{}", p.kind.name()));
        if full && !depth && pick.frames.len() > 1 {
            o.push(d.rv("juice"), format!("Animate each {} with {} at {} frames per second.", p.kind.name(), frames(&pick.frames), num(pick.fps)));
        }
    }
    if let Some(x) = d.exit {
        let pick = art.get("exit");
        if full && !depth && pick.frames.len() > 1 {
            o.push(d.rv("juice"), format!("Animate each {} with {} at {} frames per second.", x.name(), frames(&pick.frames), num(pick.fps)));
        }
    }

    // ---- touching things
    for p in &d.pickups {
        let k = p.kind.name();
        let mut acts = vec!["destroy it".to_string()];
        if count {
            acts.push("add 1 to the score".into());
        } else if p.value > 0 {
            acts.push(format!("add {} points", p.value));
        }
        if full {
            acts.push(if depth { "play the spin animation on the player".into() } else { "squash the player".into() });
        }
        acts.push(format!("play the {} sound", if k == "coin" { "coin" } else { "pickup" }));
        o.push(d.rv("pickups"), format!("When the player touches {}, {}.", a(k), join_and(&acts)));
    }
    if d.genre == Genre::Platformer {
        for e in &d.enemies {
            let mut acts = vec!["destroy it".to_string()];
            if !count {
                acts.push(format!("add {} points", e.score));
            }
            acts.push(format!("bounce the player up at {} units per second", num(r.stomp_bounce)));
            if full {
                acts.push("squash the player".into());
            }
            acts.push("play the bounce sound".into());
            o.push("genre/platformer (stomping)", format!("When the player lands on {}, {}.", a(e.kind.name()), join_and(&acts)));
        }
    }
    if d.player_shoots {
        for e in &d.enemies {
            o.push("abilities.player_bullet_speed", format!("When a shot hits {}, destroy both, add {} points and play the explosion sound.", a(e.kind.name()), e.score));
        }
        if d.is("layout", "shields") {
            o.push(d.rv("layout"), "When a shot hits a shield, destroy the shot.");
        }
    }
    for e in &d.enemies {
        if d.genre == Genre::Shooter {
            break;
        }
        o.push(if d.lives > 1 { d.rv("health") } else { "health/one_hit".into() }, format!("When the player touches {}, {hurt}.", a(e.kind.name())));
    }
    for h in &d.hazards {
        o.push(d.rv("hazards"), format!("When the player touches {}, {hurt}.", a(h.kind.name())));
    }
    if bomb.is_some() {
        if d.is("layout", "shields") {
            o.push(d.rv("layout"), "When a bomb hits a shield, destroy both.");
        }
        o.push(d.rv("enemy_attack"), format!("When a bomb hits the player, destroy the bomb and {hurt}."));
    }

    // ---- falling, goals
    if d.genre == Genre::Platformer {
        let fall = if d.lives > 1 { "the player loses a life and goes back to the start".to_string() } else { "the player loses".to_string() };
        o.push("layout.platformer.kill_y", format!("When the player falls below {}, {fall}.", num(lv.kill_y)));
    }
    if depth {
        o.push("layout.platformer.kill_y", format!("When the player falls below {}, the player goes back to the start.", num(lv.kill_y)));
    }
    let goal = d.rv("goal");
    match d.goal {
        Goal::ReachExit => {
            let x = d.exit.expect("reach_exit has an exit").name();
            o.push(goal, format!("When the player touches the {x}, the player wins \"LEVEL CLEAR\"."));
        }
        Goal::CollectAll => match d.exit {
            Some(x) => o.push(goal, format!("When the player touches the {} and no {} are left, the player wins \"LEVEL CLEAR\".", x.name(), pickups_word(d))),
            None => o.push(goal, format!("When no {} are left, the player wins \"ALL COLLECTED\".", pickups_word(d))),
        },
        Goal::Survive => {
            if d.waves > 1 {
                o.push(d.rv("ramp"), format!("Every {} seconds, start the next wave of {}.", num(cfg.enemy_rules.wave_every), enemies_word(d)));
            }
            o.push(goal, format!("When the time reaches {} seconds, the player wins \"YOU SURVIVED\".", d.survive));
        }
        Goal::DestroyAll => {
            let who = enemies_word(d);
            if d.waves > 1 {
                let per_wave: u32 = d.enemies.iter().map(|e| e.count * e.score).sum();
                let bombs = if bomb.is_some() { "destroy the bombs and " } else { "" };
                o.push(d.rv("ramp"), format!("When no {who} are left, {bombs}start the next wave of {who}."));
                o.push(goal, format!("When the score reaches {}, the player wins \"ALL WAVES CLEARED\".", per_wave * d.waves));
            } else {
                o.push(goal, format!("When no {who} are left, the player wins \"ALL CLEAR\"."));
            }
        }
    }
    if d.genre == Genre::Shooter {
        let who = if d.enemies.len() == 1 { d.enemies[0].kind.name().to_string() } else { "enemy".into() };
        o.push("layout.shooter.lose_y", format!("When {} falls below {}, the player loses \"INVADED\".", a(&who), num(lv.kill_y)));
    }
    o.push("every game restarts", "When the player presses r, restart the game.");
    o.0
}

fn frames(f: &[String]) -> String {
    let q: Vec<String> = f.iter().map(|x| format!("\"{x}\"")).collect();
    join_and(&q)
}

/// "a, b and c".
pub fn join_and(parts: &[String]) -> String {
    match parts.len() {
        0 => String::new(),
        1 => parts[0].clone(),
        n => format!("{} and {}", parts[..n - 1].join(", "), parts[n - 1]),
    }
}

pub fn text(lines: &[Line]) -> String {
    let mut s = String::new();
    for l in lines {
        s.push_str(&l.text);
        s.push('\n');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_read_well() {
        assert_eq!(num(7.0), "7");
        assert_eq!(num(7.5), "7.5");
        assert_eq!(num(2.345), "2.35");
        assert_eq!(num(-12.0), "-12");
        assert_eq!(num(-0.001), "0");
        assert_eq!(join_and(&["a".into(), "b".into(), "c".into()]), "a, b and c");
    }
}

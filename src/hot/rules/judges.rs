//! Judges look across several choices at once. Each fires only when every rule it looks at has
//! been decided, so partial solutions in the beam are scored fairly. Weights: rules.toml [judges].

use super::*;

/// Does any enemy end up chasing? (stated, or by the behaviour style and the kinds' defaults)
fn chases(cx: &Ctx, chosen: &Chosen) -> bool {
    let r = cx.req;
    if r.enemies.iter().flat_map(|e| e.behaviors.iter()).chain(r.generic_behaviors.iter()).any(|b| b.v == Behavior::Chase) {
        return true;
    }
    match chosen.get("enemy_behavior").copied() {
        Some("aggressive") => true,
        Some("by_kind") => {
            let mut kinds: Vec<&str> = r.enemies.iter().map(|e| e.kind.v.name()).collect();
            if r.generic_enemies.is_some() || r.enemies.is_empty() {
                kinds.push(cx.cfg.default_enemy[genre_of(chosen).name()].as_str());
            }
            kinds.iter().any(|k| cx.cfg.enemies[*k].behavior == "chase")
        }
        _ => false,
    }
}

/// Every judge that fires on these choices: (score, note).
pub fn score(cx: &Ctx, chosen: &Chosen) -> Vec<(f32, String)> {
    let is = |rule: &str, v: &str| chosen.get(rule).copied() == Some(v);
    let decided = |rule: &str| chosen.contains_key(rule);
    let j = |name: &str| cx.cfg.judge(name);
    let mut out = Vec::new();
    if decided("enemy_behavior") && is("controls", "momentum") && chases(cx, chosen) {
        out.push((j("chase_needs_direct_control"), "chasers against momentum controls".to_string()));
    }
    if is("pickups", "rewarding") && is("layout", "stacked") {
        out.push((j("rewarding_pickups_on_stacked"), "rewarding pickups on stacked platforms".into()));
    }
    if is("pickups", "trail") && is("layout", "flat_run") {
        out.push((j("trail_pickups_on_flat_run"), "a coin trail along a flat run".into()));
    }
    if is("hazards", "in_gaps") && is("controls", "snappy") {
        out.push((j("hazard_gaps_need_snappy"), "precise jumps over hazard gaps".into()));
    }
    if is("health", "one_hit") && decided("enemy_attack") {
        out.push((j("one_hit_with_shooting_enemies"), "bullets with one-hit deaths".into()));
    }
    if is("health", "lives") && is("ramp", "waves") {
        out.push((j("lives_with_waves"), "lives carry over between waves".into()));
    }
    if is("view", "depth") && is("layout", "open") {
        out.push((j("open_arena_in_depth"), "open arena keeps the 3d camera clear".into()));
    }
    if is("camera", "lookahead") && is("controls", "snappy") {
        out.push((j("lookahead_with_fast_controls"), "lookahead camera for fast running".into()));
    }
    if is("music", "calm") && is("enemy_behavior", "aggressive") {
        out.push((j("calm_music_with_aggressive"), "calm music under aggressive enemies".into()));
    }
    if is("juice", "full") && is("view", "depth") {
        out.push((j("full_juice_in_depth"), "animated 3d models".into()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::test_util::{cfg, req};
    use super::*;

    fn fired(sentence: &str, chosen: &[(&'static str, &'static str)]) -> Vec<String> {
        let c = cfg();
        let r = req(sentence);
        let cx = Ctx { req: &r, cfg: &c, profile: profile(&r, &c) };
        score(&cx, &chosen.iter().copied().collect()).into_iter().map(|(_, n)| n).collect()
    }

    #[test]
    fn chasers_and_momentum_clash() {
        let n = fired("make an arena where zombies chase the player", &[("genre", "arena"), ("controls", "momentum"), ("enemy_behavior", "by_kind")]);
        assert_eq!(n, vec!["chasers against momentum controls"]);
    }

    #[test]
    fn judges_wait_for_their_rules() {
        assert!(fired("make an arena where zombies chase the player", &[("genre", "arena"), ("controls", "momentum")]).is_empty());
    }
}

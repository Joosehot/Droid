//! Beam search over the design rules, in order. Each needed rule expands every partial solution
//! with every variant it offers; a variant scores `weight * (1 + axes . (profile - 0.5))`, plus
//! `search.evidence_bonus` when the sentence named it. Judges rescore the whole set of choices.
//! The best `search.beam_width` survive each step; ties break on the order variants were offered,
//! so the same sentence always gives the same result.

use super::rules::{self, Chosen, Ctx};
use std::cmp::Ordering;

/// One decision in a candidate, for explanations.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub rule: &'static str,
    pub variant: &'static str,
    pub score: f32,
    pub why: String,
    pub evidence: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Candidate {
    pub chosen: Chosen,
    pub steps: Vec<Step>,
    pub local: f32,
    pub judges: Vec<(f32, String)>,
    pub score: f32,
    /// Offer indices taken, for deterministic tie-breaks.
    key: Vec<u16>,
}

/// Everything the search weighed at one rule, for --explain.
#[derive(Clone, Debug)]
pub struct Weighed {
    pub rule: &'static str,
    pub why: String,
    /// (variant, score, evidence) as seen from the best partial solution.
    pub offers: Vec<(&'static str, f32, Option<String>)>,
}

pub struct Outcome {
    /// Complete candidates, best first (at most `search.finalists`).
    pub finalists: Vec<Candidate>,
    pub weighed: Vec<Weighed>,
    pub explored: usize,
}

/// Rules whose choices change the level geometry or the player's reach.
const STRUCTURAL: &[&str] = &["view", "layout", "controls", "pickups", "hazards"];

fn shape(c: &Candidate) -> Vec<Option<&'static str>> {
    STRUCTURAL.iter().map(|r| c.chosen.get(r).copied()).collect()
}

pub fn variant_score(cx: &Ctx, rule: &str, variant: &str, evidence: bool) -> f32 {
    let v = cx.cfg.variant(rule, variant);
    let dot: f32 = (0..3).map(|i| v.axes[i] * (cx.profile[i] - 0.5)).sum();
    v.weight * (1.0 + dot) + if evidence { cx.cfg.search.evidence_bonus } else { 0.0 }
}

fn order(a: &Candidate, b: &Candidate) -> Ordering {
    b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal).then_with(|| a.key.cmp(&b.key))
}

pub fn search(cx: &Ctx) -> Result<Outcome, String> {
    let mut beam = vec![Candidate { chosen: Chosen::new(), steps: vec![], local: 0.0, judges: vec![], score: 0.0, key: vec![] }];
    let mut weighed = Vec::new();
    let mut explored = 0;
    for rule in rules::all() {
        let mut next = Vec::new();
        let mut failure: Option<String> = None;
        let mut seen_weighed = false;
        for cand in &beam {
            let Some(why) = rule.needed(cx, &cand.chosen) else {
                next.push(cand.clone());
                continue;
            };
            let offers = match rule.offers(cx, &cand.chosen) {
                Ok(o) if !o.is_empty() => o,
                Ok(_) => {
                    failure.get_or_insert_with(|| format!("{}: nothing fits", rule.name()));
                    continue;
                }
                Err(e) => {
                    failure.get_or_insert(e);
                    continue;
                }
            };
            if !seen_weighed {
                weighed.push(Weighed {
                    rule: rule.name(),
                    why: why.clone(),
                    offers: offers.iter().map(|o| (o.variant, variant_score(cx, rule.name(), o.variant, o.evidence.is_some()), o.evidence.clone())).collect(),
                });
                seen_weighed = true;
            }
            for (i, o) in offers.iter().enumerate() {
                explored += 1;
                let s = variant_score(cx, rule.name(), o.variant, o.evidence.is_some());
                let mut c = cand.clone();
                c.chosen.insert(rule.name(), o.variant);
                c.steps.push(Step { rule: rule.name(), variant: o.variant, score: s, why: why.clone(), evidence: o.evidence.clone() });
                c.local += s;
                c.key.push(i as u16);
                c.judges = rules::judges::score(cx, &c.chosen);
                c.score = c.local + c.judges.iter().map(|(s, _)| s).sum::<f32>();
                next.push(c);
            }
        }
        if next.is_empty() {
            return Err(failure.unwrap_or_else(|| format!("{}: nothing fits", rule.name())));
        }
        next.sort_by(order);
        // keep the beam diverse: at most `per_shape` candidates share a level shape
        let mut per: std::collections::BTreeMap<Vec<Option<&str>>, usize> = Default::default();
        next.retain(|c| {
            let n = per.entry(shape(c)).or_insert(0);
            *n += 1;
            *n <= cx.cfg.search.per_shape
        });
        next.truncate(cx.cfg.search.beam_width);
        beam = next;
    }
    // Finalists must differ in something the checks can fail on (the level's shape and how the
    // player moves through it), so a failing finalist is followed by a real alternative.
    let mut seen = std::collections::BTreeSet::new();
    let mut finalists = Vec::new();
    for c in beam {
        if seen.insert(shape(&c)) {
            finalists.push(c);
        }
        if finalists.len() == cx.cfg.search.finalists {
            break;
        }
    }
    Ok(Outcome { finalists, weighed, explored })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hot::rules::test_util::{cfg, req};

    fn best(s: &str) -> Chosen {
        let c = cfg();
        let r = req(s);
        let cx = Ctx { req: &r, cfg: &c, profile: rules::profile(&r, &c) };
        search(&cx).unwrap_or_else(|e| panic!("{s}: {e}")).finalists.remove(0).chosen
    }

    #[test]
    fn deterministic() {
        let a = best("make a platformer with coins and slimes");
        for _ in 0..5 {
            assert_eq!(a, best("make a platformer with coins and slimes"));
        }
    }

    #[test]
    fn stated_things_win() {
        let c = best("make a shooter with 3 rows of aliens that shoot back");
        assert_eq!(c["genre"], "shooter");
        assert!(!c.contains_key("camera"));
        let c = best("make a 3d arena with zombies");
        assert_eq!(c["view"], "depth");
        let c = best("make a platformer with slimes in a night forest");
        assert_eq!(c["palette"], "dark");
    }

    #[test]
    fn genre_is_inferred_from_the_things() {
        assert_eq!(best("make a game with aliens")["genre"], "shooter");
        assert_eq!(best("make a game where zombies chase the player, survive 60 seconds")["genre"], "arena");
    }

    #[test]
    fn modifiers_change_the_choices() {
        let plain = best("make a platformer with coins and slimes");
        let hard = best("make a hard fast platformer with coins and slimes");
        let simple = best("make a simple easy slow platformer with coins and slimes");
        assert_ne!(plain, hard);
        assert_ne!(hard, simple);
    }

    #[test]
    fn impossible_requests_fail_with_a_reason() {
        let c = cfg();
        let r = req("make a 3d platformer");
        let cx = Ctx { req: &r, cfg: &c, profile: rules::profile(&r, &c) };
        let e = search(&cx).err().unwrap();
        assert!(e.contains("3d"), "{e}");
    }
}

//! How a round is won: collect everything, reach the exit, survive a time, or destroy every
//! enemy. A stated goal wins; otherwise the genre and the things present decide what is possible.

use super::*;

pub struct Goal;

impl Rule for Goal {
    fn name(&self) -> &'static str {
        "goal"
    }
    fn all_variants(&self) -> &'static [&'static str] {
        &["collect_all", "reach_exit", "survive", "destroy_all"]
    }
    fn needed(&self, cx: &Ctx, chosen: &Chosen) -> Option<String> {
        let _ = (cx, chosen);
        Some("every game can be won".into())
    }
    fn offers(&self, cx: &Ctx, chosen: &Chosen) -> Result<Vec<Offer>, String> {
        let _ = (cx, chosen);
        let r = cx.req;
        let genre = genre_of(chosen);
        let pickups = has_pickups(r, chosen);
        if let Some(g) = r.goals.last() {
            let (v, ok) = match g.v {
                GoalReq::CollectAll => ("collect_all", pickups),
                GoalReq::Reach => ("reach_exit", genre == Genre::Platformer),
                GoalReq::Survive(_) => ("survive", genre != Genre::Platformer && has_enemies(r)),
                GoalReq::DestroyAll => ("destroy_all", has_enemies(r)),
            };
            if !ok {
                return Err(format!("\"{}\" can't be the goal of this {}", g.words, genre.name()));
            }
            return Ok(vec![Offer::named(v, g.words.clone())]);
        }
        let mut out = Vec::new();
        match genre {
            Genre::Platformer => {
                if pickups {
                    out.push(Offer::plain("collect_all"));
                }
                out.push(match &r.exit {
                    Some(e) => Offer::named("reach_exit", e.words.clone()),
                    None => Offer::plain("reach_exit"),
                });
            }
            Genre::Arena => {
                if pickups {
                    out.push(Offer::plain("collect_all"));
                }
                if has_enemies(r) {
                    out.push(Offer::plain("survive"));
                }
                if r.has_ability(Ability::Shoot).is_some() && has_enemies(r) {
                    out.push(Offer::plain("destroy_all"));
                }
            }
            Genre::Shooter => out.push(Offer::plain("destroy_all")),
        }
        if out.is_empty() {
            return Err("nothing to do: add pickups to collect or enemies to survive".into());
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::offers;
    use super::*;

    #[test]
    fn a_stated_goal_wins() {
        assert_eq!(offers(&Goal, "make an arena with zombies, survive 60 seconds", &[("genre", "arena")]), Some(vec!["survive"]));
    }

    #[test]
    fn impossible_goal_is_refused() {
        assert_eq!(offers(&Goal, "make an arena with no enemies, survive 60 seconds", &[("genre", "arena")]), None);
    }

}

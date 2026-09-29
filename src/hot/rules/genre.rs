//! Which kind of game: platformer, arena or shooter. A named genre wins; otherwise the things
//! in the sentence are evidence (aliens and rows point to a shooter, jumping and exits to a
//! platformer, chasing and surviving to an arena).

use super::*;

pub struct Genre_;

impl Rule for Genre_ {
    fn name(&self) -> &'static str {
        "genre"
    }
    fn all_variants(&self) -> &'static [&'static str] {
        &["platformer", "arena", "shooter"]
    }
    fn needed(&self, cx: &Ctx, chosen: &Chosen) -> Option<String> {
        let _ = (cx, chosen);
        Some("every game has a genre".into())
    }
    fn offers(&self, cx: &Ctx, chosen: &Chosen) -> Result<Vec<Offer>, String> {
        let _ = (cx, chosen);
        let r = cx.req;
        if let Some(g) = &r.genre {
            if r.view.as_ref().is_some_and(|v| v.v == ViewReq::Depth) && g.v != Genre::Arena {
                return Err(format!("a 3d {} isn't supported yet; 3d works for arenas (\"make a 3d arena ...\")", g.v.name()));
            }
            return Ok(vec![Offer::named(g.v.name(), g.words.clone())]);
        }
        if let Some(v) = r.view.as_ref().filter(|v| v.v == ViewReq::Depth) {
            return Ok(vec![Offer::named("arena", v.words.clone())]);
        }
        let shooter = r
            .enemies
            .iter()
            .find(|e| e.kind.v == EnemyKind::Alien || e.rows.is_some() || e.behaviors.iter().any(|b| b.v == Behavior::March))
            .map(|e| e.kind.words.clone())
            .or_else(|| r.player.as_ref().filter(|p| p.v == PlayerKind::Ship).map(|p| p.words.clone()));
        let platformer = r
            .abilities
            .iter()
            .find(|a| matches!(a.v, Ability::Jump | Ability::DoubleJump))
            .map(|a| a.words.clone())
            .or_else(|| r.exit.as_ref().map(|e| e.words.clone()));
        let arena = r
            .goal(|g| matches!(g, GoalReq::Survive(_)))
            .map(|g| g.words.clone())
            .or_else(|| r.enemies.iter().flat_map(|e| e.behaviors.iter()).find(|b| matches!(b.v, Behavior::Chase | Behavior::Wander)).map(|b| b.words.clone()))
            .or_else(|| r.generic_behaviors.iter().find(|b| matches!(b.v, Behavior::Chase | Behavior::Wander)).map(|b| b.words.clone()));
        Ok(vec![
            Offer { variant: "platformer", evidence: platformer },
            Offer { variant: "arena", evidence: arena },
            Offer { variant: "shooter", evidence: shooter },
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::offers;
    use super::*;

    #[test]
    fn a_named_genre_is_the_only_offer() {
        assert_eq!(offers(&Genre_, "make a shooter", &[]), Some(vec!["shooter"]));
    }

    #[test]
    fn a_3d_platformer_is_refused() {
        assert_eq!(offers(&Genre_, "make a 3d platformer", &[]), None);
    }

}

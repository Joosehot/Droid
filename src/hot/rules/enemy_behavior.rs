//! How enemies without a stated behaviour act: as their kind usually does, more aggressively
//! (they chase), or calmer (they patrol and wander). Stated behaviours always win. A shooter's
//! fleet always marches, so this doesn't apply there.

use super::*;

pub struct EnemyBehavior;

impl Rule for EnemyBehavior {
    fn name(&self) -> &'static str {
        "enemy_behavior"
    }
    fn all_variants(&self) -> &'static [&'static str] {
        &["by_kind", "aggressive", "calm"]
    }
    fn needed(&self, cx: &Ctx, chosen: &Chosen) -> Option<String> {
        let _ = (cx, chosen);
        (has_enemies(cx.req) && genre_of(chosen) != Genre::Shooter).then(|| "the game has enemies".into())
    }
    fn offers(&self, cx: &Ctx, chosen: &Chosen) -> Result<Vec<Offer>, String> {
        let _ = (cx, chosen);
        Ok(vec![Offer::plain("by_kind"), Offer::plain("aggressive"), Offer::plain("calm")])
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::offers;
    use super::*;

    #[test]
    fn applies_with_enemies() {
        assert!(offers(&EnemyBehavior, "make a platformer with slimes", &[("genre", "platformer")]).is_some());
    }

    #[test]
    fn not_without_enemies() {
        assert_eq!(offers(&EnemyBehavior, "make a platformer with no enemies", &[("genre", "platformer")]), None);
    }

}

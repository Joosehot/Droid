//! How shooting enemies aim: straight or at the player. Only when enemies shoot (always for a
//! shooter's fleet).

use super::*;

pub struct EnemyAttack;

impl Rule for EnemyAttack {
    fn name(&self) -> &'static str {
        "enemy_attack"
    }
    fn all_variants(&self) -> &'static [&'static str] {
        &["straight", "aimed"]
    }
    fn needed(&self, cx: &Ctx, chosen: &Chosen) -> Option<String> {
        let _ = (cx, chosen);
        if !has_enemies(cx.req) {
            return None;
        }
        cx.req
            .any_enemy_shoots()
            .or_else(|| cx.req.generic_shoots.as_ref().map(|s| s.words.clone()))
            .or_else(|| (genre_of(chosen) == Genre::Shooter).then(|| "a shooter's fleet fires back".to_string()))
    }
    fn offers(&self, cx: &Ctx, chosen: &Chosen) -> Result<Vec<Offer>, String> {
        let _ = (cx, chosen);
        Ok(vec![Offer::plain("straight"), Offer::plain("aimed")])
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::offers;
    use super::*;

    #[test]
    fn applies_when_enemies_shoot() {
        assert!(offers(&EnemyAttack, "make an arena where ghosts shoot", &[("genre", "arena")]).is_some());
    }

    #[test]
    fn not_when_nobody_shoots() {
        assert_eq!(offers(&EnemyAttack, "make an arena with zombies", &[("genre", "arena")]), None);
    }

}

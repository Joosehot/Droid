//! One hit ends the round, or the player has lives and a moment of safety after each hit.

use super::*;

pub struct Health;

impl Rule for Health {
    fn name(&self) -> &'static str {
        "health"
    }
    fn all_variants(&self) -> &'static [&'static str] {
        &["one_hit", "lives"]
    }
    fn needed(&self, cx: &Ctx, chosen: &Chosen) -> Option<String> {
        let _ = (cx, chosen);
        (has_enemies(cx.req) || !cx.req.hazards.is_empty()).then(|| "something can hurt the player".into())
    }
    fn offers(&self, cx: &Ctx, chosen: &Chosen) -> Result<Vec<Offer>, String> {
        let _ = (cx, chosen);
        Ok(match &cx.req.lives {
            Some(l) if l.v == 1 => vec![Offer::named("one_hit", l.words.clone())],
            Some(l) => vec![Offer::named("lives", l.words.clone())],
            None => vec![Offer::plain("one_hit"), Offer::plain("lives")],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::offers;
    use super::*;

    #[test]
    fn stated_lives() {
        assert_eq!(offers(&Health, "make a platformer with slimes and 3 lives", &[("genre", "platformer")]), Some(vec!["lives"]));
    }

    #[test]
    fn nothing_can_hurt() {
        assert_eq!(offers(&Health, "make a platformer with no enemies", &[("genre", "platformer")]), None);
    }

}

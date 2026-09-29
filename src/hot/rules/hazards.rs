//! Where hazards go: in gaps the player must jump, or standing on the floor.

use super::*;

pub struct Hazards;

impl Rule for Hazards {
    fn name(&self) -> &'static str {
        "hazards"
    }
    fn all_variants(&self) -> &'static [&'static str] {
        &["in_gaps", "on_floor"]
    }
    fn needed(&self, cx: &Ctx, chosen: &Chosen) -> Option<String> {
        let _ = (cx, chosen);
        cx.req.hazards.first().map(|h| h.kind.words.clone())
    }
    fn offers(&self, cx: &Ctx, chosen: &Chosen) -> Result<Vec<Offer>, String> {
        let _ = (cx, chosen);
        match genre_of(chosen) {
            Genre::Platformer => Ok(vec![Offer::plain("in_gaps"), Offer::plain("on_floor")]),
            Genre::Arena => Ok(vec![Offer::plain("on_floor")]),
            Genre::Shooter => Err(format!("\"{}\": shooters have no floor for hazards", cx.req.hazards[0].kind.words)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::offers;
    use super::*;

    #[test]
    fn applies_with_spikes() {
        assert_eq!(offers(&Hazards, "make a platformer with spikes", &[("genre", "platformer")]), Some(vec!["in_gaps", "on_floor"]));
    }

    #[test]
    fn not_without_hazards() {
        assert_eq!(offers(&Hazards, "make a platformer", &[("genre", "platformer")]), None);
    }

}

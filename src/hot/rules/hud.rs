//! Where the score, lives and timer are shown.

use super::*;

pub struct Hud;

impl Rule for Hud {
    fn name(&self) -> &'static str {
        "hud"
    }
    fn all_variants(&self) -> &'static [&'static str] {
        &["corner", "top_bar"]
    }
    fn needed(&self, cx: &Ctx, chosen: &Chosen) -> Option<String> {
        let _ = (cx, chosen);
        Some("the player needs to see the score".into())
    }
    fn offers(&self, cx: &Ctx, chosen: &Chosen) -> Result<Vec<Offer>, String> {
        let _ = (cx, chosen);
        Ok(vec![Offer::plain("corner"), Offer::plain("top_bar")])
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::offers;
    use super::*;

    #[test]
    fn two_places() {
        assert_eq!(offers(&Hud, "make a shooter", &[("genre", "shooter")]), Some(vec!["corner", "top_bar"]));
    }

    #[test]
    fn every_genre_has_one() {
        assert!(offers(&Hud, "make a platformer", &[("genre", "platformer")]).is_some());
    }

}

//! Feedback: sounds only, or sounds plus squash, blinking and animations.

use super::*;

pub struct Juice;

impl Rule for Juice {
    fn name(&self) -> &'static str {
        "juice"
    }
    fn all_variants(&self) -> &'static [&'static str] {
        &["minimal", "full"]
    }
    fn needed(&self, cx: &Ctx, chosen: &Chosen) -> Option<String> {
        let _ = (cx, chosen);
        Some("actions need feedback".into())
    }
    fn offers(&self, cx: &Ctx, chosen: &Chosen) -> Result<Vec<Offer>, String> {
        let _ = (cx, chosen);
        Ok(vec![Offer::plain("minimal"), Offer::plain("full")])
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::offers;
    use super::*;

    #[test]
    fn two_levels() {
        assert_eq!(offers(&Juice, "make a platformer", &[("genre", "platformer")]), Some(vec!["minimal", "full"]));
    }

    #[test]
    fn simple_games_still_decide() {
        assert!(offers(&Juice, "make a simple arena with zombies", &[("genre", "arena")]).is_some());
    }

}

//! The music loop: bouncy, driving or calm (notes and tempo in rules.toml [music.*]).

use super::*;

pub struct Music;

impl Rule for Music {
    fn name(&self) -> &'static str {
        "music"
    }
    fn all_variants(&self) -> &'static [&'static str] {
        &["bounce", "drive", "calm"]
    }
    fn needed(&self, cx: &Ctx, chosen: &Chosen) -> Option<String> {
        let _ = (cx, chosen);
        Some("every game has a music loop".into())
    }
    fn offers(&self, cx: &Ctx, chosen: &Chosen) -> Result<Vec<Offer>, String> {
        let _ = (cx, chosen);
        Ok(vec![Offer::plain("bounce"), Offer::plain("drive"), Offer::plain("calm")])
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::offers;
    use super::*;

    #[test]
    fn three_moods() {
        assert_eq!(offers(&Music, "make a platformer", &[("genre", "platformer")]), Some(vec!["bounce", "drive", "calm"]));
    }

    #[test]
    fn needed_for_every_genre() {
        assert!(offers(&Music, "make a shooter", &[("genre", "shooter")]).is_some());
    }

}

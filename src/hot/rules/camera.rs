//! Where the camera looks. Shooters use a fixed screen, so this rule doesn't apply to them.

use super::*;

pub struct Camera;

impl Rule for Camera {
    fn name(&self) -> &'static str {
        "camera"
    }
    fn all_variants(&self) -> &'static [&'static str] {
        &["follow", "lookahead", "fixed", "chase_cam", "high_cam"]
    }
    fn needed(&self, cx: &Ctx, chosen: &Chosen) -> Option<String> {
        let _ = (cx, chosen);
        (genre_of(chosen) != Genre::Shooter).then(|| "the level is bigger than the screen or seen in 3d".into())
    }
    fn offers(&self, cx: &Ctx, chosen: &Chosen) -> Result<Vec<Offer>, String> {
        let _ = (cx, chosen);
        let bigger = cx.req.edit_steps(Target::Level, Change::Bigger, Change::Smaller) > 0;
        Ok(match genre_of(chosen) {
            Genre::Platformer => vec![Offer::plain("follow"), Offer::plain("lookahead")],
            Genre::Arena if depth(chosen) => vec![Offer::plain("chase_cam"), Offer::plain("high_cam")],
            // a bigger arena no longer fits one screen
            Genre::Arena if bigger => vec![Offer::named("follow", cx.req.edit_words(Target::Level))],
            Genre::Arena => vec![Offer::plain("fixed"), Offer::plain("follow")],
            Genre::Shooter => unreachable!("not needed for shooters"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::offers;
    use super::*;

    #[test]
    fn applies_to_platformers() {
        assert_eq!(offers(&Camera, "make a platformer", &[("genre", "platformer")]), Some(vec!["follow", "lookahead"]));
    }

    #[test]
    fn not_for_shooters() {
        assert_eq!(offers(&Camera, "make a shooter", &[("genre", "shooter")]), None);
    }

}

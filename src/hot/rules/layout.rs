//! The shape of the level: for platformers how the ground and platforms climb, for arenas what
//! stands in the way, for shooters whether shields protect the player.

use super::*;

pub struct Layout;

impl Rule for Layout {
    fn name(&self) -> &'static str {
        "layout"
    }
    fn all_variants(&self) -> &'static [&'static str] {
        &["flat_run", "steps", "stacked", "open", "pillars", "rooms", "open_sky", "shields"]
    }
    fn needed(&self, cx: &Ctx, chosen: &Chosen) -> Option<String> {
        let _ = (cx, chosen);
        Some("every game has a level".into())
    }
    fn offers(&self, cx: &Ctx, chosen: &Chosen) -> Result<Vec<Offer>, String> {
        let _ = (cx, chosen);
        Ok(match genre_of(chosen) {
            Genre::Platformer => vec![Offer::plain("flat_run"), Offer::plain("steps"), Offer::plain("stacked")],
            // inner walls hide the player from a 3D camera
            Genre::Arena if depth(chosen) => vec![Offer::plain("open"), Offer::plain("pillars")],
            Genre::Arena => vec![Offer::plain("open"), Offer::plain("pillars"), Offer::plain("rooms")],
            Genre::Shooter => vec![Offer::plain("open_sky"), Offer::plain("shields")],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::offers;
    use super::*;

    #[test]
    fn platformer_layouts() {
        assert_eq!(offers(&Layout, "make a platformer", &[("genre", "platformer")]), Some(vec!["flat_run", "steps", "stacked"]));
    }

    #[test]
    fn no_rooms_in_3d() {
        assert_eq!(offers(&Layout, "make a 3d arena", &[("genre", "arena"), ("view", "depth")]), Some(vec!["open", "pillars"]));
    }

}

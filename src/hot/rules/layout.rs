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
        let _ = cx;
        let all = match genre_of(chosen) {
            Genre::Platformer => vec![Offer::plain("flat_run"), Offer::plain("steps"), Offer::plain("stacked")],
            // inner walls hide the player from a 3D camera
            Genre::Arena if depth(chosen) => vec![Offer::plain("open"), Offer::plain("pillars")],
            Genre::Arena => vec![Offer::plain("open"), Offer::plain("pillars"), Offer::plain("rooms")],
            Genre::Shooter => vec![Offer::plain("open_sky"), Offer::plain("shields")],
        };
        // a named shape wins, if this kind of game has it
        match &cx.req.layout {
            Some(l) => match all.iter().find(|o| o.variant == l.v.name()) {
                Some(o) => Ok(vec![Offer::named(o.variant, l.words.clone())]),
                None => Err(format!(
                    "\"{}\": a {}{} has no {}; it can have {}",
                    l.words,
                    if depth(chosen) { "3d " } else { "" },
                    genre_of(chosen).name(),
                    l.words,
                    all.iter().map(|o| o.variant.replace('_', " ")).collect::<Vec<_>>().join(", ")
                )),
            },
            None => Ok(all),
        }
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
    fn a_named_shape_wins() {
        assert_eq!(offers(&Layout, "make an arena with rooms", &[("genre", "arena"), ("view", "flat")]), Some(vec!["rooms"]));
        assert_eq!(offers(&Layout, "make a platformer with rooms", &[("genre", "platformer")]), None);
    }

    #[test]
    fn no_rooms_in_3d() {
        assert_eq!(offers(&Layout, "make a 3d arena", &[("genre", "arena"), ("view", "depth")]), Some(vec!["open", "pillars"]));
    }

}

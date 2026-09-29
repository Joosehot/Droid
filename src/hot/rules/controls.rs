//! How the player moves: snappy or floaty jumps, direct or momentum steering, a sliding or
//! drifting ship. The numbers for each live in rules.toml [controls.*].

use super::*;

pub struct Controls;

impl Rule for Controls {
    fn name(&self) -> &'static str {
        "controls"
    }
    fn all_variants(&self) -> &'static [&'static str] {
        &["snappy", "floaty", "direct", "momentum", "slide", "drift"]
    }
    fn needed(&self, cx: &Ctx, chosen: &Chosen) -> Option<String> {
        let _ = (cx, chosen);
        Some("the player has to move".into())
    }
    fn offers(&self, cx: &Ctx, chosen: &Chosen) -> Result<Vec<Offer>, String> {
        let _ = (cx, chosen);
        Ok(match genre_of(chosen) {
            Genre::Platformer => vec![Offer::plain("snappy"), Offer::plain("floaty")],
            Genre::Arena => vec![Offer::plain("direct"), Offer::plain("momentum")],
            Genre::Shooter => vec![Offer::plain("slide"), Offer::plain("drift")],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::offers;
    use super::*;

    #[test]
    fn platformer_controls() {
        assert_eq!(offers(&Controls, "make a platformer", &[("genre", "platformer")]), Some(vec!["snappy", "floaty"]));
    }

    #[test]
    fn ship_controls() {
        assert_eq!(offers(&Controls, "make a shooter", &[("genre", "shooter")]), Some(vec!["slide", "drift"]));
    }

}

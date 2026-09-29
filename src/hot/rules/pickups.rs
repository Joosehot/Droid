//! Where pickups go: in trails that show the way, scattered, or where they reward risk (high
//! platforms, corners).

use super::*;

pub struct Pickups;

impl Rule for Pickups {
    fn name(&self) -> &'static str {
        "pickups"
    }
    fn all_variants(&self) -> &'static [&'static str] {
        &["trail", "scattered", "rewarding"]
    }
    fn needed(&self, cx: &Ctx, chosen: &Chosen) -> Option<String> {
        let _ = (cx, chosen);
        has_pickups(cx.req, chosen).then(|| "the game has pickups".into())
    }
    fn offers(&self, cx: &Ctx, chosen: &Chosen) -> Result<Vec<Offer>, String> {
        let _ = (cx, chosen);
        Ok(match genre_of(chosen) {
            Genre::Platformer => vec![Offer::plain("trail"), Offer::plain("scattered"), Offer::plain("rewarding")],
            Genre::Arena => vec![Offer::plain("scattered"), Offer::plain("rewarding")],
            Genre::Shooter => vec![Offer::plain("scattered")],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::offers;
    use super::*;

    #[test]
    fn applies_to_platformers() {
        assert_eq!(offers(&Pickups, "make a platformer", &[("genre", "platformer")]), Some(vec!["trail", "scattered", "rewarding"]));
    }

    #[test]
    fn not_in_a_plain_shooter() {
        assert_eq!(offers(&Pickups, "make a shooter", &[("genre", "shooter")]), None);
    }

}

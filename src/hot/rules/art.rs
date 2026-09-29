//! Which family of library art draws the game: pixel or vector sprites in 2D, low-poly models
//! in 3D. The asset library tags every asset with a `style`; the build step picks, for each
//! kind, an asset of the chosen style (falling back to any style when that kind has none).

use super::*;

pub struct Art;

impl Rule for Art {
    fn name(&self) -> &'static str {
        "art"
    }
    fn all_variants(&self) -> &'static [&'static str] {
        &["pixel", "vector", "lowpoly"]
    }
    fn needed(&self, cx: &Ctx, chosen: &Chosen) -> Option<String> {
        let _ = (cx, chosen);
        Some("everything is drawn with art from the asset library".into())
    }
    fn offers(&self, cx: &Ctx, chosen: &Chosen) -> Result<Vec<Offer>, String> {
        let style = cx.req.style.as_ref();
        if depth(chosen) {
            return match style {
                Some(s) if s.v != ArtStyle::Lowpoly => Err(format!("\"{}\": 3d games are drawn with low-poly models", s.words)),
                Some(s) => Ok(vec![Offer::named("lowpoly", s.words.clone())]),
                None => Ok(vec![Offer::plain("lowpoly")]),
            };
        }
        match style {
            Some(s) if s.v == ArtStyle::Lowpoly => Err(format!("\"{}\": low-poly models need a 3d game (\"make a 3d arena ...\")", s.words)),
            Some(s) => Ok(vec![Offer::named(s.v.name(), s.words.clone())]),
            None => Ok(vec![Offer::plain("pixel"), Offer::plain("vector")]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_util::offers;
    use super::*;

    #[test]
    fn flat_games_choose_pixel_or_vector() {
        assert_eq!(offers(&Art, "make a platformer", &[("genre", "platformer"), ("view", "flat")]).unwrap(), vec!["pixel", "vector"]);
        assert_eq!(offers(&Art, "make a pixel platformer", &[("genre", "platformer"), ("view", "flat")]).unwrap(), vec!["pixel"]);
    }

    #[test]
    fn depth_is_lowpoly_only() {
        assert_eq!(offers(&Art, "make a 3d arena", &[("genre", "arena"), ("view", "depth")]).unwrap(), vec!["lowpoly"]);
        assert!(offers(&Art, "make a pixel 3d arena", &[("genre", "arena"), ("view", "depth")]).is_none());
    }
}

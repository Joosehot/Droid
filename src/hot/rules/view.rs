//! Flat 2D or 3D with depth. Only arenas have a 3D version in v0.

use super::*;

pub struct View;

impl Rule for View {
    fn name(&self) -> &'static str {
        "view"
    }
    fn all_variants(&self) -> &'static [&'static str] {
        &["flat", "depth"]
    }
    fn needed(&self, cx: &Ctx, chosen: &Chosen) -> Option<String> {
        let _ = (cx, chosen);
        Some("every game is 2d or 3d".into())
    }
    fn offers(&self, cx: &Ctx, chosen: &Chosen) -> Result<Vec<Offer>, String> {
        let _ = (cx, chosen);
        let arena = genre_of(chosen) == Genre::Arena;
        match &cx.req.view {
            Some(v) if v.v == ViewReq::Depth && !arena => Err(format!("\"{}\": only arenas have a 3d version", v.words)),
            Some(v) if v.v == ViewReq::Depth => Ok(vec![Offer::named("depth", v.words.clone())]),
            Some(v) => Ok(vec![Offer::named("flat", v.words.clone())]),
            // "top-down" names the flat view outright
            None if arena => match cx.req.genre.as_ref().filter(|g| g.words.to_lowercase().starts_with("top")) {
                Some(g) => Ok(vec![Offer::named("flat", g.words.clone())]),
                None => Ok(vec![Offer::plain("flat"), Offer::plain("depth")]),
            },
            None => Ok(vec![Offer::plain("flat")]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::offers;
    use super::*;

    #[test]
    fn arenas_may_be_3d() {
        assert_eq!(offers(&View, "make an arena", &[("genre", "arena")]), Some(vec!["flat", "depth"]));
    }

    #[test]
    fn top_down_is_flat() {
        assert_eq!(offers(&View, "make a top-down arena", &[("genre", "arena")]), Some(vec!["flat"]));
    }

    #[test]
    fn shooters_are_flat() {
        assert_eq!(offers(&View, "make a shooter", &[("genre", "shooter")]), Some(vec!["flat"]));
    }

}

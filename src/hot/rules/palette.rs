//! The mood of the colours: bright, muted or dark. "night" asks for dark, "day" for bright.

use super::*;

pub struct Palette;

impl Rule for Palette {
    fn name(&self) -> &'static str {
        "palette"
    }
    fn all_variants(&self) -> &'static [&'static str] {
        &["bright", "muted", "dark"]
    }
    fn needed(&self, cx: &Ctx, chosen: &Chosen) -> Option<String> {
        let _ = (cx, chosen);
        Some("everything has a colour".into())
    }
    fn offers(&self, cx: &Ctx, chosen: &Chosen) -> Result<Vec<Offer>, String> {
        let _ = (cx, chosen);
        Ok(match &cx.req.time {
            Some(t) if t.v == TimeOfDay::Night => vec![Offer::named("dark", t.words.clone())],
            Some(t) => vec![Offer::named("bright", t.words.clone())],
            None => vec![Offer::plain("bright"), Offer::plain("muted"), Offer::plain("dark")],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::offers;
    use super::*;

    #[test]
    fn night_is_dark() {
        assert_eq!(offers(&Palette, "make a platformer in a night forest", &[("genre", "platformer")]), Some(vec!["dark"]));
    }

    #[test]
    fn otherwise_all_moods() {
        assert_eq!(offers(&Palette, "make a platformer", &[("genre", "platformer")]), Some(vec!["bright", "muted", "dark"]));
    }

}

//! How difficulty grows: steady, in waves, or speeding up over time. Platformer levels are
//! shaped by hand, so this only applies to arenas and shooters.

use super::*;

pub struct Ramp;

impl Rule for Ramp {
    fn name(&self) -> &'static str {
        "ramp"
    }
    fn all_variants(&self) -> &'static [&'static str] {
        &["steady", "waves", "speedup"]
    }
    fn needed(&self, cx: &Ctx, chosen: &Chosen) -> Option<String> {
        let _ = (cx, chosen);
        (has_enemies(cx.req) && genre_of(chosen) != Genre::Platformer).then(|| "enemies keep coming".into())
    }
    fn offers(&self, cx: &Ctx, chosen: &Chosen) -> Result<Vec<Offer>, String> {
        let _ = (cx, chosen);
        Ok(match (genre_of(chosen), chosen.get("goal").copied()) {
            // new waves come when the field is cleared, or on a timer while surviving
            (Genre::Shooter, _) | (_, Some("destroy_all")) | (_, Some("survive")) => vec![Offer::plain("steady"), Offer::plain("waves"), Offer::plain("speedup")],
            _ => vec![Offer::plain("steady"), Offer::plain("speedup")],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::offers;
    use super::*;

    #[test]
    fn shooters_ramp() {
        assert_eq!(offers(&Ramp, "make a shooter", &[("genre", "shooter"), ("goal", "destroy_all")]), Some(vec!["steady", "waves", "speedup"]));
    }

    #[test]
    fn survival_waves_and_collecting_without() {
        assert_eq!(offers(&Ramp, "make an arena with zombies", &[("genre", "arena"), ("goal", "survive")]), Some(vec!["steady", "waves", "speedup"]));
        assert_eq!(offers(&Ramp, "make an arena with zombies", &[("genre", "arena"), ("goal", "collect_all")]), Some(vec!["steady", "speedup"]));
    }

    #[test]
    fn not_for_platformers() {
        assert_eq!(offers(&Ramp, "make a platformer with slimes", &[("genre", "platformer")]), None);
    }

}

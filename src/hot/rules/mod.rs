//! Design rules: one decision per file. A rule says whether the request needs its decision,
//! which variants are allowed (and which the sentence named outright), and nothing else. How a
//! chosen variant turns into level geometry, art and behaviour sentences is the build step's job.
//! Every weight and axis lives in rules.toml `[variants.<rule>.<variant>]`.

use super::config::Config;
use super::request::*;
use std::collections::BTreeMap;

pub mod camera;
pub mod controls;
pub mod enemy_attack;
pub mod enemy_behavior;
pub mod genre;
pub mod goal;
pub mod hazards;
pub mod health;
pub mod hud;
pub mod judges;
pub mod juice;
pub mod layout;
pub mod music;
pub mod palette;
pub mod pickups;
pub mod ramp;
pub mod view;

/// What the search knows while deciding.
pub struct Ctx<'a> {
    pub req: &'a Request,
    pub cfg: &'a Config,
    /// [challenge, pace, simplicity], each 0..1.
    pub profile: [f32; 3],
}

/// Decisions made so far: rule -> variant.
pub type Chosen = BTreeMap<&'static str, &'static str>;

/// One allowed variant, with the words that named it outright (if any).
#[derive(Clone, Debug, PartialEq)]
pub struct Offer {
    pub variant: &'static str,
    pub evidence: Option<String>,
}

impl Offer {
    pub fn plain(variant: &'static str) -> Offer {
        Offer { variant, evidence: None }
    }
    pub fn named(variant: &'static str, words: impl Into<String>) -> Offer {
        Offer { variant, evidence: Some(words.into()) }
    }
}

pub trait Rule: Sync {
    fn name(&self) -> &'static str;
    fn all_variants(&self) -> &'static [&'static str];
    /// Some(why) when this request needs the decision, None when it doesn't apply.
    fn needed(&self, cx: &Ctx, chosen: &Chosen) -> Option<String>;
    /// The variants allowed here. Err explains why nothing fits (an honest failure).
    fn offers(&self, cx: &Ctx, chosen: &Chosen) -> Result<Vec<Offer>, String>;
}

/// The rules in decision order (earlier choices constrain later ones).
pub fn all() -> Vec<&'static dyn Rule> {
    vec![
        &genre::Genre_,
        &view::View,
        &layout::Layout,
        &controls::Controls,
        &camera::Camera,
        &enemy_behavior::EnemyBehavior,
        &enemy_attack::EnemyAttack,
        &pickups::Pickups,
        &hazards::Hazards,
        &goal::Goal,
        &health::Health,
        &ramp::Ramp,
        &palette::Palette,
        &music::Music,
        &juice::Juice,
        &hud::Hud,
    ]
}

/// Profile from rules.toml defaults shifted by the sentence's hard/easy/fast/slow/simple words
/// (and "faster"/"slower" edits on nothing in particular), clamped to 0..1.
pub fn profile(req: &Request, cfg: &Config) -> [f32; 3] {
    let mut p = cfg.profile.default;
    for m in &req.mods {
        let s = cfg.profile.shift[m.v.name()];
        for i in 0..3 {
            p[i] += s[i];
        }
    }
    p.map(|x| x.clamp(0.0, 1.0))
}

/// The genre chosen so far (genre is decided first, so later rules can rely on it).
pub fn genre_of(chosen: &Chosen) -> Genre {
    match chosen.get("genre").copied() {
        Some("arena") => Genre::Arena,
        Some("shooter") => Genre::Shooter,
        _ => Genre::Platformer,
    }
}

pub fn depth(chosen: &Chosen) -> bool {
    chosen.get("view") == Some(&"depth")
}

/// Any enemies at all, after "no enemies" and the genre default.
pub fn has_enemies(req: &Request) -> bool {
    req.no_enemies.is_none()
}

pub fn has_pickups(req: &Request, chosen: &Chosen) -> bool {
    if req.no_pickups.is_some() {
        return false;
    }
    // shooters have no pickups unless the sentence asks for them
    genre_of(chosen) != Genre::Shooter || !req.pickups.is_empty() || req.generic_pickups.is_some()
}

#[cfg(test)]
pub mod test_util {
    use super::*;

    pub fn cfg() -> Config {
        Config::load(None).expect("built-in rules.toml loads")
    }

    pub fn req(s: &str) -> Request {
        super::super::parser::parse(s, false).unwrap_or_else(|d| panic!("{s:?}: {d:?}"))
    }

    /// Offers of `rule` for `sentence`, with `chosen` decisions already made.
    pub fn offers(rule: &dyn Rule, sentence: &str, chosen: &[(&'static str, &'static str)]) -> Option<Vec<&'static str>> {
        let cfg = cfg();
        let r = req(sentence);
        let cx = Ctx { req: &r, cfg: &cfg, profile: profile(&r, &cfg) };
        let ch: Chosen = chosen.iter().copied().collect();
        rule.needed(&cx, &ch)?;
        Some(rule.offers(&cx, &ch).ok()?.into_iter().map(|o| o.variant).collect())
    }
}

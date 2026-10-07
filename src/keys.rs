//! Scripted key presses for headless checks and demos: `DROID_KEYS`.
//!
//! A comma-separated list of `key@seconds` (a tap: the key goes down at that game time and up
//! [`TAP_SECS`] later) or `key@from-to` (held from one time to the other), read against the game
//! clock like `DROID_VOICE`: `DROID_KEYS="up@2,up@2.6,space@8-9.5"`. It works in the window too
//! (real keys keep working beside it), so a demo can be driven the same way as a check.

use crate::world::{Input, World};
use std::sync::OnceLock;

/// How long a tap holds its key down (seconds).
pub const TAP_SECS: f32 = 0.1;

/// One scripted press: the key, when it goes down and when it comes up again.
#[derive(Clone, Debug, PartialEq)]
pub struct Press {
    pub key: String,
    pub from: f32,
    pub to: f32,
}

/// `"up@2,up@2.6,space@8-9.5"` -> presses.
pub fn parse(s: &str) -> Result<Vec<Press>, String> {
    let mut out = Vec::new();
    for part in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (key, when) = part.split_once('@').ok_or_else(|| format!("DROID_KEYS: \"{part}\" is not key@seconds"))?;
        let key = key.trim().to_lowercase();
        if key.is_empty() {
            return Err(format!("DROID_KEYS: \"{part}\" has no key"));
        }
        let num = |t: &str| t.trim().parse::<f32>().map_err(|_| format!("DROID_KEYS: \"{t}\" is not seconds"));
        let (from, to) = match when.split_once('-') {
            Some((a, b)) => (num(a)?, num(b)?),
            None => {
                let a = num(when)?;
                (a, a + TAP_SECS)
            }
        };
        if !(to > from) {
            return Err(format!("DROID_KEYS: \"{part}\" ends before it starts"));
        }
        out.push(Press { key, from, to });
    }
    Ok(out)
}

fn script() -> &'static [Press] {
    static S: OnceLock<Vec<Press>> = OnceLock::new();
    S.get_or_init(|| match std::env::var("DROID_KEYS") {
        Ok(v) => parse(&v).unwrap_or_else(|e| {
            eprintln!("droid: {e}; no scripted keys");
            Vec::new()
        }),
        Err(_) => Vec::new(),
    })
}

/// Apply the presses whose edges fall in (`t0`, `t1`] to the input: down and pressed when one
/// starts, up and released when it ends.
pub fn apply(presses: &[Press], input: &mut Input, t0: f32, t1: f32) {
    for p in presses {
        if p.from > t0 && p.from <= t1 && input.down.insert(p.key.clone()) {
            input.pressed.insert(p.key.clone());
        }
        if p.to > t0 && p.to <= t1 && input.down.remove(&p.key) {
            input.released.insert(p.key.clone());
        }
    }
}

/// The `DROID_KEYS` script for the step from `t0` to `t1`.
pub fn step(w: &mut World, t0: f32, t1: f32) {
    let s = script();
    if !s.is_empty() {
        apply(s, &mut w.input, t0, t1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taps_and_holds() {
        let p = parse("up@2, W@2.6 ,space@8-9.5").unwrap();
        assert_eq!(p[0], Press { key: "up".into(), from: 2.0, to: 2.0 + TAP_SECS });
        assert_eq!(p[1].key, "w");
        assert_eq!((p[2].from, p[2].to), (8.0, 9.5));
        assert!(parse("up").is_err() && parse("up@x").is_err() && parse("up@3-2").is_err());
        assert!(parse("").unwrap().is_empty());
    }

    #[test]
    fn a_tap_is_pressed_once_held_briefly_and_released() {
        let p = parse("up@1").unwrap();
        let mut w = Input::default();
        let dt = 1.0 / 60.0;
        let (mut pressed, mut down, mut released) = (0, 0, 0);
        let mut t = 0.0f32;
        for _ in 0..120 {
            let t1 = t + dt;
            apply(&p, &mut w, t, t1);
            pressed += w.pressed.contains("up") as i32;
            down += w.down.contains("up") as i32;
            released += w.released.contains("up") as i32;
            w.end_frame();
            t = t1;
        }
        assert_eq!((pressed, released), (1, 1));
        assert!((5..=7).contains(&down), "held about {TAP_SECS} s: {down} frames");
    }
}

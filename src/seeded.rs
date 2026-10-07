//! Game code's random numbers ([`crate::api::Ctx::random`]): a small PRNG (SplitMix64) shared
//! by the process. `DROID_SEED=<n>` starts it from `n`, so a headless run (`DROID_SIM`) with the
//! same seed, keys and voice plays the same game every time; without it the seed comes from the
//! system.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

static STATE: AtomicU64 = AtomicU64::new(0);
static START: OnceLock<()> = OnceLock::new();

/// The seed from `DROID_SEED`, if it is set to a number.
pub fn env_seed() -> Option<u64> {
    std::env::var("DROID_SEED").ok().and_then(|s| s.trim().parse::<u64>().ok())
}

fn start() {
    START.get_or_init(|| {
        let seed = env_seed().unwrap_or_else(rand::random::<u64>);
        STATE.store(seed, Ordering::Relaxed);
    });
}

/// SplitMix64's step: the next state and its output.
pub fn mix(state: u64) -> (u64, u64) {
    let s = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = s;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (s, z ^ (z >> 31))
}

/// The next number in 0..1.
pub fn next_f32() -> f32 {
    start();
    let mut out = 0;
    let _ = STATE.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |s| {
        let (next, z) = mix(s);
        out = z;
        Some(next)
    });
    // the top 24 bits: every f32 in 0..1 at that spacing, never 1.0
    (out >> 40) as f32 / (1u64 << 24) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_seed_gives_the_same_numbers() {
        let run = |seed: u64| {
            let mut s = seed;
            (0..5)
                .map(|_| {
                    let (n, z) = mix(s);
                    s = n;
                    z
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(run(7), run(7));
        assert_ne!(run(7), run(8));
    }

    #[test]
    fn numbers_are_in_0_to_1() {
        for _ in 0..1000 {
            let x = next_f32();
            assert!((0.0..1.0).contains(&x), "{x}");
        }
    }
}

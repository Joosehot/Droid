//! A tiny deterministic PRNG (SplitMix64) and a stable string hash (FNV-1a), so the same
//! sentence and the same choices always build the same level on every machine.

pub fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    pub fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Uniform in [lo, hi]; `lo` when the range is empty.
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        if hi <= lo {
            lo
        } else {
            lo + self.unit() * (hi - lo)
        }
    }

    /// Uniform integer in [lo, hi].
    pub fn int(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        lo + (self.next_u64() % (hi - lo + 1) as u64) as i32
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.unit() < p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_numbers() {
        let a: Vec<u64> = (0..5).scan(Rng::new(7), |r, _| Some(r.next_u64())).collect();
        let b: Vec<u64> = (0..5).scan(Rng::new(7), |r, _| Some(r.next_u64())).collect();
        assert_eq!(a, b);
        assert_ne!(fnv("a"), fnv("b"));
        let mut r = Rng::new(1);
        for _ in 0..100 {
            let x = r.range(2.0, 3.0);
            assert!((2.0..=3.0).contains(&x));
            let i = r.int(-2, 2);
            assert!((-2..=2).contains(&i));
        }
    }
}

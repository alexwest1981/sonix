//! En liten deterministisk slumptalare (xorshift64\*).
//!
//! Egen i stället för ett nytt beroende, och framför allt för att allt som
//! använder slump ska gå att testa: samma frö ger samma resultat, varje gång.
//! Används av humaniseringen (Fas 6.4) och av dithern (Fas 6.5).

#[derive(Clone, Copy, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        // 0 är en fast punkt i xorshift och skulle ge samma tal för alltid.
        Rng(if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed })
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Jämnt fördelat i [0, 1).
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u32 << 24) as f32
    }

    /// Jämnt fördelat i [-1, 1).
    pub fn next_sym(&mut self) -> f32 {
        self.next_f32() * 2.0 - 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_seed_gives_the_same_sequence() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_seeds_give_different_sequences() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        assert_ne!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn zero_seed_does_not_get_stuck() {
        // Utan skyddet skulle xorshift stå still på 0 för alltid.
        let mut r = Rng::new(0);
        let first = r.next_u64();
        assert_ne!(first, 0);
        assert_ne!(r.next_u64(), first);
    }

    #[test]
    fn floats_stay_inside_their_range() {
        let mut r = Rng::new(7);
        let mut sum = 0.0f32;
        let n = 10_000;
        for _ in 0..n {
            let x = r.next_f32();
            assert!((0.0..1.0).contains(&x));
            let s = r.next_sym();
            assert!((-1.0..1.0).contains(&s));
            sum += x;
        }
        // Medelvärdet ska ligga nära 0.5 — inte exakt, men inte heller snett.
        let mean = sum / n as f32;
        assert!((mean - 0.5).abs() < 0.02, "medelvärde {mean}");
    }
}

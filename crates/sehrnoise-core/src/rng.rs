//! Noise sources.
//!
//! A noise source produces one bipolar sample at a time. The v1 implementation is
//! uniform white noise; a new spectrum (brownian, blue, texture, ...) is a new file
//! implementing [`NoiseSource`] plus one enum variant — nothing else changes.

/// A source of bipolar noise samples in `[-1.0, 1.0)`.
pub trait NoiseSource {
    /// Produce the next sample.
    fn next(&mut self) -> f32;

    /// Restart the sequence from a given seed.
    fn reset(&mut self, seed: u64);
}

/// The xoshiro256++ generator.
///
/// Chosen because it is fast, has a 2^256 period, and passes the usual statistical
/// test batteries. It is seeded through SplitMix64 so that neighbouring seeds do
/// not produce correlated streams.
#[derive(Clone, Debug)]
pub struct Xoshiro256pp {
    state: [u64; 4],
}

impl Xoshiro256pp {
    /// Create a generator from a single 64-bit seed.
    pub fn seed_from_u64(seed: u64) -> Self {
        let mut sm = seed;
        let mut state = [0u64; 4];
        for slot in state.iter_mut() {
            sm = sm.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = sm;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            *slot = z ^ (z >> 31);
        }
        Self { state }
    }

    /// Produce the next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        let result = self.state[0]
            .wrapping_add(self.state[3])
            .rotate_left(23)
            .wrapping_add(self.state[0]);
        let t = self.state[1] << 17;

        self.state[2] ^= self.state[0];
        self.state[3] ^= self.state[1];
        self.state[1] ^= self.state[2];
        self.state[0] ^= self.state[3];
        self.state[2] ^= t;
        self.state[3] = self.state[3].rotate_left(45);

        result
    }
}

/// Uniform white noise.
///
/// Samples are drawn uniformly from `[-1, 1)` using 24 bits of the generator's
/// output. The tempting alternative — summing two uniforms to get a triangular
/// distribution — is deliberately avoided: its spectrum is not flat, and that
/// spectral tilt would corrupt every measurement in the test suite.
#[derive(Clone, Debug)]
pub struct WhiteSource {
    rng: Xoshiro256pp,
    seed: u64,
}

impl WhiteSource {
    /// Create a white source from a seed.
    pub fn new(seed: u64) -> Self {
        Self {
            rng: Xoshiro256pp::seed_from_u64(seed),
            seed,
        }
    }
}

impl NoiseSource for WhiteSource {
    fn next(&mut self) -> f32 {
        // 24 bits of entropy scaled to [-1, 1).
        (self.rng.next_u64() >> 40) as f32 * (1.0 / 8_388_608.0) - 1.0
    }

    fn reset(&mut self, seed: u64) {
        self.seed = seed;
        self.rng = Xoshiro256pp::seed_from_u64(seed);
    }
}

impl Default for WhiteSource {
    fn default() -> Self {
        Self::new(0)
    }
}

/// Derive an independent seed from a master seed and a stream index.
///
/// The two stereo channels must not share a stream, otherwise the side signal
/// would be identically zero and the width control would do nothing.
pub fn derive_seed(master: u64, stream: u64) -> u64 {
    let mut z = master
        .wrapping_add(0x9E37_79B9_7F4A_7C15)
        .wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 30)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 27;
    z = z.wrapping_add(stream.wrapping_mul(0xD6E8_FEB8_6659_FD93));
    z = (z ^ (z >> 31)).wrapping_mul(0xA24B_AED4_963E_E407);
    z ^ (z >> 28)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_range_is_respected() {
        let mut src = WhiteSource::new(42);
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        for _ in 0..100_000 {
            let x = src.next();
            assert!((-1.0..1.0).contains(&x), "sample out of range: {x}");
            min = min.min(x);
            max = max.max(x);
        }
        // The full 24-bit range should be covered.
        assert!(min < -0.99, "min {min} never approached -1");
        assert!(max > 0.99, "max {max} never approached 1");
    }

    #[test]
    fn distribution_is_flat() {
        // A uniform generator must not show the triangular artefact of the
        // "sum of two uniforms" trick: that would show up as a spectral tilt.
        let mut src = WhiteSource::new(7);
        let n = 200_000;
        let mut hist = [0u32; 10];
        for _ in 0..n {
            let x = src.next();
            let bin = (((x + 1.0) * 0.5 * 10.0) as usize).min(9);
            hist[bin] += 1;
        }
        let expected = n as f32 / 10.0;
        for (i, count) in hist.iter().enumerate() {
            let dev = (*count as f32 - expected).abs() / expected;
            assert!(dev < 0.03, "bin {i} deviates by {dev:.3}: {hist:?}");
        }
    }

    #[test]
    fn seeds_are_independent() {
        let mut a = WhiteSource::new(1);
        let mut b = WhiteSource::new(2);
        assert_ne!(a.next(), b.next());
        assert_ne!(derive_seed(1, 0), derive_seed(1, 1));
        assert_ne!(derive_seed(1, 0), derive_seed(2, 0));
    }

    #[test]
    fn reset_reproduces_the_stream() {
        let mut a = WhiteSource::new(99);
        let first: Vec<f32> = (0..64).map(|_| a.next()).collect();
        a.reset(99);
        let second: Vec<f32> = (0..64).map(|_| a.next()).collect();
        assert_eq!(first, second);
    }
}

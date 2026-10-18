//! Dense stat accumulators with the JS semantics of `addLoose` / `addInto` (src/calc/stats.js).
//!
//! A JS stat object is a plain object whose keys appear when first written. The float operations on one key happen in
//! the order the JS code performs them; the order of *different* keys never matters, so a dense array indexed by
//! [`Stat`] plus a presence mask reproduces every number bit for bit.

use dps_core::stat::Stat;

/// A JS stat object built with `addLoose`: every [`Stat`] key can be present; `mult*` keys multiply (start at 1), the
/// rest add `value * scale` (start at 0).
#[derive(Clone, Copy)]
pub struct Loose {
    pub v: [f64; Stat::COUNT],
    /// Bit `i` is set when the key `Stat::ALL[i]` exists.
    pub mask: u32,
}

impl Default for Loose {
    fn default() -> Self {
        Loose::new()
    }
}

impl std::fmt::Debug for Loose {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map().entries(self.iter().map(|(k, v)| (k.js_name(), v))).finish()
    }
}

/// Mask of the bucket keys (everything `addInto` adds).
pub const BUCKET_MASK: u32 = (1u32 << Stat::BUCKET_COUNT) - 1;

/// Mask of the `mult*` keys.
pub const MULT_MASK: u32 = (1 << Stat::MultStrength as u32)
    | (1 << Stat::MultCritDamage as u32)
    | (1 << Stat::MultCritChance as u32)
    | (1 << Stat::MultIntelligence as u32);

impl Loose {
    #[inline]
    pub const fn new() -> Loose {
        Loose { v: [0.0; Stat::COUNT], mask: 0 }
    }

    /// `{ ...stats }` of an ordered stat slice (JS spread of a config object).
    pub fn from_slice(stats: &[(Stat, f64)]) -> Loose {
        let mut l = Loose::new();
        for &(k, v) in stats {
            l.set(k, v);
        }
        l
    }

    #[inline]
    pub fn has(&self, s: Stat) -> bool {
        self.mask & (1 << s.index()) != 0
    }

    /// JS `obj[stat]`.
    #[inline]
    pub fn get(&self, s: Stat) -> Option<f64> {
        if self.has(s) {
            Some(self.v[s.index()])
        } else {
            None
        }
    }

    /// JS `obj[stat] ?? 0`.
    #[inline]
    pub fn get0(&self, s: Stat) -> f64 {
        if self.has(s) {
            self.v[s.index()]
        } else {
            0.0
        }
    }

    /// JS `obj[stat] = value`.
    #[inline]
    pub fn set(&mut self, s: Stat, value: f64) {
        self.v[s.index()] = value;
        self.mask |= 1 << s.index();
    }

    /// `addLoose(self, { [s]: value })` with scale 1 (or any scale).
    #[inline]
    pub fn add_one(&mut self, s: Stat, value: f64, scale: f64) {
        let i = s.index();
        if s.is_mult() {
            if self.mask & (1 << i) != 0 {
                self.v[i] *= value;
            } else {
                self.v[i] = 1.0 * value;
                self.mask |= 1 << i;
            }
        } else if self.mask & (1 << i) != 0 {
            self.v[i] += value * scale;
        } else {
            self.v[i] = 0.0 + value * scale;
            self.mask |= 1 << i;
        }
    }

    /// JS `addLoose(self, stats, scale)` for a config stat object.
    #[inline]
    pub fn add_loose(&mut self, stats: &[(Stat, f64)], scale: f64) {
        for &(k, v) in stats {
            self.add_one(k, v, scale);
        }
    }

    /// JS `addLoose(self, other, scale)` for another accumulator.
    #[inline]
    pub fn add_loose_from(&mut self, other: &Loose, scale: f64) {
        let mut m = other.mask;
        while m != 0 {
            let i = m.trailing_zeros() as usize;
            m &= m - 1;
            self.add_one(Stat::ALL[i], other.v[i], scale);
        }
    }

    /// Keys present, ascending [`Stat`] order (not JS key order; only used where key order is irrelevant).
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (Stat, f64)> + '_ {
        let mut m = self.mask;
        std::iter::from_fn(move || {
            if m == 0 {
                return None;
            }
            let i = m.trailing_zeros() as usize;
            m &= m - 1;
            Some((Stat::ALL[i], self.v[i]))
        })
    }

    /// True when any value is truthy (JS `Object.values(x).some(Boolean)`).
    pub fn any_truthy(&self) -> bool {
        self.iter().any(|(_, v)| v != 0.0 && !v.is_nan())
    }
}

/// JS `emptyBucket()`: one slot per bucket key.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bucket(pub [f64; Stat::BUCKET_COUNT]);

impl Default for Bucket {
    fn default() -> Self {
        Bucket::new()
    }
}

impl Bucket {
    #[inline]
    pub const fn new() -> Bucket {
        Bucket(Stat::BUCKET_INIT)
    }

    #[inline]
    pub fn get(&self, s: Stat) -> f64 {
        self.0[s.index()]
    }

    /// JS `addInto(bucket, stats, scale)` for a config stat slice.
    #[inline]
    pub fn add(&mut self, stats: &[(Stat, f64)], scale: f64) {
        for &(k, v) in stats {
            self.add_one(k, v, scale);
        }
    }

    /// `addInto(bucket, { [k]: v }, scale)`.
    #[inline]
    pub fn add_one(&mut self, k: Stat, v: f64, scale: f64) {
        if !k.is_bucket() {
            return;
        }
        if k.is_mult() {
            self.0[k.index()] *= v;
        } else {
            self.0[k.index()] += v * scale;
        }
    }

    /// JS `addInto(bucket, loose, scale)`.
    #[inline]
    pub fn add_loose(&mut self, l: &Loose, scale: f64) {
        let mut m = l.mask & BUCKET_MASK & !MULT_MASK;
        while m != 0 {
            let i = m.trailing_zeros() as usize;
            m &= m - 1;
            self.0[i] += l.v[i] * scale;
        }
        let mut m = l.mask & MULT_MASK;
        while m != 0 {
            let i = m.trailing_zeros() as usize;
            m &= m - 1;
            self.0[i] *= l.v[i];
        }
    }
}

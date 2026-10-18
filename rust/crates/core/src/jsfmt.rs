//! JavaScript number formatting, bit-exact with Node 22 (checked against `testdata/jsfmt.json`).
//!
//! - `to_fixed(x, d)`: `Number.prototype.toFixed` (rounds the exact binary value; ties go up, e.g. `0.125` -> `0.13`,
//!   `1.005` -> `1.00` because 1.005 is really 1.00499...).
//! - `to_locale_string(x)`: `x.toLocaleString('en-US')` with default options (grouping commas, at most 3 fraction
//!   digits, half-expand rounding of the *shortest* decimal form, so `1.0005` -> `1.001`; `-0` -> `-0`).
//! - `js_string(x)`: `String(x)` / template literals (shortest round-trip, `1e+21`, `1e-7`, `-0` -> `0`).
//! - `math_round(x)`: `Math.round` (ties toward +infinity, `-0.4` -> `-0`).
//! - `format_coins(n)`: `formatCoins` from src/commands/dps.js.
//!
//! The `write_*` variants append to a `String` (no allocation beyond its growth).

use std::fmt::Write as _;

/// A small stack buffer for `core::fmt` output.
struct StackBuf {
    b: [u8; 48],
    n: usize,
}

impl std::fmt::Write for StackBuf {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        let bytes = s.as_bytes();
        let end = self.n + bytes.len();
        if end > self.b.len() {
            return Err(std::fmt::Error);
        }
        self.b[self.n..end].copy_from_slice(bytes);
        self.n = end;
        Ok(())
    }
}

/// Shortest round-trip decimal digits of a finite `x > 0`: `x = 0.d1d2...dk * 10^n` (no leading/trailing zeros).
struct Digits {
    d: [u8; 24],
    k: usize,
    n: i32,
}

fn shortest(x: f64) -> Digits {
    debug_assert!(x.is_finite() && x > 0.0);
    let mut buf = StackBuf { b: [0; 48], n: 0 };
    // `{:e}` prints the shortest round-trip digits: "1.2345e3", "5e-324".
    write!(buf, "{:e}", x).expect("float fits the buffer");
    let s = &buf.b[..buf.n];
    let mut out = Digits { d: [0; 24], k: 0, n: 0 };
    let mut i = 0;
    while i < s.len() && s[i] != b'e' {
        if s[i] != b'.' {
            out.d[out.k] = s[i];
            out.k += 1;
        }
        i += 1;
    }
    let mut exp: i32 = 0;
    let mut neg = false;
    i += 1;
    if i < s.len() && s[i] == b'-' {
        neg = true;
        i += 1;
    }
    while i < s.len() {
        exp = exp * 10 + (s[i] - b'0') as i32;
        i += 1;
    }
    out.n = if neg { -exp } else { exp } + 1;
    // Rust never prints trailing zeros in the shortest mantissa, but be safe.
    while out.k > 1 && out.d[out.k - 1] == b'0' {
        out.k -= 1;
    }
    even_tie(x, &mut out);
    out
}

/// When `x` lies exactly halfway between two shortest candidates, JS (and ICU) pick the even one; Rust's shortest
/// formatter can pick the odd one. Exact integer test: with `x = o * 2^(v+e)` (`o` odd) and `t = k - n`,
/// `2x * 10^t` equals the odd integer `W = 2D -+ 1` iff `v+e+1+t == 0` and `o * 5^t == W` (or `o == W * 5^-t`).
fn even_tie(x: f64, out: &mut Digits) {
    if out.k > 17 {
        return;
    }
    let mut d: u64 = 0;
    for &c in &out.d[..out.k] {
        d = d * 10 + (c - b'0') as u64;
    }
    if d.is_multiple_of(2) {
        return;
    }
    let t = out.k as i32 - out.n;
    let (m, e) = decompose(x);
    let v = m.trailing_zeros() as i32;
    let o = (m >> v) as u128;
    if v + e + 1 + t != 0 {
        return;
    }
    let pow5 = |p: i32| -> Option<u128> { if p > 50 { None } else { Some(5u128.pow(p as u32)) } };
    let is_half = |w: u64| -> bool {
        let w = w as u128;
        if t >= 0 {
            pow5(t).and_then(|p| o.checked_mul(p)) == Some(w)
        } else {
            pow5(-t).and_then(|p| w.checked_mul(p)) == Some(o)
        }
    };
    let alt = if is_half(2 * d - 1) {
        d - 1
    } else if is_half(2 * d + 1) {
        d + 1
    } else {
        return;
    };
    // The even candidate must round-trip to x too.
    let mut buf = StackBuf { b: [0; 48], n: 0 };
    if write!(buf, "{}e{}", alt, out.n - out.k as i32).is_err() {
        return;
    }
    let parsed: f64 = match std::str::from_utf8(&buf.b[..buf.n]).ok().and_then(|s| s.parse().ok()) {
        Some(p) => p,
        None => return,
    };
    if parsed != x {
        return;
    }
    let mut digits = [0u8; 24];
    let mut len = 0;
    let mut a = alt;
    let mut tmp = [0u8; 24];
    while a > 0 {
        tmp[len] = b'0' + (a % 10) as u8;
        a /= 10;
        len += 1;
    }
    for i in 0..len {
        digits[i] = tmp[len - 1 - i];
    }
    // Same length as D (an odd D never turns into a power of ten here), then drop trailing zeros.
    out.n += len as i32 - out.k as i32;
    out.d = digits;
    out.k = len;
    while out.k > 1 && out.d[out.k - 1] == b'0' {
        out.k -= 1;
    }
}

fn push_zeros(out: &mut String, n: usize) {
    for _ in 0..n {
        out.push('0');
    }
}

fn push_ascii(out: &mut String, bytes: &[u8]) {
    // Digits only, always ASCII.
    out.push_str(std::str::from_utf8(bytes).expect("ascii digits"));
}

/// `String(x)`: JS Number::toString (radix 10).
pub fn write_js_string(out: &mut String, x: f64) {
    if x.is_nan() {
        out.push_str("NaN");
        return;
    }
    if x == 0.0 {
        out.push('0');
        return;
    }
    if x.is_infinite() {
        out.push_str(if x > 0.0 { "Infinity" } else { "-Infinity" });
        return;
    }
    // Fast path: integers below 2^53 print as plain integers.
    if x.fract() == 0.0 && x.abs() < 9007199254740992.0 {
        let _ = write!(out, "{}", x as i64);
        return;
    }
    if x < 0.0 {
        out.push('-');
    }
    let s = shortest(x.abs());
    let k = s.k as i32;
    let n = s.n;
    let d = &s.d[..s.k];
    if k <= n && n <= 21 {
        push_ascii(out, d);
        push_zeros(out, (n - k) as usize);
    } else if 0 < n && n <= 21 {
        push_ascii(out, &d[..n as usize]);
        out.push('.');
        push_ascii(out, &d[n as usize..]);
    } else if -6 < n && n <= 0 {
        out.push_str("0.");
        push_zeros(out, (-n) as usize);
        push_ascii(out, d);
    } else {
        let e = n - 1;
        push_ascii(out, &d[..1]);
        if k > 1 {
            out.push('.');
            push_ascii(out, &d[1..]);
        }
        out.push('e');
        out.push(if e < 0 { '-' } else { '+' });
        let _ = write!(out, "{}", e.abs());
    }
}

/// `String(x)`.
pub fn js_string(x: f64) -> String {
    let mut s = String::new();
    write_js_string(&mut s, x);
    s
}

/// Number of characters of `String(Math.floor(x))` for `x > 0` (JS `String(Math.floor(n)).length`).
pub fn floor_digits(x: f64) -> usize {
    let f = x.floor();
    if f.is_finite() && f < 1e21 {
        if f == 0.0 {
            return 1;
        }
        // Integers below 1e21 print in full: count decimal digits exactly (powers of 10 are exact up to 1e22).
        let mut n = 1;
        let mut p = 10.0;
        while p <= f {
            n += 1;
            p *= 10.0;
        }
        return n;
    }
    js_string(f).len()
}

// ---------------------------------------------------------------------------------------------------------------
// toFixed
// ---------------------------------------------------------------------------------------------------------------

/// Decompose a finite `x >= 0` into `m * 2^e` with integer `m < 2^53`.
fn decompose(x: f64) -> (u64, i32) {
    let bits = x.to_bits();
    let exp = ((bits >> 52) & 0x7ff) as i32;
    let frac = bits & ((1u64 << 52) - 1);
    if exp == 0 {
        (frac, -1074)
    } else {
        (frac | (1u64 << 52), exp - 1075)
    }
}

/// Render the integer `digits` (decimal ASCII) as a fixed-point number with `f` fraction digits.
fn push_fixed_digits(out: &mut String, digits: &[u8], f: usize) {
    if f == 0 {
        push_ascii(out, digits);
        return;
    }
    let k = digits.len();
    if k <= f {
        out.push_str("0.");
        push_zeros(out, f - k);
        push_ascii(out, digits);
    } else {
        push_ascii(out, &digits[..k - f]);
        out.push('.');
        push_ascii(out, &digits[k - f..]);
    }
}

fn u128_digits(mut v: u128, buf: &mut [u8; 40]) -> &[u8] {
    let mut i = buf.len();
    if v == 0 {
        i -= 1;
        buf[i] = b'0';
    }
    while v > 0 {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
    }
    &buf[i..]
}

/// Minimal fixed-width unsigned big integer for `toFixed` with more than 19 fraction digits.
#[derive(Clone, Copy)]
struct Big {
    limbs: [u64; 12],
}

impl Big {
    fn from_u64(v: u64) -> Big {
        let mut limbs = [0u64; 12];
        limbs[0] = v;
        Big { limbs }
    }
    fn mul_small(&mut self, m: u64) {
        let mut carry: u128 = 0;
        for l in &mut self.limbs {
            let p = (*l as u128) * (m as u128) + carry;
            *l = p as u64;
            carry = p >> 64;
        }
        debug_assert!(carry == 0);
    }
    fn bit(&self, i: u32) -> bool {
        let w = (i / 64) as usize;
        w < self.limbs.len() && (self.limbs[w] >> (i % 64)) & 1 == 1
    }
    fn shr(&self, s: u32) -> Big {
        let mut out = [0u64; 12];
        let w = (s / 64) as usize;
        let b = s % 64;
        for (i, o) in out.iter_mut().enumerate() {
            let src = i + w;
            if src >= self.limbs.len() {
                break;
            }
            let lo = self.limbs[src] >> b;
            let hi = if b > 0 && src + 1 < self.limbs.len() { self.limbs[src + 1] << (64 - b) } else { 0 };
            *o = lo | hi;
        }
        Big { limbs: out }
    }
    fn add_small(&mut self, v: u64) {
        let mut carry = v;
        for l in &mut self.limbs {
            let (s, c) = l.overflowing_add(carry);
            *l = s;
            if !c {
                return;
            }
            carry = 1;
        }
    }
    fn is_zero(&self) -> bool {
        self.limbs.iter().all(|&l| l == 0)
    }
    /// Divide in place by `d`, returning the remainder.
    fn div_small(&mut self, d: u64) -> u64 {
        let mut rem: u128 = 0;
        for l in self.limbs.iter_mut().rev() {
            let cur = (rem << 64) | (*l as u128);
            *l = (cur / d as u128) as u64;
            rem = cur % d as u128;
        }
        rem as u64
    }
    fn to_digits(mut self) -> Vec<u8> {
        if self.is_zero() {
            return vec![b'0'];
        }
        let mut chunks = Vec::new();
        while !self.is_zero() {
            chunks.push(self.div_small(10_000_000_000_000_000_000));
        }
        let mut out = Vec::new();
        for (i, c) in chunks.iter().rev().enumerate() {
            let s = if i == 0 { format!("{c}") } else { format!("{c:019}") };
            out.extend_from_slice(s.as_bytes());
        }
        out
    }
}

/// `x.toFixed(f)` appended to `out`. `f` must be at most 100 (JS throws a RangeError above).
pub fn write_to_fixed(out: &mut String, x: f64, f: u32) {
    assert!(f <= 100, "toFixed digits out of range");
    if x.is_nan() {
        out.push_str("NaN");
        return;
    }
    let mut x = x;
    if x < 0.0 {
        out.push('-');
        x = -x;
    }
    if x >= 1e21 {
        write_js_string(out, x);
        return;
    }
    let f = f as usize;
    let (m, e) = decompose(x);
    let mut buf = [0u8; 40];
    if m == 0 {
        push_fixed_digits(out, b"0", f);
        return;
    }
    if e >= 0 {
        // An integer below 1e21 (< 2^70): exact, the fraction is all zeros.
        let int = (m as u128) << e;
        push_ascii(out, u128_digits(int, &mut buf));
        if f > 0 {
            out.push('.');
            push_zeros(out, f);
        }
        return;
    }
    // x * 10^f = m * 10^f / 2^s, rounded half up: add bit s-1 (r >= 2^(s-1) exactly when that bit is set).
    let s = (-e) as u32;
    if f <= 19 {
        let p = (m as u128) * 10u128.pow(f as u32); // < 2^117
        let q = if s >= 128 { 0 } else { (p >> s) + ((p >> (s - 1)) & 1) };
        push_fixed_digits(out, u128_digits(q, &mut buf), f);
        return;
    }
    let mut p = Big::from_u64(m);
    let mut left = f;
    while left > 0 {
        let step = left.min(19);
        p.mul_small(10u64.pow(step as u32));
        left -= step;
    }
    let mut q = p.shr(s);
    if p.bit(s - 1) {
        q.add_small(1);
    }
    push_fixed_digits(out, &q.to_digits(), f);
}

/// `x.toFixed(f)`.
pub fn to_fixed(x: f64, f: u32) -> String {
    let mut s = String::new();
    write_to_fixed(&mut s, x, f);
    s
}

/// JS `+x.toFixed(f)` (the number the fixed-point string parses back to).
pub fn fixed_number(x: f64, f: u32) -> f64 {
    let s = to_fixed(x, f);
    s.parse::<f64>().unwrap_or(f64::NAN)
}

// ---------------------------------------------------------------------------------------------------------------
// toLocaleString('en-US')
// ---------------------------------------------------------------------------------------------------------------

fn push_grouped(out: &mut String, int_digits: &[u8]) {
    let k = int_digits.len();
    for (i, &c) in int_digits.iter().enumerate() {
        if i > 0 && (k - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c as char);
    }
}

/// `x.toLocaleString('en-US')` appended to `out` (default options: max 3 fraction digits, half-expand, grouping).
pub fn write_to_locale_string(out: &mut String, x: f64) {
    if x.is_nan() {
        out.push_str("NaN");
        return;
    }
    if x.is_sign_negative() {
        out.push('-');
    }
    if x.is_infinite() {
        out.push('∞');
        return;
    }
    let a = x.abs();
    if a == 0.0 {
        out.push('0');
        return;
    }
    // Fast path: integers below 2^53.
    if a.fract() == 0.0 && a < 9007199254740992.0 {
        let mut buf = [0u8; 40];
        push_grouped(out, u128_digits(a as u128, &mut buf));
        return;
    }
    let s = shortest(a);
    let mut d = s.d;
    let mut k = s.k;
    let mut n = s.n;
    // Keep n + 3 digits (3 fraction digits), rounding half away from zero on the shortest decimal form.
    let keep = n + 3;
    if (keep as i64) < k as i64 {
        if keep < 0 {
            out.push('0');
            return;
        }
        let keep = keep as usize;
        let round_up = d[keep] >= b'5';
        k = keep;
        if round_up {
            let mut i = k;
            loop {
                if i == 0 {
                    // Carry out of the first digit (or nothing kept): the value becomes 10^n.
                    d[0] = b'1';
                    k = 1;
                    n += 1;
                    break;
                }
                i -= 1;
                if d[i] == b'9' {
                    d[i] = b'0';
                } else {
                    d[i] += 1;
                    break;
                }
            }
        }
        while k > 0 && d[k - 1] == b'0' {
            k -= 1;
        }
        if k == 0 {
            out.push('0');
            return;
        }
    }
    let digits = &d[..k];
    if n <= 0 {
        out.push_str("0.");
        push_zeros(out, (-n) as usize);
        push_ascii(out, digits);
    } else if (n as usize) >= k {
        let mut int = [b'0'; 400];
        int[..k].copy_from_slice(digits);
        push_grouped(out, &int[..n as usize]);
    } else {
        push_grouped(out, &digits[..n as usize]);
        out.push('.');
        push_ascii(out, &digits[n as usize..]);
    }
}

/// `x.toLocaleString('en-US')`.
pub fn to_locale_string(x: f64) -> String {
    let mut s = String::new();
    write_to_locale_string(&mut s, x);
    s
}

// ---------------------------------------------------------------------------------------------------------------
// Math.round, formatCoins
// ---------------------------------------------------------------------------------------------------------------

/// `Math.round(x)`: nearest integer, ties toward +infinity; keeps `-0` and gives `-0` for `-0.5 <= x < 0`.
#[inline]
pub fn math_round(x: f64) -> f64 {
    if !x.is_finite() || x == 0.0 {
        return x;
    }
    if (-0.5..0.0).contains(&x) {
        return -0.0;
    }
    let f = x.floor();
    // x - floor(x) is exact for every double.
    if x - f >= 0.5 {
        f + 1.0
    } else {
        f
    }
}

/// `formatCoins(n)` (src/commands/dps.js): `1.23B`, `4.50M`, `12.3K`, or the rounded number.
pub fn write_format_coins(out: &mut String, n: f64) {
    if n >= 1e9 {
        write_to_fixed(out, n / 1e9, 2);
        out.push('B');
    } else if n >= 1e6 {
        write_to_fixed(out, n / 1e6, 2);
        out.push('M');
    } else if n >= 1e3 {
        write_to_fixed(out, n / 1e3, 1);
        out.push('K');
    } else {
        write_js_string(out, math_round(n));
    }
}

/// `formatCoins(n)`.
pub fn format_coins(n: f64) -> String {
    let mut s = String::new();
    write_format_coins(&mut s, n);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spot_checks() {
        assert_eq!(to_fixed(0.125, 2), "0.13");
        assert_eq!(to_fixed(1.005, 2), "1.00");
        assert_eq!(to_fixed(2.5, 0), "3");
        assert_eq!(to_fixed(-0.0, 2), "0.00");
        assert_eq!(to_fixed(-0.0001, 3), "-0.000");
        assert_eq!(to_fixed(1e21, 2), "1e+21");
        assert_eq!(to_locale_string(1.0005), "1.001");
        assert_eq!(to_locale_string(123456.7895), "123,456.79");
        assert_eq!(to_locale_string(-0.0), "-0");
        assert_eq!(to_locale_string(1e21), "1,000,000,000,000,000,000,000");
        assert_eq!(js_string(1e21), "1e+21");
        assert_eq!(js_string(1e-7), "1e-7");
        assert_eq!(js_string(-0.0), "0");
        assert_eq!(js_string(0.1 + 0.2), "0.30000000000000004");
        assert_eq!(math_round(-2.5), -2.0);
        assert!(math_round(-0.4).is_sign_negative());
        assert_eq!(format_coins(1_234_567.0), "1.23M");
        assert_eq!(floor_digits(123.9), 3);
        assert_eq!(floor_digits(0.5), 1);
        assert_eq!(floor_digits(1e9), 10);
    }
}

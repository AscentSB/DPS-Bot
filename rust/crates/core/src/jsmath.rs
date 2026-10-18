//! `Math.log` and `**` / `Math.pow` exactly as V8 computes them: ports of V8's fdlibm-derived `log` and `pow`
//! (src/base/ieee754.cc, including its one deviation from fdlibm's e_pow.c in the final `r = ...` division). Rust's `f64::ln` / `powf` call the platform libm,
//! which differs from V8 in the last bit for a few percent of inputs; these match Node bit for bit (see
//! tests/jsfmt_vectors.rs).

// The fdlibm constants are written as in the C source.
#![allow(clippy::excessive_precision, clippy::approx_constant)]

/// JS `Math.min(a, b)`: NaN if either is NaN, and `-0 < +0`. (Rust's `f64::min` ignores NaN.)
#[inline]
pub fn min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a < b || (a == b && a.is_sign_negative()) {
        a
    } else {
        b
    }
}

/// JS `Math.max(a, b)`: NaN if either is NaN, and `+0 > -0`.
#[inline]
pub fn max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a > b || (a == b && a.is_sign_positive()) {
        a
    } else {
        b
    }
}

/// JS `Math.max(0, Math.min(1, x))` (NaN stays NaN).
#[inline]
pub fn clamp01(x: f64) -> f64 {
    max(0.0, min(1.0, x))
}

#[inline]
fn hi(x: f64) -> i32 {
    (x.to_bits() >> 32) as u32 as i32
}

#[inline]
fn lo(x: f64) -> u32 {
    x.to_bits() as u32
}

#[inline]
fn with_hi(x: f64, h: i32) -> f64 {
    f64::from_bits(((h as u32 as u64) << 32) | (x.to_bits() & 0xffff_ffff))
}

#[inline]
fn with_lo(x: f64, l: u32) -> f64 {
    f64::from_bits((x.to_bits() & 0xffff_ffff_0000_0000) | l as u64)
}

/// JS `Math.log(x)` (fdlibm e_log.c).
pub fn log(x: f64) -> f64 {
    const LN2_HI: f64 = 6.93147180369123816490e-01;
    const LN2_LO: f64 = 1.90821492927058770002e-10;
    const TWO54: f64 = 1.80143985094819840000e+16;
    const LG1: f64 = 6.666666666666735130e-01;
    const LG2: f64 = 3.999999999940941908e-01;
    const LG3: f64 = 2.857142874366239149e-01;
    const LG4: f64 = 2.222219843214978396e-01;
    const LG5: f64 = 1.818357216161805012e-01;
    const LG6: f64 = 1.531383769920937332e-01;
    const LG7: f64 = 1.479819860511658591e-01;

    let mut x = x;
    let mut hx = hi(x);
    let lx = lo(x);
    let mut k: i32 = 0;
    if hx < 0x0010_0000 {
        // x < 2**-1022
        if ((hx & 0x7fff_ffff) as u32 | lx) == 0 {
            return f64::NEG_INFINITY; // log(+-0) = -inf
        }
        if hx < 0 {
            return f64::NAN; // log(-#) = NaN
        }
        k -= 54;
        x *= TWO54; // subnormal: scale up
        hx = hi(x);
    }
    if hx >= 0x7ff0_0000 {
        return x + x;
    }
    k += (hx >> 20) - 1023;
    hx &= 0x000f_ffff;
    let i = (hx + 0x95f64) & 0x10_0000;
    x = with_hi(x, hx | (i ^ 0x3ff0_0000)); // normalize x or x/2
    k += i >> 20;
    let f = x - 1.0;
    if (0x000f_ffff & (2 + hx)) < 3 {
        // -2**-20 <= f < 2**-20
        if f == 0.0 {
            if k == 0 {
                return 0.0;
            }
            let dk = k as f64;
            return dk * LN2_HI + dk * LN2_LO;
        }
        let r = f * f * (0.5 - 0.33333333333333333 * f);
        if k == 0 {
            return f - r;
        }
        let dk = k as f64;
        return dk * LN2_HI - ((r - dk * LN2_LO) - f);
    }
    let s = f / (2.0 + f);
    let dk = k as f64;
    let z = s * s;
    let mut i = hx - 0x6147a;
    let w = z * z;
    let j = 0x6b851 - hx;
    let t1 = w * (LG2 + w * (LG4 + w * LG6));
    let t2 = z * (LG1 + w * (LG3 + w * (LG5 + w * LG7)));
    i |= j;
    let r = t2 + t1;
    if i > 0 {
        let hfsq = 0.5 * f * f;
        if k == 0 {
            f - (hfsq - s * (hfsq + r))
        } else {
            dk * LN2_HI - ((hfsq - (s * (hfsq + r) + dk * LN2_LO)) - f)
        }
    } else if k == 0 {
        f - s * (f - r)
    } else {
        dk * LN2_HI - ((s * (f - r) - dk * LN2_LO) - f)
    }
}

/// musl `scalbn`: `x * 2^n` with a single rounding.
fn scalbn(x: f64, mut n: i32) -> f64 {
    let mut y = x;
    if n > 1023 {
        y *= f64::from_bits(0x7fe0_0000_0000_0000); // 2^1023
        n -= 1023;
        if n > 1023 {
            y *= f64::from_bits(0x7fe0_0000_0000_0000);
            n -= 1023;
            if n > 1023 {
                n = 1023;
            }
        }
    } else if n < -1022 {
        // 2^-1022 * 2^53, keeping the final n < -53 to avoid double rounding in the subnormal range.
        let k = f64::from_bits(0x0010_0000_0000_0000) * f64::from_bits(0x4340_0000_0000_0000);
        y *= k;
        n += 1022 - 53;
        if n < -1022 {
            y *= k;
            n += 1022 - 53;
            if n < -1022 {
                n = -1022;
            }
        }
    }
    y * f64::from_bits(((0x3ff + n) as u64) << 52)
}

/// JS `x ** y` / `Math.pow(x, y)` (fdlibm e_pow.c, plus the ECMAScript rule that `(+-1) ** +-Infinity` and
/// `x ** NaN` are NaN).
pub fn pow(x: f64, y: f64) -> f64 {
    if y.is_nan() || ((x == 1.0 || x == -1.0) && y.is_infinite()) {
        return f64::NAN;
    }
    ieee754_pow(x, y)
}

fn ieee754_pow(x: f64, y: f64) -> f64 {
    const BP: [f64; 2] = [1.0, 1.5];
    const DP_H: [f64; 2] = [0.0, 5.84962487220764160156e-01];
    const DP_L: [f64; 2] = [0.0, 1.35003920212974897128e-08];
    const TWO53: f64 = 9007199254740992.0;
    const HUGE: f64 = 1.0e300;
    const TINY: f64 = 1.0e-300;
    const L1: f64 = 5.99999999999994648725e-01;
    const L2: f64 = 4.28571428578550184252e-01;
    const L3: f64 = 3.33333329818377432918e-01;
    const L4: f64 = 2.72728123808534006489e-01;
    const L5: f64 = 2.30660745775561754067e-01;
    const L6: f64 = 2.06975017800338417784e-01;
    const P1: f64 = 1.66666666666666019037e-01;
    const P2: f64 = -2.77777777770155933842e-03;
    const P3: f64 = 6.61375632143793436117e-05;
    const P4: f64 = -1.65339022054652515390e-06;
    const P5: f64 = 4.13813679705723846039e-08;
    const LG2: f64 = 6.93147180559945286227e-01;
    const LG2_H: f64 = 6.93147182464599609375e-01;
    const LG2_L: f64 = -1.90465429995776804525e-09;
    const OVT: f64 = 8.0085662595372944372e-17;
    const CP: f64 = 9.61796693925975554329e-01;
    const CP_H: f64 = 9.61796700954437255859e-01;
    const CP_L: f64 = -7.02846165095275826516e-09;
    const IVLN2: f64 = 1.44269504088896338700e+00;
    const IVLN2_H: f64 = 1.44269502162933349609e+00;
    const IVLN2_L: f64 = 1.92596299112661746887e-08;

    let hx = hi(x);
    let lx = lo(x);
    let hy = hi(y);
    let ly = lo(y);
    let mut ix = hx & 0x7fff_ffff;
    let iy = hy & 0x7fff_ffff;

    // y == 0: x**0 = 1
    if (iy as u32 | ly) == 0 {
        return 1.0;
    }
    // NaN
    if ix > 0x7ff0_0000 || (ix == 0x7ff0_0000 && lx != 0) || iy > 0x7ff0_0000 || (iy == 0x7ff0_0000 && ly != 0) {
        return x + y;
    }

    // yisint = 0: not an integer, 1: odd integer, 2: even integer (only computed for x < 0).
    let mut yisint = 0;
    if hx < 0 {
        if iy >= 0x4340_0000 {
            yisint = 2;
        } else if iy >= 0x3ff0_0000 {
            let k = (iy >> 20) - 0x3ff;
            if k > 20 {
                let j = ly >> (52 - k);
                if (j << (52 - k)) == ly {
                    yisint = 2 - (j & 1) as i32;
                }
            } else if ly == 0 {
                let j = iy >> (20 - k);
                if (j << (20 - k)) == iy {
                    yisint = 2 - (j & 1);
                }
            }
        }
    }

    // special values of y
    if ly == 0 {
        if iy == 0x7ff0_0000 {
            // y is +-inf
            if ((ix - 0x3ff0_0000) as u32 | lx) == 0 {
                return f64::NAN; // inf**+-1 is NaN (fdlibm: y - y)
            } else if ix >= 0x3ff0_0000 {
                return if hy >= 0 { y } else { 0.0 };
            } else {
                return if hy < 0 { -y } else { 0.0 };
            }
        }
        if iy == 0x3ff0_0000 {
            // y is +-1
            return if hy < 0 { 1.0 / x } else { x };
        }
        if hy == 0x4000_0000 {
            return x * x; // y is 2
        }
        if hy == 0x3fe0_0000 && hx >= 0 {
            return x.sqrt(); // y is 0.5, x >= +0
        }
    }

    let mut ax = x.abs();
    // special values of x
    if lx == 0 && (ix == 0x7ff0_0000 || ix == 0 || ix == 0x3ff0_0000) {
        let mut z = ax; // x is +-0, +-inf, +-1
        if hy < 0 {
            z = 1.0 / z;
        }
        if hx < 0 {
            if ((ix - 0x3ff0_0000) | yisint) == 0 {
                z = f64::NAN; // (-1)**non-int is NaN
            } else if yisint == 1 {
                z = -z;
            }
        }
        return z;
    }

    // fdlibm `n = (hx>>31)+1`: 0 for negative x, 1 otherwise.
    let n: i32 = if hx < 0 { 0 } else { 1 };

    // (x<0)**(non-int) is NaN
    if (n | yisint) == 0 {
        return f64::NAN;
    }

    let mut s = 1.0; // sign of the result
    if (n | (yisint - 1)) == 0 {
        s = -1.0; // (-ve)**(odd int)
    }

    let t1: f64;
    let t2: f64;
    if iy > 0x41e0_0000 {
        // |y| > 2**31
        if iy > 0x43f0_0000 {
            // |y| > 2**64, must over/underflow
            if ix <= 0x3fef_ffff {
                return if hy < 0 { HUGE * HUGE } else { TINY * TINY };
            }
            if ix >= 0x3ff0_0000 {
                return if hy > 0 { HUGE * HUGE } else { TINY * TINY };
            }
        }
        // over/underflow if x is not close to one
        if ix < 0x3fef_ffff {
            return if hy < 0 { s * HUGE * HUGE } else { s * TINY * TINY };
        }
        if ix > 0x3ff0_0000 {
            return if hy > 0 { s * HUGE * HUGE } else { s * TINY * TINY };
        }
        // |1-x| is tiny <= 2**-20: log(x) by x-x^2/2+x^3/3-x^4/4
        let t = ax - 1.0;
        let w = (t * t) * (0.5 - t * (0.3333333333333333333333 - t * 0.25));
        let u = IVLN2_H * t;
        let v = t * IVLN2_L - w * IVLN2;
        let tt1 = with_lo(u + v, 0);
        t1 = tt1;
        t2 = v - (tt1 - u);
    } else {
        let mut n2: i32 = 0;
        // subnormal x
        if ix < 0x0010_0000 {
            ax *= TWO53;
            n2 -= 53;
            ix = hi(ax);
        }
        n2 += (ix >> 20) - 0x3ff;
        let j = ix & 0x000f_ffff;
        // determine interval
        ix = j | 0x3ff0_0000; // normalize ix
        let k: usize;
        if j <= 0x3988e {
            k = 0; // |x| < sqrt(3/2)
        } else if j < 0xbb67a {
            k = 1; // |x| < sqrt(3)
        } else {
            k = 0;
            n2 += 1;
            ix -= 0x0010_0000;
        }
        ax = with_hi(ax, ix);

        // ss = s_h + s_l = (x-1)/(x+1) or (x-1.5)/(x+1.5)
        let u = ax - BP[k];
        let v = 1.0 / (ax + BP[k]);
        let ss = u * v;
        let s_h = with_lo(ss, 0);
        // t_h = ax + bp[k] High
        let t_h = with_hi(0.0, ((ix >> 1) | 0x2000_0000) + 0x0008_0000 + ((k as i32) << 18));
        let t_l = ax - (t_h - BP[k]);
        let s_l = v * ((u - s_h * t_h) - s_h * t_l);
        // log(ax)
        let mut s2 = ss * ss;
        let mut r = s2 * s2 * (L1 + s2 * (L2 + s2 * (L3 + s2 * (L4 + s2 * (L5 + s2 * L6)))));
        r += s_l * (s_h + ss);
        s2 = s_h * s_h;
        let t_h = with_lo(3.0 + s2 + r, 0);
        let t_l = r - ((t_h - 3.0) - s2);
        // u + v = ss*(1+...)
        let u = s_h * t_h;
        let v = s_l * t_h + t_l * ss;
        // 2/(3log2)*(ss+...)
        let p_h = with_lo(u + v, 0);
        let p_l = v - (p_h - u);
        let z_h = CP_H * p_h;
        let z_l = CP_L * p_h + p_l * CP + DP_L[k];
        // log2(ax) = (ss+..)*2/(3*log2) = n + dp_h + z_h + z_l
        let t = n2 as f64;
        let tt1 = with_lo(((z_h + z_l) + DP_H[k]) + t, 0);
        t1 = tt1;
        t2 = z_l - (((tt1 - t) - DP_H[k]) - z_h);
    }

    // split y into y1 + y2 and compute (y1+y2)*(t1+t2)
    let y1 = with_lo(y, 0);
    let p_l = (y - y1) * t1 + y * t2;
    let mut p_h = y1 * t1;
    let mut z = p_l + p_h;
    let j = hi(z);
    let i = lo(z) as i32;
    if j >= 0x4090_0000 {
        // z >= 1024
        if ((j - 0x4090_0000) | i) != 0 {
            return s * HUGE * HUGE; // overflow
        } else if p_l + OVT > z - p_h {
            return s * HUGE * HUGE;
        }
    } else if (j & 0x7fff_ffff) >= 0x4090_cc00 {
        // z <= -1075
        if ((j.wrapping_sub(0xc090_cc00_u32 as i32)) | i) != 0 {
            return s * TINY * TINY; // underflow
        } else if p_l <= z - p_h {
            return s * TINY * TINY;
        }
    }

    // 2**(p_h+p_l)
    let i = j & 0x7fff_ffff;
    let mut k = (i >> 20) - 0x3ff;
    let mut n: i32 = 0;
    if i > 0x3fe0_0000 {
        // |z| > 0.5: n = [z+0.5]
        n = j + (0x0010_0000 >> (k + 1));
        k = ((n & 0x7fff_ffff) >> 20) - 0x3ff; // new k for n
        let t = with_hi(0.0, n & !(0x000f_ffff >> k));
        n = ((n & 0x000f_ffff) | 0x0010_0000) >> (20 - k);
        if j < 0 {
            n = -n;
        }
        p_h -= t;
    }
    let t = with_lo(p_l + p_h, 0);
    let u = t * LG2_H;
    let v = (p_l - (t - p_h)) * LG2 + t * LG2_L;
    z = u + v;
    let w = v - (z - u);
    let t = z * z;
    let t1 = z - t * (P1 + t * (P2 + t * (P3 + t * (P4 + t * P5))));
    // V8's port divides by the whole difference (fdlibm has `(z*t1)/(t1-two)-(w+z*w)`); keep V8's version.
    let r = (z * t1) / ((t1 - 2.0) - (w + z * w));
    z = 1.0 - (r - z);
    let mut j = hi(z);
    j = j.wrapping_add(n << 20);
    if (j >> 20) <= 0 {
        z = scalbn(z, n); // subnormal output
    } else {
        z = with_hi(z, hi(z).wrapping_add(n << 20));
    }
    s * z
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basics() {
        assert_eq!(log(1.0), 0.0);
        assert_eq!(log(0.0), f64::NEG_INFINITY);
        assert!(log(-1.0).is_nan());
        assert_eq!(log(std::f64::consts::E), 1.0);
        assert_eq!(pow(2.0, 10.0), 1024.0);
        assert_eq!(pow(-2.0, 3.0), -8.0);
        assert!(pow(-2.0, 0.5).is_nan());
        assert!(pow(1.0, f64::INFINITY).is_nan());
        assert_eq!(pow(4.0, 0.5), 2.0);
        assert_eq!(pow(0.0, -1.0), f64::INFINITY);
        assert_eq!(pow(2.0, -1075.0), 0.0);
        assert_eq!(pow(2.0, -1074.0), 5e-324);
        assert!(min(f64::NAN, 1.0).is_nan());
        assert!(min(0.0, -0.0).is_sign_negative());
        assert!(max(-0.0, 0.0).is_sign_positive());
        assert!(clamp01(f64::NAN).is_nan());
        assert_eq!(clamp01(2.0), 1.0);
    }
}

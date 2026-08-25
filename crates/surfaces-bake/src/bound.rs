//! Exact `MAX_ERR_LSB` residual: `n/d * 2^exp` with a 256-bit numerator.
//! Do not replace with host `f64`, ULP padding, or unreduced `i128` `n/d`.
//! `NonFiniteDeviation` is for a true non-finite residual, not integer width.

#![allow(clippy::many_single_char_names)]

#[derive(Clone, Copy, Debug)]
pub(crate) struct Ratio {
    n: I256,
    d: i128,
    exp: i32,
}

#[derive(Clone, Copy, Debug)]
struct I256 {
    hi: u128,
    lo: u128,
    neg: bool,
}

#[rustfmt::skip]
mod arith {
    use super::{I256, Ratio};
    use core::cmp::Ordering;

    impl I256 {
        pub(super) const ZERO: Self = Self { hi: 0, lo: 0, neg: false };
        pub(super) fn from_i128(n: i128) -> Self {
            if n == 0 { Self::ZERO } else { Self { hi: 0, lo: n.unsigned_abs(), neg: n < 0 } }
        }
        pub(super) fn is_zero(self) -> bool { self.hi == 0 && self.lo == 0 }
        fn abs(self) -> Self { Self { hi: self.hi, lo: self.lo, neg: false } }
        pub(super) fn wrapping_neg(self) -> Self {
            if self.is_zero() { self } else { Self { hi: self.hi, lo: self.lo, neg: !self.neg } }
        }
        pub(super) fn cmp_mag(self, o: Self) -> Ordering { self.hi.cmp(&o.hi).then(self.lo.cmp(&o.lo)) }
        fn sub_mag(self, o: Self) -> Self {
            let (lo, b) = self.lo.overflowing_sub(o.lo);
            let out = Self { hi: self.hi - o.hi - u128::from(b), lo, neg: self.neg };
            if out.is_zero() { Self::ZERO } else { out }
        }
        pub(super) fn add(self, o: Self) -> Option<Self> {
            if self.is_zero() { return Some(o); }
            if o.is_zero() { return Some(self); }
            if self.neg == o.neg {
                let (lo, c) = self.lo.overflowing_add(o.lo);
                let (hi, o1) = self.hi.overflowing_add(o.hi);
                let (hi, o2) = hi.overflowing_add(u128::from(c));
                if o1 || o2 { None } else { Some(Self { hi, lo, neg: self.neg }) }
            } else {
                Some(match self.cmp_mag(o) {
                    Ordering::Equal => Self::ZERO,
                    Ordering::Greater => self.sub_mag(o),
                    Ordering::Less => o.sub_mag(self),
                })
            }
        }
        fn checked_shl(self, bits: u32) -> Option<Self> {
            if bits == 0 || self.is_zero() { return Some(self); }
            if bits >= 256 { return None; }
            if bits >= 128 {
                if self.hi != 0 { return None; }
                let s = bits - 128;
                if s > 0 && self.lo >> (128 - s) != 0 { return None; }
                return Some(Self { hi: self.lo << s, lo: 0, neg: self.neg });
            }
            if self.hi >> (128 - bits) != 0 { return None; }
            Some(Self { hi: (self.hi << bits) | (self.lo >> (128 - bits)), lo: self.lo << bits, neg: self.neg })
        }
        fn trailing_zeros(self) -> u32 {
            if self.lo != 0 { self.lo.trailing_zeros() } else if self.hi != 0 { 128 + self.hi.trailing_zeros() } else { 0 }
        }
        fn shr(self, bits: u32) -> Self {
            if bits == 0 || self.is_zero() { return self; }
            if bits >= 256 { return Self::ZERO; }
            let out = if bits >= 128 {
                Self { hi: 0, lo: self.hi >> (bits - 128), neg: self.neg }
            } else {
                Self { hi: self.hi >> bits, lo: (self.lo >> bits) | (self.hi << (128 - bits)), neg: self.neg }
            };
            if out.is_zero() { Self::ZERO } else { out }
        }
        pub(super) fn mul(self, o: Self) -> Option<Self> {
            if self.is_zero() || o.is_zero() { return Some(Self::ZERO); }
            if self.hi != 0 && o.hi != 0 { return None; }
            let (ll_hi, ll_lo) = mul_u128(self.lo, o.lo);
            let (c_hi, c_lo) = if self.hi == 0 { mul_u128(self.lo, o.hi) } else { mul_u128(self.hi, o.lo) };
            if c_hi != 0 { return None; }
            let (hi, overflow) = ll_hi.overflowing_add(c_lo);
            if overflow { None } else { Some(Self { hi, lo: ll_lo, neg: self.neg != o.neg }) }
        }
        pub(super) fn mul_i128(self, o: i128) -> Option<Self> { self.mul(Self::from_i128(o)) }
        fn rem_u128(self, d: u128) -> u128 {
            if d <= 1 { return 0; }
            if self.hi == 0 { return self.lo % d; }
            let two = (u128::MAX % d).saturating_add(1) % d;
            (mul_mod(self.hi % d, two, d) + (self.lo % d)) % d
        }
        fn div_u128(self, d: u128) -> Option<Self> {
            if d == 0 { return None; }
            if self.hi == 0 {
                let lo = self.lo / d;
                return Some(if lo == 0 { Self::ZERO } else { Self { hi: 0, lo, neg: self.neg } });
            }
            let (q_lo, rem) = div_mod_wide(self.hi % d, self.lo, d)?;
            if rem != 0 { return None; }
            let out = Self { hi: self.hi / d, lo: q_lo, neg: self.neg };
            Some(if out.is_zero() { Self::ZERO } else { out })
        }
    }

    fn mul_u128(a: u128, b: u128) -> (u128, u128) {
        let (a0, a1, b0, b1) = (a as u64 as u128, a >> 64, b as u64 as u128, b >> 64);
        let (ll, lh, hl, hh) = (a0 * b0, a0 * b1, a1 * b0, a1 * b1);
        let mid = (ll >> 64) + (lh & 0xffff_ffff_ffff_ffff) + (hl & 0xffff_ffff_ffff_ffff);
        (hh + (lh >> 64) + (hl >> 64) + (mid >> 64), (ll & 0xffff_ffff_ffff_ffff) | (mid << 64))
    }
    fn mul_mod(a: u128, b: u128, m: u128) -> u128 {
        if m <= 1 { return 0; }
        let (hi, lo) = mul_u128(a % m, b % m);
        if hi == 0 { return lo % m; }
        let two = (u128::MAX % m).saturating_add(1) % m;
        (mul_mod(hi, two, m) + (lo % m)) % m
    }
    fn div_mod_wide(hi: u128, lo: u128, d: u128) -> Option<(u128, u128)> {
        if d == 0 || hi >= d { return None; }
        if hi == 0 { return Some((lo / d, lo % d)); }
        let (mut q, mut r) = (0u128, hi);
        for i in (0..128).rev() {
            let carry = r >> 127;
            r = (r << 1) | ((lo >> i) & 1);
            if carry != 0 || r >= d {
                r = if carry != 0 { r.wrapping_sub(d) } else { r - d };
                q |= 1 << i;
            }
        }
        Some((q, r))
    }
    fn gcd_u128(mut a: u128, mut b: u128) -> u128 {
        while b != 0 { (a, b) = (b, a % b); }
        if a == 0 { 1 } else { a }
    }

    impl Ratio {
        pub(super) fn new(n: I256, d: i128, exp: i32) -> Option<Self> {
            if d == 0 { return None; }
            let (n, d) = if d < 0 { (n.wrapping_neg(), -d) } else { (n, d) };
            Some(Self { n, d, exp }.normalize())
        }
        pub(crate) fn from_i128(n: i128) -> Self {
            Self { n: I256::from_i128(n), d: 1, exp: 0 }.normalize()
        }
        pub(crate) fn is_zero(self) -> bool { self.n.is_zero() }
        fn normalize(self) -> Self {
            if self.n.is_zero() { return Self { n: I256::ZERO, d: 1, exp: 0 }; }
            let (mut n, mut d, mut exp) = (self.n, self.d, self.exp);
            let tz_d = d.trailing_zeros().min(30);
            d >>= tz_d;
            exp = exp.saturating_add(tz_d as i32);
            let tz_n = n.trailing_zeros().min(255);
            n = n.shr(tz_n);
            exp = exp.saturating_add(tz_n as i32);
            let g = gcd_u128(n.rem_u128(d as u128), d.unsigned_abs());
            if g > 1 && let Some(div) = n.div_u128(g) {
                n = div;
                d /= i128::try_from(g).unwrap_or(1);
            }
            Self { n, d, exp }
        }
        pub(crate) fn add(self, o: Self) -> Option<Self> {
            if self.is_zero() { return Some(o); }
            if o.is_zero() { return Some(self); }
            let (left, right, exp) = if self.exp >= o.exp {
                let sh = u32::try_from(self.exp - o.exp).ok()?;
                (self.n.mul_i128(o.d)?.checked_shl(sh)?, o.n.mul_i128(self.d)?, o.exp)
            } else {
                let sh = u32::try_from(o.exp - self.exp).ok()?;
                (self.n.mul_i128(o.d)?, o.n.mul_i128(self.d)?.checked_shl(sh)?, self.exp)
            };
            Self::new(left.add(right)?, self.d.checked_mul(o.d)?, exp)
        }
        pub(crate) fn sub(self, o: Self) -> Option<Self> {
            self.add(Self::new(o.n.wrapping_neg(), o.d, o.exp)?)
        }
        pub(crate) fn mul(self, o: Self) -> Option<Self> {
            Self::new(self.n.mul(o.n)?, self.d.checked_mul(o.d)?, self.exp.checked_add(o.exp)?)
        }
        pub(crate) fn div(self, o: Self) -> Option<Self> {
            if o.is_zero() || o.n.hi != 0 { return None; }
            let n = self.n.mul_i128(o.d)?;
            let den = i128::try_from(o.n.lo).ok()?.checked_mul(self.d)?;
            let n = if o.n.neg { n.wrapping_neg() } else { n };
            Self::new(n, den, self.exp.checked_sub(o.exp)?)
        }
        pub(crate) fn to_f64(self) -> f64 {
            if self.is_zero() { return 0.0; }
            let mag = if self.n.hi == 0 { self.n.lo as f64 } else { (self.n.hi as f64).mul_add(2.0f64.powi(128), self.n.lo as f64) };
            let signed = if self.n.neg { -mag } else { mag };
            signed / (self.d as f64) * 2.0f64.powi(self.exp)
        }
        pub(crate) fn abs_gt(self, o: Self) -> Option<bool> { Some(self.cmp_abs(o)? == Ordering::Greater) }
        fn cmp_abs(self, o: Self) -> Option<Ordering> {
            if self.is_zero() || o.is_zero() { return Some(self.n.cmp_mag(o.n)); }
            let (left, right) = (self.n.abs().mul_i128(o.d)?, o.n.abs().mul_i128(self.d)?);
            Some(if self.exp >= o.exp {
                match u32::try_from(self.exp - o.exp).ok().and_then(|sh| left.checked_shl(sh)) {
                    Some(l) => l.cmp_mag(right),
                    None => Ordering::Greater,
                }
            } else {
                match u32::try_from(o.exp - self.exp).ok().and_then(|sh| right.checked_shl(sh)) {
                    Some(r) => left.cmp_mag(r),
                    None => Ordering::Less,
                }
            })
        }
        fn cmp_abs_int(self, k: i32) -> Option<Ordering> { self.cmp_abs(Self::from_i128(i128::from(k))) }
        pub(crate) fn ceil_abs(self) -> Option<i32> {
            if self.is_zero() { return Some(0); }
            if self.cmp_abs_int(1)? == Ordering::Less { return Some(1); }
            if self.cmp_abs_int(i32::MAX)? == Ordering::Greater { return None; }
            let (mut lo, mut hi) = (1i32, i32::MAX);
            while lo < hi {
                let mid = lo + (hi - lo) / 2;
                if self.cmp_abs_int(mid)? == Ordering::Greater { lo = mid + 1; } else { hi = mid; }
            }
            Some(lo)
        }
    }

    pub(crate) fn ratio_from_f64(x: f64) -> Option<Ratio> {
        let (m, e) = decode_f64(x)?;
        Ratio::new(I256::from_i128(m), 1, e)
    }
    pub(crate) fn scaled_sample(value: f64, scale: f64) -> Option<Ratio> {
        let (m0, e0) = decode_f64(value)?;
        let (m1, e1) = decode_f64(scale)?;
        Ratio::new(I256::from_i128(m0).mul(I256::from_i128(m1))?, 1, e0.checked_add(e1)?)
    }
    pub(crate) fn lerp_ratio(t: Ratio, t0: Ratio, t1: Ratio, v0: Ratio, v1: Ratio) -> Option<Ratio> {
        let span = t1.sub(t0)?;
        if span.is_zero() { return None; }
        v0.mul(t1.sub(t)?)?.add(v1.mul(t.sub(t0)?)?)?.div(span)
    }
    fn decode_f64(x: f64) -> Option<(i128, i32)> {
        if !x.is_finite() { return None; }
        if x == 0.0 { return Some((0, 0)); }
        let bits = x.to_bits();
        let sign = if bits >> 63 == 0 { 1i128 } else { -1 };
        let exp_bits = ((bits >> 52) & 0x7ff) as i32;
        let frac = (bits & 0x000f_ffff_ffff_ffff) as i128;
        let (mant, exp) = if exp_bits == 0 { (frac, -1074) } else { (frac + (1i128 << 52), exp_bits - 1075) };
        Some((sign * mant, exp))
    }
}

pub(crate) use arith::{lerp_ratio, ratio_from_f64, scaled_sample};

#[cfg(test)]
mod tests {
    use super::{Ratio, scaled_sample};

    #[test]
    fn ceil_of_half_is_one() {
        let half = Ratio::from_i128(1).div(Ratio::from_i128(2)).unwrap();
        assert_eq!(half.ceil_abs(), Some(1));
        assert_eq!(Ratio::from_i128(0).ceil_abs(), Some(0));
        assert_eq!(Ratio::from_i128(2).ceil_abs(), Some(2));
    }

    #[test]
    fn tiny_dyadic_is_not_non_finite() {
        let r = scaled_sample(1e-300, 1.0).unwrap();
        assert_eq!(r.ceil_abs(), Some(1));
    }
}

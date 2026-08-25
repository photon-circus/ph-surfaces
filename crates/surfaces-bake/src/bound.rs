//! Exact `MAX_ERR_LSB` residual on the host.
//!
//! The baker may allocate and may take reviewed crates. `ceil` applies only to
//! the finished `|sample*scale − reconstruct|`. Do not implement the bound as
//! host `f64` lerp, ULP padding, an 8-ULP envelope, unreduced `i128` `n/d`, a
//! fixed-width integer with a ceil shortcut inside add, or a stand-in dyadic.
//! `NonFiniteDeviation` is for a true non-finite residual. `BoundOverflow` is
//! a finite ceil that does not fit `i32`.

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{Signed, ToPrimitive, Zero};

/// Exact rational used for host bilinear and the residual.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Ratio(BigRational);

impl Ratio {
    pub(crate) fn from_i128(n: i128) -> Self {
        Self(BigRational::from(BigInt::from(n)))
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    pub(crate) fn add(&self, other: &Self) -> Option<Self> {
        Some(Self(&self.0 + &other.0))
    }

    pub(crate) fn sub(&self, other: &Self) -> Option<Self> {
        Some(Self(&self.0 - &other.0))
    }

    pub(crate) fn mul(&self, other: &Self) -> Option<Self> {
        Some(Self(&self.0 * &other.0))
    }

    pub(crate) fn div(&self, other: &Self) -> Option<Self> {
        if other.is_zero() {
            return None;
        }
        Some(Self(&self.0 / &other.0))
    }

    /// Approximate the ratio as `f64` for the RMS statistic only.
    ///
    /// Independent `numer`/`denom` conversions overflow around `2^1024` even
    /// when the reduced ratio is ordinary (`1 + 1e-300`). Shift both limbs
    /// into the finite exponent range, but never farther than the smaller
    /// limb can survive: `1 / 2^1023` must not become `0`. This is not the
    /// bound.
    pub(crate) fn to_f64(&self) -> f64 {
        let n = self.0.numer();
        let d = self.0.denom();
        let n_bits = n.bits();
        let d_bits = d.bits();
        let shift = n_bits
            .max(d_bits)
            .saturating_sub(1023)
            .min(n_bits.min(d_bits).saturating_sub(1)) as usize;
        match ((n >> shift).to_f64(), (d >> shift).to_f64()) {
            (Some(n), Some(d)) if d != 0.0 => n / d,
            _ if self.0.is_negative() => f64::NEG_INFINITY,
            _ => f64::INFINITY,
        }
    }

    pub(crate) fn abs_gt(&self, other: &Self) -> Option<bool> {
        Some(self.0.abs() > other.0.abs())
    }

    pub(crate) fn ceil_abs(&self) -> Option<i32> {
        if self.is_zero() {
            return Some(0);
        }
        let abs = self.0.abs();
        let n = abs.numer();
        let d = abs.denom();
        let one = BigInt::from(1);
        ((n + d - one) / d).to_i32()
    }
}

pub(crate) fn ratio_from_f64(x: f64) -> Option<Ratio> {
    Some(Ratio(dyadic(x)?))
}

pub(crate) fn scaled_sample(value: f64, scale: f64) -> Option<Ratio> {
    Some(Ratio(dyadic(value)? * dyadic(scale)?))
}

pub(crate) fn lerp_ratio(
    t: &Ratio,
    t0: &Ratio,
    t1: &Ratio,
    v0: &Ratio,
    v1: &Ratio,
) -> Option<Ratio> {
    let span = t1.sub(t0)?;
    if span.is_zero() {
        return None;
    }
    v0.mul(&t1.sub(t)?)?.add(&v1.mul(&t.sub(t0)?)?)?.div(&span)
}

fn dyadic(x: f64) -> Option<BigRational> {
    if !x.is_finite() {
        return None;
    }
    if x == 0.0 {
        return Some(BigRational::from(BigInt::from(0)));
    }
    let bits = x.to_bits();
    let sign = if bits >> 63 == 0 { 1i128 } else { -1 };
    let exp_bits = ((bits >> 52) & 0x7ff) as i32;
    let frac = (bits & 0x000f_ffff_ffff_ffff) as i128;
    let (mant, exp) = if exp_bits == 0 {
        (frac, -1074)
    } else {
        (frac + (1i128 << 52), exp_bits - 1075)
    };
    let n = BigInt::from(sign * mant);
    if exp >= 0 {
        Some(BigRational::from(n << (exp as u32)))
    } else {
        Some(BigRational::new(n, BigInt::from(1) << ((-exp) as u32)))
    }
}

#[cfg(test)]
mod tests {
    use super::{Ratio, scaled_sample};

    #[test]
    fn ceil_of_half_is_one() {
        let half = Ratio::from_i128(1).div(&Ratio::from_i128(2)).unwrap();
        assert_eq!(half.ceil_abs(), Some(1));
        assert_eq!(Ratio::from_i128(0).ceil_abs(), Some(0));
        assert_eq!(Ratio::from_i128(2).ceil_abs(), Some(2));
    }

    #[test]
    fn tiny_dyadic_is_not_non_finite() {
        let r = scaled_sample(1e-300, 1.0).unwrap();
        assert_eq!(r.ceil_abs(), Some(1));
    }

    #[test]
    fn one_plus_or_minus_a_tiny_dyadic_is_exact() {
        let one = Ratio::from_i128(1);
        let tiny = scaled_sample(1e-300, 1.0).unwrap();
        assert_eq!(one.sub(&tiny).unwrap().ceil_abs(), Some(1));
        assert_eq!(tiny.sub(&one).unwrap().ceil_abs(), Some(1));
        assert_eq!(one.add(&tiny).unwrap().ceil_abs(), Some(2));
    }

    #[test]
    fn one_plus_a_tiny_dyadic_is_a_finite_f64() {
        let one = Ratio::from_i128(1);
        let tiny = scaled_sample(1e-300, 1.0).unwrap();
        let x = one.add(&tiny).unwrap().to_f64();
        assert!(x.is_finite());
        assert!((x - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_power_of_two_subnormal_to_f64_is_nonzero() {
        let r = scaled_sample(f64::MIN_POSITIVE / 2.0, 1.0).unwrap();
        let x = r.to_f64();
        assert!(x > 0.0);
        assert!(x.is_finite());
        assert_eq!(x, f64::MIN_POSITIVE / 2.0);
    }
}

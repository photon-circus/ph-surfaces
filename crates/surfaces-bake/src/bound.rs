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
    /// `num-rational` performs the conversion correctly rounded (round to
    /// even with sticky bits, subnormal handling, and overflow to infinity).
    /// A `BigRational` never carries a zero denominator, so the conversion
    /// cannot fail. This is not the bound.
    pub(crate) fn to_f64(&self) -> f64 {
        self.0.to_f64().unwrap_or(f64::NAN)
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
    use super::{Ratio, lerp_ratio, ratio_from_f64, scaled_sample};

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

    #[test]
    fn the_smallest_subnormal_to_f64_is_finite() {
        let tiny = f64::from_bits(1);
        let r = scaled_sample(tiny, 1.0).unwrap();
        let x = r.to_f64();
        assert!(x.is_finite());
        assert_eq!(x, tiny);
    }

    #[test]
    fn a_near_half_min_subnormal_times_scale_is_the_smallest_subnormal() {
        let value = f64::from_bits(0x0c70_0000_0000_0001);
        let scale = f64::from_bits(0x3040_0000_0000_0001);
        let x = scaled_sample(value, scale).unwrap().to_f64();
        assert!(x.is_finite());
        assert_eq!(x, f64::from_bits(1));
    }

    #[test]
    fn a_subnormal_lerp_residual_rounds_once_to_three_units() {
        // Knots [0, 17] × [0, 1], rows [0, 1] / [0, 1]. The off-knot sample
        // at x = 0x0088_0000_0000_0000, y = 0.5 has an exact residual of three
        // minimum-subnormal units. Applying 2^-1022 first rounds the
        // intermediate to seven units and the remaining ×0.5 to four.
        let x = ratio_from_f64(f64::from_bits(0x0088_0000_0000_0000)).unwrap();
        let y = ratio_from_f64(0.5).unwrap();
        let sample = scaled_sample(f64::from_bits(0x0046_9696_9696_9697), 1.0).unwrap();
        let x0 = Ratio::from_i128(0);
        let x1 = Ratio::from_i128(17);
        let y0 = Ratio::from_i128(0);
        let y1 = Ratio::from_i128(1);
        let v0 = Ratio::from_i128(0);
        let v1 = Ratio::from_i128(1);
        let row = lerp_ratio(&x, &x0, &x1, &v0, &v1).unwrap();
        let reconstructed = lerp_ratio(&y, &y0, &y1, &row, &row).unwrap();
        let residual = sample.sub(&reconstructed).unwrap();
        assert_eq!(residual.to_f64().abs(), f64::from_bits(3));
    }
}

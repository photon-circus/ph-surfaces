//! Private scalar segment interpolation.
//!
//! This module owns the one signed interpolation primitive the two-dimensional
//! evaluator needs, and the one rounding policy it applies. Both are crate
//! private: v0.1 exposes no general scalar-transfer API, and the shared
//! arithmetic question stays deferred until shipped duplication justifies it.
//!
//! Everything here is core integer arithmetic. There is no floating point, no
//! fixed-point layer, no allocator, and no external dependency.

/// Interpolates linearly between the two endpoints of one axis segment.
///
/// Given `offset = x - x0` and `span = x1 - x0`, this returns the signed value
/// mathematically represented by
///
/// ```text
/// (y0 * span + (y1 - y0) * offset) / span
/// ```
///
/// rounded to nearest, with exact half-way values rounded away from zero. The
/// implementation evaluates the equal nonnegative-weight arrangement
/// `y0 * (span - offset) + y1 * offset`, which avoids materializing `y1 - y0`
/// (that difference overflows `i32` for extreme endpoints) and gives the
/// tighter overflow bound recorded below.
///
/// # Preconditions
///
/// `x0 < x1`, and `x` lies in the closed interval `[x0, x1]`. These are
/// internal invariants established by validated axes and by axis lookup, not
/// runtime inputs, so they are checked only by `debug_assert!` and there is no
/// error outcome for them.
///
/// # Bounds
///
/// The intermediate arithmetic is 64-bit and cannot overflow for any valid
/// operands:
///
/// - the two weights are nonnegative and sum to `span`, which is at most
///   `65_535`, so the numerator magnitude is at most `2^31 * 65_535`, below
///   `2^47` (about `1.41e14`);
/// - the arrangement named in the contract, `y0 * span + (y1 - y0) * offset`,
///   has the wider envelope `2^31 * 65_535 + (2^32 - 1) * 65_535`, still below
///   `4.23e14`;
/// - the rounding term added before dividing is at most `span / 2`, so at most
///   `32_767`.
///
/// All of these are far below `i64::MAX` (about `9.22e18`).
///
/// Because both weights are nonnegative and sum to the divisor, the exact
/// quotient lies in the convex hull of `y0` and `y1`, and rounding to nearest
/// cannot leave that hull. The result therefore always fits `i32`, and there is
/// no overflow outcome to report.
pub(crate) fn interpolate_segment(x: u16, x0: u16, x1: u16, y0: i32, y1: i32) -> i32 {
    debug_assert!(x0 < x1, "segment endpoints must strictly increase");
    debug_assert!(x0 <= x, "x must not sit below the segment");
    debug_assert!(x <= x1, "x must not sit above the segment");

    // Widen before subtracting: `x1 - x0` in `u16` would panic on a violated
    // precondition instead of staying arithmetic.
    let span = i64::from(x1) - i64::from(x0);
    let offset = i64::from(x) - i64::from(x0);

    let numerator = i64::from(y0) * (span - offset) + i64::from(y1) * offset;
    let value = div_round_half_away_from_zero(numerator, span);

    debug_assert!(
        value >= i64::from(y0.min(y1)) && value <= i64::from(y0.max(y1)),
        "rounded value left the endpoint convex hull"
    );

    value as i32
}

/// Divides `numerator` by `denominator`, rounding to nearest and rounding
/// exact half-way values away from zero.
///
/// This is the sole implementation of that rounding policy in the crate. Every
/// interpolated value goes through it, so the policy has exactly one place to
/// read and one place to change.
///
/// # Preconditions
///
/// `denominator` is strictly positive. Callers derive it from a validated,
/// strictly increasing axis, so a zero or negative divisor is a bug rather than
/// an input, and is checked only by `debug_assert!`.
///
/// # Why this is exact
///
/// Rust integer division truncates toward zero. Adding half the divisor with
/// the sign of the numerator therefore rounds to nearest in both directions and
/// makes the function odd: the result for `-n` is exactly the negation of the
/// result for `n`. That symmetry is what makes sign-reflected segments produce
/// sign-reflected values.
///
/// The truncation in `denominator / 2` is harmless for an odd divisor. An exact
/// half-way value requires `2 * numerator == denominator * (2k + 1)`, which
/// forces `denominator` to be even; for an odd divisor no tie exists, and
/// adding `(denominator - 1) / 2` still selects the nearest quotient.
fn div_round_half_away_from_zero(numerator: i64, denominator: i64) -> i64 {
    debug_assert!(denominator > 0, "denominator must be strictly positive");

    let half = denominator / 2;

    if numerator >= 0 {
        (numerator + half) / denominator
    } else {
        (numerator - half) / denominator
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // On the "no floating point, allocator, unsafe, or external curve crate
    // path" criterion: a unit test cannot observe the absence of a code path,
    // so that evidence is mechanical rather than behavioural. It is carried by
    // `#![forbid(unsafe_code)]` in the crate root, the `integer only` grep in
    // `cargo xtask ci`, the `-Z build-std=core` core-only builds in the same
    // gate, and the `deny.toml` ban on the crate name. The tests below carry
    // the numerical contract instead.

    /// Independent reference for the same value.
    ///
    /// It deliberately uses the algebraic arrangement named in the contract,
    /// `y0 * span + (y1 - y0) * offset`, 128-bit arithmetic, and a remainder
    /// comparison rather than a biased-numerator division. It shares no code
    /// with the implementation, so agreement is evidence that the two
    /// arrangements are equal and that the rounding policies match.
    fn reference(x: u16, x0: u16, x1: u16, y0: i32, y1: i32) -> i32 {
        let span = i128::from(x1) - i128::from(x0);
        let offset = i128::from(x) - i128::from(x0);
        let numerator = i128::from(y0) * span + (i128::from(y1) - i128::from(y0)) * offset;

        let quotient = numerator / span;
        let remainder = numerator % span;

        let rounded = if remainder.abs() * 2 >= span {
            if numerator < 0 {
                quotient - 1
            } else {
                quotient + 1
            }
        } else {
            quotient
        };

        rounded as i32
    }

    /// `(x0, x1, y0, y1)` segments spanning the shapes the contract calls out:
    /// minimum and maximum spans, increasing, decreasing, flat, all-positive,
    /// all-negative, zero-crossing, and saturated `i32` endpoints in both
    /// directions.
    const SEGMENTS: &[(u16, u16, i32, i32)] = &[
        (0, 1, 0, 1),
        (0, 2, 0, 1),
        (0, 2, 0, 3),
        (0, 2, i32::MIN, i32::MAX),
        (0, 2, i32::MAX, i32::MIN),
        (0, 3, -7, 7),
        (0, 4, 42, 42),
        (0, 4, -100, -20),
        (0, 4, -100, 100),
        (0, 10, 0, 100),
        (0, 10, 100, 0),
        (7, 9, i32::MAX - 1, i32::MAX),
        (100, 101, -5, 5),
        (1000, 1007, 12345, -54321),
        (65534, 65535, i32::MIN, i32::MIN + 1),
        (0, 65535, 0, 65535),
        (0, 65535, 0, 1),
        (0, 65535, i32::MIN, i32::MAX),
        (0, 65535, i32::MAX, i32::MIN),
    ];

    /// Visits both endpoints, the two midpoints, and a bounded sweep of the
    /// interior. Wide segments are strided so the sweep stays cheap without
    /// skipping the tie-sensitive middle.
    fn for_each_sample(x0: u16, x1: u16, mut visit: impl FnMut(u16)) {
        let low = u32::from(x0);
        let high = u32::from(x1);
        let span = high - low;
        let stride = if span > 4096 { span / 4096 } else { 1 };

        let mut x = low;
        while x < high {
            visit(x as u16);
            x += stride;
        }
        visit(x1);

        let middle = low + span / 2;
        visit(middle as u16);
        visit((middle + 1).min(high) as u16);
    }

    #[test]
    fn agrees_with_independent_reference() {
        for &(x0, x1, y0, y1) in SEGMENTS {
            for_each_sample(x0, x1, |x| {
                assert_eq!(
                    interpolate_segment(x, x0, x1, y0, y1),
                    reference(x, x0, x1, y0, y1),
                    "segment ({x0}, {x1}, {y0}, {y1}) at {x}"
                );
            });
        }
    }

    #[test]
    fn endpoints_return_exact_endpoint_values() {
        for &(x0, x1, y0, y1) in SEGMENTS {
            assert_eq!(interpolate_segment(x0, x0, x1, y0, y1), y0);
            assert_eq!(interpolate_segment(x1, x0, x1, y0, y1), y1);
        }
    }

    #[test]
    fn increasing_decreasing_and_flat_segments() {
        assert_eq!(interpolate_segment(5, 0, 10, 0, 100), 50);
        assert_eq!(interpolate_segment(2, 0, 10, 0, 100), 20);

        assert_eq!(interpolate_segment(5, 0, 10, 100, 0), 50);
        assert_eq!(interpolate_segment(2, 0, 10, 100, 0), 80);

        for x in 0..=4 {
            assert_eq!(interpolate_segment(x, 0, 4, 42, 42), 42);
        }
    }

    #[test]
    fn positive_negative_and_zero_crossing_ranges() {
        assert_eq!(interpolate_segment(1, 0, 4, 20, 100), 40);

        assert_eq!(interpolate_segment(1, 0, 4, -100, -20), -80);
        assert_eq!(interpolate_segment(3, 0, 4, -100, -20), -40);

        assert_eq!(interpolate_segment(1, 0, 4, -100, 100), -50);
        assert_eq!(interpolate_segment(2, 0, 4, -100, 100), 0);
        assert_eq!(interpolate_segment(3, 0, 4, -100, 100), 50);
    }

    #[test]
    fn positive_half_way_values_round_away_from_zero() {
        // Exactly 0.5 and exactly 1.5, both rounded up.
        assert_eq!(interpolate_segment(1, 0, 2, 0, 1), 1);
        assert_eq!(interpolate_segment(1, 0, 2, 0, 3), 2);
        assert_eq!(interpolate_segment(1, 0, 2, 10, 11), 11);
    }

    #[test]
    fn negative_half_way_values_round_away_from_zero() {
        // Exactly -0.5 and -1.5, both rounded down.
        assert_eq!(interpolate_segment(1, 0, 2, 0, -1), -1);
        assert_eq!(interpolate_segment(1, 0, 2, 0, -3), -2);
        assert_eq!(interpolate_segment(1, 0, 2, -10, -11), -11);

        // The midpoint of the full `i32` range is exactly -0.5.
        assert_eq!(interpolate_segment(1, 0, 2, i32::MIN, i32::MAX), -1);
        assert_eq!(interpolate_segment(1, 0, 2, i32::MAX, i32::MIN), -1);
    }

    #[test]
    fn minimum_span_of_one_has_only_endpoints() {
        assert_eq!(interpolate_segment(65534, 65534, 65535, -5, 5), -5);
        assert_eq!(interpolate_segment(65535, 65534, 65535, -5, 5), 5);
        assert_eq!(interpolate_segment(0, 0, 1, i32::MIN, i32::MAX), i32::MIN);
        assert_eq!(interpolate_segment(1, 0, 1, i32::MIN, i32::MAX), i32::MAX);
    }

    #[test]
    fn maximum_span_covers_the_full_u16_domain() {
        // With `y` equal to `x` over the whole domain, every point is exact.
        for x in [0, 1, 2, 32767, 32768, 40000, 65533, 65534, 65535] {
            assert_eq!(interpolate_segment(x, 0, 65535, 0, 65535), i32::from(x));
        }

        // A unit rise over the widest span puts the tie just off centre.
        assert_eq!(interpolate_segment(32767, 0, 65535, 0, 1), 0);
        assert_eq!(interpolate_segment(32768, 0, 65535, 0, 1), 1);
    }

    #[test]
    fn extreme_i32_endpoints_in_both_directions() {
        for &(x0, x1) in &[(0u16, 1u16), (0, 2), (0, 65535), (65534, 65535)] {
            for &(y0, y1) in &[(i32::MIN, i32::MAX), (i32::MAX, i32::MIN)] {
                for_each_sample(x0, x1, |x| {
                    let value = interpolate_segment(x, x0, x1, y0, y1);
                    assert_eq!(value, reference(x, x0, x1, y0, y1));
                });
            }
        }
    }

    #[test]
    fn sign_reflected_segments_produce_sign_reflected_results() {
        for &(x0, x1, y0, y1) in SEGMENTS {
            // `i32::MIN` has no positive counterpart, so reflection is only
            // defined on the symmetric part of the range.
            if y0 == i32::MIN || y1 == i32::MIN {
                continue;
            }
            for_each_sample(x0, x1, |x| {
                assert_eq!(
                    interpolate_segment(x, x0, x1, -y0, -y1),
                    -interpolate_segment(x, x0, x1, y0, y1),
                    "segment ({x0}, {x1}, {y0}, {y1}) at {x}"
                );
            });
        }
    }

    #[test]
    fn results_stay_in_the_endpoint_convex_hull() {
        for &(x0, x1, y0, y1) in SEGMENTS {
            let low = y0.min(y1);
            let high = y0.max(y1);
            for_each_sample(x0, x1, |x| {
                let value = interpolate_segment(x, x0, x1, y0, y1);
                assert!(
                    (low..=high).contains(&value),
                    "segment ({x0}, {x1}, {y0}, {y1}) at {x} produced {value}"
                );
            });
        }
    }

    #[test]
    fn div_round_half_away_from_zero_rounds_exact_ties_away() {
        assert_eq!(div_round_half_away_from_zero(1, 2), 1);
        assert_eq!(div_round_half_away_from_zero(3, 2), 2);
        assert_eq!(div_round_half_away_from_zero(5, 2), 3);

        assert_eq!(div_round_half_away_from_zero(-1, 2), -1);
        assert_eq!(div_round_half_away_from_zero(-3, 2), -2);
        assert_eq!(div_round_half_away_from_zero(-5, 2), -3);

        assert_eq!(div_round_half_away_from_zero(2, 4), 1);
        assert_eq!(div_round_half_away_from_zero(-2, 4), -1);
    }

    #[test]
    fn div_round_half_away_from_zero_rounds_to_nearest_without_ties() {
        // An odd divisor admits no exact tie, so the truncated half must still
        // select the nearest quotient on both sides of the midpoint.
        assert_eq!(div_round_half_away_from_zero(4, 3), 1);
        assert_eq!(div_round_half_away_from_zero(5, 3), 2);
        assert_eq!(div_round_half_away_from_zero(7, 5), 1);
        assert_eq!(div_round_half_away_from_zero(8, 5), 2);

        assert_eq!(div_round_half_away_from_zero(-4, 3), -1);
        assert_eq!(div_round_half_away_from_zero(-5, 3), -2);
        assert_eq!(div_round_half_away_from_zero(-7, 5), -1);
        assert_eq!(div_round_half_away_from_zero(-8, 5), -2);
    }

    #[test]
    fn div_round_half_away_from_zero_handles_exact_and_trivial_cases() {
        assert_eq!(div_round_half_away_from_zero(0, 1), 0);
        assert_eq!(div_round_half_away_from_zero(0, 65535), 0);
        assert_eq!(div_round_half_away_from_zero(10, 5), 2);
        assert_eq!(div_round_half_away_from_zero(-10, 5), -2);

        // A unit divisor leaves every numerator untouched.
        for numerator in [i64::MIN, -1, 0, 1, i64::MAX] {
            assert_eq!(div_round_half_away_from_zero(numerator, 1), numerator);
        }
    }

    #[test]
    fn div_round_half_away_from_zero_is_odd() {
        let numerators = [0, 1, 2, 3, 7, 1234, 999_999, 422_000_000_000_000];
        let denominators = [1, 2, 3, 5, 4096, 65534, 65535];

        for numerator in numerators {
            for denominator in denominators {
                assert_eq!(
                    div_round_half_away_from_zero(-numerator, denominator),
                    -div_round_half_away_from_zero(numerator, denominator),
                    "numerator {numerator} over denominator {denominator}"
                );
            }
        }
    }

    #[test]
    fn division_stays_exact_at_the_documented_numerator_bound() {
        // The widest numerator the contract admits, plus the largest rounding
        // term, still divides without saturating 64-bit arithmetic.
        let span = 65535;
        let numerator = i64::from(i32::MIN) * span;
        assert_eq!(
            div_round_half_away_from_zero(numerator, span),
            i64::from(i32::MIN)
        );

        let numerator = i64::from(i32::MAX) * span;
        assert_eq!(
            div_round_half_away_from_zero(numerator, span),
            i64::from(i32::MAX)
        );
    }
}

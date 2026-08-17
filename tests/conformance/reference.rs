//! Independent reference implementation of the v0.1 numerical contract.
//!
//! Everything here is written from the contract text, not from `src/`. It
//! deliberately differs from the crate in every incidental choice so that
//! agreement is evidence rather than tautology:
//!
//! - arithmetic is `i128`, not `i64`;
//! - the segment numerator is arranged as `v0 * span + (v1 - v0) * offset`,
//!   not `v0 * (span - offset) + v1 * offset`;
//! - rounding compares the remainder against half the span instead of biasing
//!   the numerator before dividing;
//! - axis location is a linear scan of the fixture knot arrays, so it also
//!   serves as the simple oracle for every production locator;
//! - the result is narrowed with `i32::try_from` and a panic, so the reference
//!   itself asserts that every expected value is representable.
//!
//! The `mutant_*` functions each change exactly one contract point. They exist
//! so the suite can show that its fixture points *discriminate* the accepted
//! behaviour from the rejected one, and so the reader can see which
//! implementation mistake each test would catch.
//!
//! No `ph-curves`, no floating point, no dependency: `scripts/ci.sh` greps
//! this directory for both.

use ph_surfaces::{BilinearSurface, Boundary, BoundaryPolicy, SurfaceError};

/// Which side of an axis a coordinate fell off, before it is mapped to an
/// axis-specific [`SurfaceError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Below,
    Above,
}

/// Round `numerator / denominator` to nearest, exact half-way values away
/// from zero. `denominator` is positive.
fn div_round_ties_away(numerator: i128, denominator: i128) -> i128 {
    let quotient = numerator / denominator;
    let remainder = numerator % denominator;

    if remainder.abs() * 2 >= denominator {
        if numerator < 0 {
            quotient - 1
        } else {
            quotient + 1
        }
    } else {
        quotient
    }
}

/// Round `numerator / denominator` to nearest, with exact half-way values
/// toward zero: the *rejected* tie policy, kept for discrimination only.
fn div_round_ties_toward_zero(numerator: i128, denominator: i128) -> i128 {
    let quotient = numerator / denominator;
    let remainder = numerator % denominator;

    if remainder.abs() * 2 > denominator {
        if numerator < 0 {
            quotient - 1
        } else {
            quotient + 1
        }
    } else {
        quotient
    }
}

/// The exact signed rational value of the segment before rounding, as a
/// `(numerator, span)` pair. Shared by the accepted and the mutant rounding
/// so that only the rounding step differs between them.
fn segment_rational(t: u16, t0: u16, t1: u16, v0: i32, v1: i32) -> (i128, i128) {
    let span = i128::from(t1) - i128::from(t0);
    let offset = i128::from(t) - i128::from(t0);
    let numerator = i128::from(v0) * span + (i128::from(v1) - i128::from(v0)) * offset;
    (numerator, span)
}

fn narrow(value: i128) -> i32 {
    i32::try_from(value).expect("a v0.1 result is representable as i32")
}

/// Interpolates one segment under the contract's rounding policy.
///
/// Requires `t0 < t1` and `t0 <= t <= t1`.
pub fn segment(t: u16, t0: u16, t1: u16, v0: i32, v1: i32) -> i32 {
    assert!(t0 < t1, "reference: degenerate segment");
    assert!(t0 <= t && t <= t1, "reference: {t} outside [{t0}, {t1}]");
    let (numerator, span) = segment_rational(t, t0, t1, v0, v1);
    narrow(div_round_ties_away(numerator, span))
}

/// Mutant: the same segment with half-way values rounded toward zero.
pub fn mutant_segment_ties_toward_zero(t: u16, t0: u16, t1: u16, v0: i32, v1: i32) -> i32 {
    let (numerator, span) = segment_rational(t, t0, t1, v0, v1);
    narrow(div_round_ties_toward_zero(numerator, span))
}

/// Mutant: the segment extended linearly past its endpoints, without any
/// domain check. Used to show that clamped results are boundary values, not
/// extrapolated ones.
pub fn mutant_segment_extrapolating(t: u16, t0: u16, t1: u16, v0: i32, v1: i32) -> i32 {
    let (numerator, span) = segment_rational(t, t0, t1, v0, v1);
    narrow(div_round_ties_away(numerator, span))
}

/// Locates `t` on `axis` by linear scan.
///
/// Returns the lower knot index of the bracketing cell and the coordinate to
/// evaluate at (the input, or the endpoint knot it clamped to). Exact interior
/// knots and both inclusive endpoints resolve without changing the
/// coordinate. Out-of-domain coordinates follow `below` / `above`.
pub fn locate(
    axis: &[u16],
    t: u16,
    below: Boundary,
    above: Boundary,
) -> Result<(usize, u16), Side> {
    let last = axis.len() - 1;

    if t < axis[0] {
        return match below {
            Boundary::Error => Err(Side::Below),
            Boundary::Clamp => Ok((0, axis[0])),
        };
    }
    if t > axis[last] {
        return match above {
            Boundary::Error => Err(Side::Above),
            Boundary::Clamp => Ok((last - 1, axis[last])),
        };
    }

    // In domain. Scan for the last knot that is <= t, capped so the upper
    // endpoint lands in the final cell rather than past it.
    let mut lower = 0;
    for (index, &knot) in axis.iter().enumerate() {
        if knot <= t {
            lower = index;
        }
    }
    Ok((lower.min(last - 1), t))
}

fn map_x_side<const NX: usize>(x_axis: &[u16; NX], x: u16, side: Side) -> SurfaceError {
    match side {
        Side::Below => SurfaceError::XBelow {
            coordinate: x,
            bound: x_axis[0],
        },
        Side::Above => SurfaceError::XAbove {
            coordinate: x,
            bound: x_axis[NX - 1],
        },
    }
}

fn map_y_side<const NY: usize>(y_axis: &[u16; NY], y: u16, side: Side) -> SurfaceError {
    match side {
        Side::Below => SurfaceError::YBelow {
            coordinate: y,
            bound: y_axis[0],
        },
        Side::Above => SurfaceError::YAbove {
            coordinate: y,
            bound: y_axis[NY - 1],
        },
    }
}

fn locate_x_tables<const NX: usize>(
    x_axis: &[u16; NX],
    x: u16,
    policy: BoundaryPolicy,
) -> Result<(usize, u16), SurfaceError> {
    locate(x_axis, x, policy.x_below(), policy.x_above())
        .map_err(|side| map_x_side(x_axis, x, side))
}

fn locate_y_tables<const NY: usize>(
    y_axis: &[u16; NY],
    y: u16,
    policy: BoundaryPolicy,
) -> Result<(usize, u16), SurfaceError> {
    locate(y_axis, y, policy.y_below(), policy.y_above())
        .map_err(|side| map_y_side(y_axis, y, side))
}

/// X on the lower-Y row, X on the upper-Y row, then Y between the two rounded
/// results, given already-located cells.
fn compose_x_then_y_tables<const NX: usize, const NY: usize>(
    x_axis: &[u16; NX],
    y_axis: &[u16; NY],
    values: &[[i32; NX]; NY],
    (xi, xc): (usize, u16),
    (yi, yc): (usize, u16),
    seg: fn(u16, u16, u16, i32, i32) -> i32,
) -> i32 {
    let row = |r: usize| {
        seg(
            xc,
            x_axis[xi],
            x_axis[xi + 1],
            values[r][xi],
            values[r][xi + 1],
        )
    };
    let lower = row(yi);
    let upper = row(yi + 1);
    seg(yc, y_axis[yi], y_axis[yi + 1], lower, upper)
}

fn compose_y_then_x_tables<const NX: usize, const NY: usize>(
    x_axis: &[u16; NX],
    y_axis: &[u16; NY],
    values: &[[i32; NX]; NY],
    (xi, xc): (usize, u16),
    (yi, yc): (usize, u16),
) -> i32 {
    let column = |c: usize| {
        segment(
            yc,
            y_axis[yi],
            y_axis[yi + 1],
            values[yi][c],
            values[yi + 1][c],
        )
    };
    let left = column(xi);
    let right = column(xi + 1);
    segment(xc, x_axis[xi], x_axis[xi + 1], left, right)
}

fn extrapolating_cell(axis: &[u16], t: u16) -> (usize, u16) {
    let last = axis.len() - 1;
    if t < axis[0] {
        (0, t)
    } else if t > axis[last] {
        (last - 1, t)
    } else {
        let (index, _) = locate(axis, t, Boundary::Error, Boundary::Error)
            .expect("in-domain coordinate locates");
        (index, t)
    }
}

/// The accepted contract over fixture tables: X located before Y by a linear
/// scan of the supplied knot arrays, X interpolated on each row, then Y
/// between the two rounded row results.
///
/// Location never consults a production strategy. Callers that have a
/// `UniformAxis` surface pass the even-spaced knot array the descriptor
/// describes, not a search of `BinaryAxis`.
pub fn evaluate_tables<const NX: usize, const NY: usize>(
    x_axis: &[u16; NX],
    y_axis: &[u16; NY],
    values: &[[i32; NX]; NY],
    policy: BoundaryPolicy,
    x: u16,
    y: u16,
) -> Result<i32, SurfaceError> {
    let x_cell = locate_x_tables(x_axis, x, policy)?;
    let y_cell = locate_y_tables(y_axis, y, policy)?;
    Ok(compose_x_then_y_tables(
        x_axis, y_axis, values, x_cell, y_cell, segment,
    ))
}

/// Thin wrapper so existing default-binary tests keep calling `evaluate`.
pub fn evaluate<const NX: usize, const NY: usize>(
    surface: &BilinearSurface<NX, NY>,
    x: u16,
    y: u16,
) -> Result<i32, SurfaceError> {
    evaluate_tables(
        surface.x_axis(),
        surface.y_axis(),
        surface.values(),
        surface.policy(),
        x,
        y,
    )
}

/// Mutant: Y interpolated on each of the two columns first, then X between
/// the two rounded column results. Domain handling is unchanged.
pub fn mutant_evaluate_y_then_x_tables<const NX: usize, const NY: usize>(
    x_axis: &[u16; NX],
    y_axis: &[u16; NY],
    values: &[[i32; NX]; NY],
    policy: BoundaryPolicy,
    x: u16,
    y: u16,
) -> Result<i32, SurfaceError> {
    let x_cell = locate_x_tables(x_axis, x, policy)?;
    let y_cell = locate_y_tables(y_axis, y, policy)?;
    Ok(compose_y_then_x_tables(
        x_axis, y_axis, values, x_cell, y_cell,
    ))
}

/// Thin wrapper for existing default-binary tests.
pub fn mutant_evaluate_y_then_x<const NX: usize, const NY: usize>(
    surface: &BilinearSurface<NX, NY>,
    x: u16,
    y: u16,
) -> Result<i32, SurfaceError> {
    mutant_evaluate_y_then_x_tables(
        surface.x_axis(),
        surface.y_axis(),
        surface.values(),
        surface.policy(),
        x,
        y,
    )
}

/// Mutant: the accepted composition with the rejected rounding.
pub fn mutant_evaluate_ties_toward_zero_tables<const NX: usize, const NY: usize>(
    x_axis: &[u16; NX],
    y_axis: &[u16; NY],
    values: &[[i32; NX]; NY],
    policy: BoundaryPolicy,
    x: u16,
    y: u16,
) -> Result<i32, SurfaceError> {
    let x_cell = locate_x_tables(x_axis, x, policy)?;
    let y_cell = locate_y_tables(y_axis, y, policy)?;
    Ok(compose_x_then_y_tables(
        x_axis,
        y_axis,
        values,
        x_cell,
        y_cell,
        mutant_segment_ties_toward_zero,
    ))
}

/// Thin wrapper for existing default-binary tests.
pub fn mutant_evaluate_ties_toward_zero<const NX: usize, const NY: usize>(
    surface: &BilinearSurface<NX, NY>,
    x: u16,
    y: u16,
) -> Result<i32, SurfaceError> {
    mutant_evaluate_ties_toward_zero_tables(
        surface.x_axis(),
        surface.y_axis(),
        surface.values(),
        surface.policy(),
        x,
        y,
    )
}

/// Mutant: Y resolved before X, so a Y-side error wins over an X-side one.
/// Values are unchanged; only the reported error can differ.
pub fn mutant_evaluate_y_precedence_tables<const NX: usize, const NY: usize>(
    x_axis: &[u16; NX],
    y_axis: &[u16; NY],
    values: &[[i32; NX]; NY],
    policy: BoundaryPolicy,
    x: u16,
    y: u16,
) -> Result<i32, SurfaceError> {
    let y_cell = locate_y_tables(y_axis, y, policy)?;
    let x_cell = locate_x_tables(x_axis, x, policy)?;
    Ok(compose_x_then_y_tables(
        x_axis, y_axis, values, x_cell, y_cell, segment,
    ))
}

/// Thin wrapper for existing default-binary tests.
pub fn mutant_evaluate_y_precedence<const NX: usize, const NY: usize>(
    surface: &BilinearSurface<NX, NY>,
    x: u16,
    y: u16,
) -> Result<i32, SurfaceError> {
    mutant_evaluate_y_precedence_tables(
        surface.x_axis(),
        surface.y_axis(),
        surface.values(),
        surface.policy(),
        x,
        y,
    )
}

/// Mutant: every out-of-domain coordinate extrapolates from the nearest
/// boundary cell instead of erroring or clamping. This is what "permit
/// extrapolation" would look like.
pub fn mutant_evaluate_extrapolating_tables<const NX: usize, const NY: usize>(
    x_axis: &[u16; NX],
    y_axis: &[u16; NY],
    values: &[[i32; NX]; NY],
    x: u16,
    y: u16,
) -> i32 {
    compose_x_then_y_tables(
        x_axis,
        y_axis,
        values,
        extrapolating_cell(x_axis, x),
        extrapolating_cell(y_axis, y),
        mutant_segment_extrapolating,
    )
}

/// Thin wrapper for existing default-binary tests.
pub fn mutant_evaluate_extrapolating<const NX: usize, const NY: usize>(
    surface: &BilinearSurface<NX, NY>,
    x: u16,
    y: u16,
) -> i32 {
    mutant_evaluate_extrapolating_tables(surface.x_axis(), surface.y_axis(), surface.values(), x, y)
}

/// Mutant locator: the neighbouring cell, not the one that holds `t`.
///
/// Interpolation uses the wrong cell's knots with the original coordinate, so
/// the segment is extended past its endpoints. That is the "used the next
/// cell" mistake; `segment` would refuse it because `t` is then outside the
/// cell.
fn locate_off_by_one(
    axis: &[u16],
    t: u16,
    below: Boundary,
    above: Boundary,
) -> Result<(usize, u16), Side> {
    let (lower, coord) = locate(axis, t, below, above)?;
    let last_cell = axis.len() - 2;
    let wrong = if lower < last_cell {
        lower + 1
    } else if lower > 0 {
        lower - 1
    } else {
        lower
    };
    Ok((wrong, coord))
}

/// Evaluates with an off-by-one X cell and a correctly located Y cell.
pub fn mutant_evaluate_off_by_one_cell<const NX: usize, const NY: usize>(
    x_axis: &[u16; NX],
    y_axis: &[u16; NY],
    values: &[[i32; NX]; NY],
    policy: BoundaryPolicy,
    x: u16,
    y: u16,
) -> Result<i32, SurfaceError> {
    let x_cell = locate_off_by_one(x_axis, x, policy.x_below(), policy.x_above())
        .map_err(|side| map_x_side(x_axis, x, side))?;
    let y_cell = locate_y_tables(y_axis, y, policy)?;
    Ok(compose_x_then_y_tables(
        x_axis,
        y_axis,
        values,
        x_cell,
        y_cell,
        mutant_segment_extrapolating,
    ))
}

/// Evenly spaced knots `ORIGIN + i * STEP` for `i` in `0..N`.
///
/// Used only to build a *wrong* Uniform descriptor for locator mutants. The
/// accepted oracle still scans the fixture knot array.
pub const fn uniform_knots<const N: usize>(origin: u16, step: u16) -> [u16; N] {
    let mut knots = [0u16; N];
    let mut i = 0;
    while i < N {
        knots[i] = ((origin as u32) + (i as u32) * (step as u32)) as u16;
        i += 1;
    }
    knots
}

/// Mutant: locate and interpolate X against a Uniform descriptor whose origin
/// is one larger than the fixture's. The value grid is unchanged.
pub fn mutant_evaluate_uniform_origin_off_by_one<const NX: usize, const NY: usize>(
    origin: u16,
    step: u16,
    y_axis: &[u16; NY],
    values: &[[i32; NX]; NY],
    policy: BoundaryPolicy,
    x: u16,
    y: u16,
) -> Result<i32, SurfaceError> {
    let wrong = uniform_knots::<NX>(origin.saturating_add(1), step);
    evaluate_tables(&wrong, y_axis, values, policy, x, y)
}

/// Mutant: locate and interpolate X against a Uniform descriptor whose step
/// is one larger than the fixture's. The value grid is unchanged.
pub fn mutant_evaluate_uniform_step_off_by_one<const NX: usize, const NY: usize>(
    origin: u16,
    step: u16,
    y_axis: &[u16; NY],
    values: &[[i32; NX]; NY],
    policy: BoundaryPolicy,
    x: u16,
    y: u16,
) -> Result<i32, SurfaceError> {
    let wrong = uniform_knots::<NX>(origin, step.saturating_add(1));
    evaluate_tables(&wrong, y_axis, values, policy, x, y)
}

/// The cell with the largest knot span: the tail of a clustered-then-tail axis.
fn tail_cell(axis: &[u16]) -> usize {
    let last_cell = axis.len() - 2;
    let mut tail = 0;
    let mut max_span = 0u16;
    for i in 0..=last_cell {
        let span = axis[i + 1] - axis[i];
        if span >= max_span {
            max_span = span;
            tail = i;
        }
    }
    tail
}

/// Mutant: a tail-cell coordinate is located in the last cluster cell, and a
/// cluster coordinate is located in the tail. Discriminates a bucketed search
/// that starts in the wrong region of a clustered-then-tail axis.
pub fn mutant_evaluate_bucketed_cluster_vs_tail<const NX: usize, const NY: usize>(
    x_axis: &[u16; NX],
    y_axis: &[u16; NY],
    values: &[[i32; NX]; NY],
    policy: BoundaryPolicy,
    x: u16,
    y: u16,
) -> Result<i32, SurfaceError> {
    let (lower, xc) = locate_x_tables(x_axis, x, policy)?;
    let tail = tail_cell(x_axis);
    let cluster = tail.saturating_sub(1);
    let wrong = if lower == tail { cluster } else { tail };
    let y_cell = locate_y_tables(y_axis, y, policy)?;
    Ok(compose_x_then_y_tables(
        x_axis,
        y_axis,
        values,
        (wrong, xc),
        y_cell,
        mutant_segment_extrapolating,
    ))
}

/// Bit 0 selects X-below, bit 1 X-above, bit 2 Y-below, bit 3 Y-above; a set
/// bit means Clamp. Every one of the sixteen policies is `policy_from_bits(n)`
/// for exactly one `n`.
pub const fn side_from_bit(bits: usize, shift: u32) -> Boundary {
    if (bits >> shift) & 1 == 0 {
        Boundary::Error
    } else {
        Boundary::Clamp
    }
}

pub const fn policy_from_bits(bits: usize) -> BoundaryPolicy {
    BoundaryPolicy::new()
        .with_x_below(side_from_bit(bits, 0))
        .with_x_above(side_from_bit(bits, 1))
        .with_y_below(side_from_bit(bits, 2))
        .with_y_above(side_from_bit(bits, 3))
}

/// The smallest and largest value stored anywhere in the grid: the hull no
/// non-extrapolating result may leave.
pub fn value_hull_tables<const NX: usize, const NY: usize>(values: &[[i32; NX]; NY]) -> (i32, i32) {
    let mut lo = i32::MAX;
    let mut hi = i32::MIN;
    for row in values {
        for &v in row {
            lo = lo.min(v);
            hi = hi.max(v);
        }
    }
    (lo, hi)
}

/// Thin wrapper so existing default-binary tests keep calling `value_hull`.
pub fn value_hull<const NX: usize, const NY: usize>(
    surface: &BilinearSurface<NX, NY>,
) -> (i32, i32) {
    value_hull_tables(surface.values())
}

/// Evaluates every point of the declared Cartesian domain of the supplied
/// knot arrays against the table-based reference and asserts exact agreement.
/// Callers use this only where the domain is small enough that an exhaustive
/// claim is honest; the count is returned so tests can state it.
pub fn assert_exhaustive_domain_tables<const NX: usize, const NY: usize>(
    evaluate_surface: impl Fn(u16, u16) -> Result<i32, SurfaceError>,
    x_axis: &[u16; NX],
    y_axis: &[u16; NY],
    values: &[[i32; NX]; NY],
    policy: BoundaryPolicy,
    name: &str,
) -> u64 {
    let mut count = 0u64;
    for y in y_axis[0]..=y_axis[NY - 1] {
        for x in x_axis[0]..=x_axis[NX - 1] {
            assert_eq!(
                evaluate_surface(x, y),
                evaluate_tables(x_axis, y_axis, values, policy, x, y),
                "{name}: ({x}, {y}) disagrees with the reference"
            );
            count += 1;
        }
    }
    count
}

/// Thin wrapper so existing default-binary tests keep calling
/// `assert_exhaustive_domain`.
pub fn assert_exhaustive_domain<const NX: usize, const NY: usize>(
    surface: &BilinearSurface<NX, NY>,
    name: &str,
) -> u64 {
    assert_exhaustive_domain_tables(
        |x, y| surface.evaluate(x, y),
        surface.x_axis(),
        surface.y_axis(),
        surface.values(),
        surface.policy(),
        name,
    )
}

/// Asserts that every declared knot returns its stored value exactly.
pub fn assert_every_knot_exact_tables<const NX: usize, const NY: usize>(
    evaluate_surface: impl Fn(u16, u16) -> Result<i32, SurfaceError>,
    x_axis: &[u16; NX],
    y_axis: &[u16; NY],
    values: &[[i32; NX]; NY],
    name: &str,
) {
    for (row, &y) in y_axis.iter().enumerate() {
        for (column, &x) in x_axis.iter().enumerate() {
            assert_eq!(
                evaluate_surface(x, y),
                Ok(values[row][column]),
                "{name}: knot ({x}, {y}) at values[{row}][{column}]"
            );
        }
    }
}

/// Thin wrapper so existing default-binary tests keep calling
/// `assert_every_knot_exact`.
pub fn assert_every_knot_exact<const NX: usize, const NY: usize>(
    surface: &BilinearSurface<NX, NY>,
    name: &str,
) {
    assert_every_knot_exact_tables(
        |x, y| surface.evaluate(x, y),
        surface.x_axis(),
        surface.y_axis(),
        surface.values(),
        name,
    )
}

/// The coordinates a sampled sweep of `[lo, hi]` visits: both endpoints, their
/// nearest neighbours, the two middle values (which is where a half-way tie
/// on an even span lives), and a stride of at most `steps` interior points.
///
/// This is a *sample*. Tests that use it say so and make no exhaustive claim.
pub fn sample_axis(lo: u16, hi: u16, steps: u32) -> Vec<u16> {
    let low = u32::from(lo);
    let high = u32::from(hi);
    let span = high - low;
    let stride = (span / steps).max(1);

    let mut out = Vec::new();
    let mut t = low;
    while t < high {
        out.push(t as u16);
        t += stride;
    }
    let middle = low + span / 2;
    for extra in [low + 1, middle, middle + 1, high - 1, high] {
        if extra >= low && extra <= high {
            out.push(extra as u16);
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

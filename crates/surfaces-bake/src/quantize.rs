//! Fill the declared grid, quantize to i32, and measure sample deviation.
//!
//! This is not a fitter. Each declared node is taken from an on-knot sample;
//! off-knot samples participate only in the deviation measurement.

use crate::BakeInput;
use crate::bound::{Ratio, lerp_ratio, ratio_from_f64, scaled_sample};
use crate::error::{AxisName, BakeError};
use crate::samples::Sample;

/// Quantized row-major grid plus deviation of that table from the samples.
///
/// `max_err_lsb` is an i32 value LSB (not a color LSB). It is an upper
/// bound on the maximum absolute deviation between the supplied samples
/// and the table built from them, not a typical error, and not a device,
/// vendor, sensor, calibration, accuracy, timing, flash, or WCET claim.
#[derive(Clone, Debug, PartialEq)]
pub struct QuantizedTable {
    /// Declared X knots, expanded if the axis was uniform.
    pub x: Vec<u16>,
    /// Declared Y knots, expanded if the axis was uniform.
    pub y: Vec<u16>,
    /// Row-major `values[y][x]` after `round(value * scale)` to `i32`.
    pub values: Vec<Vec<i32>>,
    /// Caller-stated output scale applied during quantization.
    pub scale: f64,
    /// Durable bound: ceil of the exact rational `|sample*scale − reconstruct|`.
    pub max_err_lsb: i32,
    /// RMS of sample deviations in i32 value LSBs. Operator statistic;
    /// scaled before squaring so a tiny representable residual does not
    /// underflow to 0. Not `MAX_ERR_LSB`.
    pub rms_lsb: f64,
    /// X coordinate of the sample with the largest absolute LSB deviation.
    pub worst_x: f64,
    /// Y coordinate of the sample with the largest absolute LSB deviation.
    pub worst_y: f64,
    /// Signed LSB residual at each declared node, row-major `[y][x]`.
    pub per_knot_lsb: Vec<Vec<f64>>,
}

impl QuantizedTable {
    /// Host `f64` bilinear of the quantized grid, X then Y, then `/ scale`.
    ///
    /// Interpolation runs on `i32 as f64` so endpoint differences stay in
    /// range; the result is then divided by scale. Samples whose coordinates
    /// are exact `u16` values also contribute the runtime-rounded path via
    /// [`Self::evaluate_u16`]. `MAX_ERR_LSB` is `ceil` of the exact rational
    /// residual, not this `f64` reconstruction. Fractional coordinates
    /// cannot call runtime `evaluate`; this method stays a host `f64` view.
    ///
    /// `x` and `y` must lie in the inclusive declared domain.
    #[must_use]
    pub fn reconstruct(&self, x: f64, y: f64) -> f64 {
        reconstruct_i32(&self.x, &self.y, &self.values, x, y) / self.scale
    }

    /// Runtime-equivalent X-then-Y evaluation of the quantized `i32` grid.
    ///
    /// Each scalar step rounds to nearest, exact half-way away from zero,
    /// matching `BilinearSurface::evaluate`. `x` and `y` must lie in the
    /// inclusive declared domain.
    #[must_use]
    pub fn evaluate_u16(&self, x: u16, y: u16) -> i32 {
        evaluate_u16(&self.x, &self.y, &self.values, x, y)
    }
}

/// Source fragment so later emission can take the bound as an argument.
///
/// Emits `pub const MAX_ERR_LSB: i32 = …;` — an i32 value LSB, an upper
/// bound on the maximum absolute deviation between supplied samples and
/// the table built from them.
#[must_use]
pub fn emit_max_err_lsb(max_err_lsb: i32) -> String {
    format!("pub const MAX_ERR_LSB: i32 = {max_err_lsb};\n")
}

impl BakeInput {
    /// Fills each declared node, applies [`BakeInput::scale`], and measures
    /// deviation from every supplied sample.
    ///
    /// A sample whose X and Y equal a declared knot (`f64` equality with
    /// `f64::from` of that `u16`) supplies that node's pre-scale value.
    /// Off-knot samples do not fill a node. Rounding is nearest, with exact
    /// half-way values away from zero — the same policy as runtime
    /// interpolation, implemented here for `f64` rather than imported from
    /// the integer kernel. Samples whose coordinates are exact `u16` values
    /// also measure the runtime-rounded X-then-Y path; `max_err_lsb` is
    /// `ceil` of the exact rational residual of both (IEEE `f64` bit-patterns
    /// as dyadics, bilinear as a ratio of the `i32` grid). Host `f64` lerp is
    /// not the bound oracle.
    ///
    /// # Errors
    ///
    /// Returns [`BakeError::NonInvertibleScale`] when `1 / scale` is not
    /// finite, [`BakeError::GridTooLarge`] when `NX * NY` exceeds
    /// [`crate::MAX_GRID_CELLS`], [`BakeError::MissingNode`] or
    /// [`BakeError::AmbiguousNode`] for the declared grid,
    /// [`BakeError::QuantizeOverflow`] when a rounded value leaves `i32`,
    /// [`BakeError::BoundOverflow`] when a finite residual's ceil does not
    /// fit in `i32`, or [`BakeError::NonFiniteDeviation`] when a residual
    /// is not a finite LSB quantity.
    pub fn quantize(&self) -> Result<QuantizedTable, BakeError> {
        if !(1.0 / self.scale()).is_finite() {
            return Err(BakeError::NonInvertibleScale);
        }
        let x = self.x().knot_list(AxisName::X)?;
        let y = self.y().knot_list(AxisName::Y)?;
        let cells = x
            .len()
            .checked_mul(y.len())
            .filter(|&n| n <= crate::MAX_GRID_CELLS);
        if cells.is_none() {
            return Err(BakeError::GridTooLarge {
                nx: x.len(),
                ny: y.len(),
            });
        }
        let filled = fill_nodes(self.samples(), &x, &y)?;
        let values = quantize_grid(&filled, &x, &y, self.scale())?;
        deviation(self.samples(), x, y, filled, values, self.scale())
    }
}

fn reconstruct_ratio(
    x_knots: &[u16],
    y_knots: &[u16],
    values: &[Vec<i32>],
    x: f64,
    y: f64,
) -> Option<Ratio> {
    let xi = segment(x_knots, x);
    let yi = segment(y_knots, y);
    let x0 = Ratio::from_i128(i128::from(x_knots[xi]));
    let x1 = Ratio::from_i128(i128::from(x_knots[xi + 1]));
    let y0 = Ratio::from_i128(i128::from(y_knots[yi]));
    let y1 = Ratio::from_i128(i128::from(y_knots[yi + 1]));
    let xr = ratio_from_f64(x)?;
    let yr = ratio_from_f64(y)?;
    let q = |row: usize, col: usize| Ratio::from_i128(i128::from(values[row][col]));
    let lower = lerp_ratio(&xr, &x0, &x1, &q(yi, xi), &q(yi, xi + 1))?;
    let upper = lerp_ratio(&xr, &x0, &x1, &q(yi + 1, xi), &q(yi + 1, xi + 1))?;
    lerp_ratio(&yr, &y0, &y1, &lower, &upper)
}

#[allow(clippy::float_cmp)] // exact f64 equality is the specified node and duplicate test
fn fill_nodes(samples: &[Sample], x: &[u16], y: &[u16]) -> Result<Vec<Vec<f64>>, BakeError> {
    let mut cells = vec![vec![None; x.len()]; y.len()];
    for sample in samples {
        let Some(xi) = knot_index(x, sample.x) else {
            continue;
        };
        let Some(yi) = knot_index(y, sample.y) else {
            continue;
        };
        match cells[yi][xi] {
            None => cells[yi][xi] = Some(sample.value),
            Some(existing) if existing == sample.value => {}
            Some(_) => {
                return Err(BakeError::AmbiguousNode { x: x[xi], y: y[yi] });
            }
        }
    }
    let mut filled = vec![vec![0.0; x.len()]; y.len()];
    for (yi, row) in cells.iter().enumerate() {
        for (xi, cell) in row.iter().enumerate() {
            match *cell {
                Some(value) => filled[yi][xi] = value,
                None => return Err(BakeError::MissingNode { x: x[xi], y: y[yi] }),
            }
        }
    }
    Ok(filled)
}

fn knot_index(knots: &[u16], coord: f64) -> Option<usize> {
    exact_u16(coord).and_then(|n| knots.binary_search(&n).ok())
}

fn quantize_grid(
    filled: &[Vec<f64>],
    x: &[u16],
    y: &[u16],
    scale: f64,
) -> Result<Vec<Vec<i32>>, BakeError> {
    let mut values = vec![vec![0; x.len()]; y.len()];
    for (yi, row) in filled.iter().enumerate() {
        for (xi, value) in row.iter().enumerate() {
            values[yi][xi] = round_to_i32(value * scale)
                .ok_or(BakeError::QuantizeOverflow { x: x[xi], y: y[yi] })?;
        }
    }
    Ok(values)
}

/// Round to nearest; exact half-way values away from zero. Host `f64` helper.
///
/// Do not use `x ± 0.5` then `floor`/`ceil`: the `f64` immediately below
/// `0.5` becomes `1.0` after that add, which would quantize to `1`.
fn round_to_i32(value: f64) -> Option<i32> {
    if !value.is_finite() {
        return None;
    }
    let trunc = value.trunc();
    let frac = value - trunc;
    let rounded = if frac.abs() >= 0.5 {
        trunc + frac.signum()
    } else {
        trunc
    };
    if rounded > f64::from(i32::MAX) || rounded < f64::from(i32::MIN) {
        return None;
    }
    Some(rounded as i32)
}

fn deviation(
    samples: &[Sample],
    x: Vec<u16>,
    y: Vec<u16>,
    filled: Vec<Vec<f64>>,
    values: Vec<Vec<i32>>,
    scale: f64,
) -> Result<QuantizedTable, BakeError> {
    let mut per_knot_lsb = vec![vec![0.0; x.len()]; y.len()];
    for yi in 0..y.len() {
        for xi in 0..x.len() {
            per_knot_lsb[yi][xi] = filled[yi][xi].mul_add(scale, -f64::from(values[yi][xi]));
            if !per_knot_lsb[yi][xi].is_finite() {
                return Err(BakeError::NonFiniteDeviation);
            }
        }
    }
    let mut lsbs = Vec::with_capacity(samples.len());
    let mut max_ratio: Option<Ratio> = None;
    let mut max_err_lsb = 0i32;
    let mut worst_x = samples[0].x;
    let mut worst_y = samples[0].y;
    for sample in samples {
        let residual = sample_residual(&x, &y, &values, scale, sample)?;
        let ceil = residual.ceil_abs().ok_or(BakeError::BoundOverflow)?;
        if ceil > max_err_lsb {
            max_err_lsb = ceil;
        }
        if max_ratio
            .as_ref()
            .is_none_or(|m| residual.abs_gt(m) == Some(true))
        {
            max_ratio = Some(residual.clone());
            worst_x = sample.x;
            worst_y = sample.y;
        }
        let lsb = residual.to_f64();
        if !lsb.is_finite() {
            return Err(BakeError::NonFiniteDeviation);
        }
        lsbs.push(lsb);
    }
    Ok(QuantizedTable {
        x,
        y,
        values,
        scale,
        max_err_lsb,
        rms_lsb: rms_lsb(&lsbs),
        worst_x,
        worst_y,
        per_knot_lsb,
    })
}

/// Operator RMS in LSB. Scale before squaring so values near `1e-200` do not
/// underflow the sum to zero. This is not `MAX_ERR_LSB`.
fn rms_lsb(lsbs: &[f64]) -> f64 {
    let n = lsbs.len() as f64;
    let scale = lsbs.iter().fold(0.0_f64, |m, x| m.max(x.abs()));
    if n == 0.0 || scale == 0.0 {
        return 0.0;
    }
    let sum_unit_sq: f64 = lsbs
        .iter()
        .map(|x| {
            let u = x / scale;
            u * u
        })
        .sum();
    (sum_unit_sq / n).sqrt() * scale
}

fn sample_residual(
    x: &[u16],
    y: &[u16],
    values: &[Vec<i32>],
    scale: f64,
    sample: &Sample,
) -> Result<Ratio, BakeError> {
    let scaled = scaled_sample(sample.value, scale).ok_or(BakeError::NonFiniteDeviation)?;
    let bilinear =
        reconstruct_ratio(x, y, values, sample.x, sample.y).ok_or(BakeError::NonFiniteDeviation)?;
    let mut residual = scaled.sub(&bilinear).ok_or(BakeError::NonFiniteDeviation)?;
    if let (Some(px), Some(py)) = (exact_u16(sample.x), exact_u16(sample.y)) {
        let runtime = Ratio::from_i128(i128::from(evaluate_u16(x, y, values, px, py)));
        let runtime_residual = scaled.sub(&runtime).ok_or(BakeError::NonFiniteDeviation)?;
        if runtime_residual.abs_gt(&residual) == Some(true) {
            residual = runtime_residual;
        }
    }
    Ok(residual)
}

#[allow(clippy::float_cmp)] // exact u16 means f64::from(n) equals the sample coordinate
fn exact_u16(coord: f64) -> Option<u16> {
    if !coord.is_finite() || coord < 0.0 || coord > f64::from(u16::MAX) {
        return None;
    }
    let n = coord as u16;
    (f64::from(n) == coord).then_some(n)
}

fn evaluate_u16(x_knots: &[u16], y_knots: &[u16], values: &[Vec<i32>], x: u16, y: u16) -> i32 {
    let xi = segment(x_knots, f64::from(x));
    let yi = segment(y_knots, f64::from(y));
    let x0 = x_knots[xi];
    let x1 = x_knots[xi + 1];
    let y0 = y_knots[yi];
    let y1 = y_knots[yi + 1];
    let lower = interpolate_i32(x, x0, x1, values[yi][xi], values[yi][xi + 1]);
    let upper = interpolate_i32(x, x0, x1, values[yi + 1][xi], values[yi + 1][xi + 1]);
    interpolate_i32(y, y0, y1, lower, upper)
}

/// Host copy of the runtime scalar interpolator for bound measurement.
fn interpolate_i32(x: u16, x0: u16, x1: u16, y0: i32, y1: i32) -> i32 {
    let span = i64::from(x1) - i64::from(x0);
    let offset = i64::from(x) - i64::from(x0);
    let numerator = i64::from(y0) * (span - offset) + i64::from(y1) * offset;
    div_round_half_away_from_zero(numerator, span) as i32
}

fn div_round_half_away_from_zero(numerator: i64, denominator: i64) -> i64 {
    let half = denominator / 2;
    if numerator >= 0 {
        (numerator + half) / denominator
    } else {
        (numerator - half) / denominator
    }
}

fn reconstruct_i32(x_knots: &[u16], y_knots: &[u16], values: &[Vec<i32>], x: f64, y: f64) -> f64 {
    let xi = segment(x_knots, x);
    let yi = segment(y_knots, y);
    let x0 = f64::from(x_knots[xi]);
    let x1 = f64::from(x_knots[xi + 1]);
    let y0 = f64::from(y_knots[yi]);
    let y1 = f64::from(y_knots[yi + 1]);
    let lower = lerp(
        x,
        x0,
        x1,
        f64::from(values[yi][xi]),
        f64::from(values[yi][xi + 1]),
    );
    let upper = lerp(
        x,
        x0,
        x1,
        f64::from(values[yi + 1][xi]),
        f64::from(values[yi + 1][xi + 1]),
    );
    lerp(y, y0, y1, lower, upper)
}

fn segment(knots: &[u16], coord: f64) -> usize {
    let last = knots.len() - 1;
    match knots.binary_search_by(|knot| f64::from(*knot).total_cmp(&coord)) {
        Ok(i) if i == last => last - 1,
        Ok(i) => i,
        Err(0) => 0,
        Err(i) if i > last => last - 1,
        Err(i) => i - 1,
    }
}

fn lerp(t: f64, t0: f64, t1: f64, v0: f64, v1: f64) -> f64 {
    let u = (t - t0) / (t1 - t0);
    v0.mul_add(1.0 - u, v1 * u)
}

#[cfg(test)]
mod tests {
    use super::{emit_max_err_lsb, round_to_i32};
    use crate::{Axis, BakeError, BakeInput, Sample};
    use ph_surfaces::BilinearSurface;

    fn corners(scale: f64) -> BakeInput {
        BakeInput::parse(
            "0 0 1.5\n10 0 2.5\n0 5 3.5\n10 5 4.5\n",
            Axis::knots(vec![0, 10]),
            Axis::knots(vec![0, 5]),
            scale,
        )
        .unwrap()
    }

    #[test]
    fn on_knot_samples_fill_the_row_major_i32_grid() {
        let table = corners(1000.0).quantize().unwrap();
        assert_eq!(table.x, vec![0, 10]);
        assert_eq!(table.y, vec![0, 5]);
        assert_eq!(table.values, vec![vec![1500, 2500], vec![3500, 4500]]);
        assert_eq!(table.scale, 1000.0);
        assert_eq!(table.max_err_lsb, 0);
        assert_eq!(table.rms_lsb, 0.0);
        assert_eq!(table.per_knot_lsb, vec![vec![0.0, 0.0], vec![0.0, 0.0]]);
    }

    #[test]
    fn half_away_from_zero_matches_the_runtime_tie_policy() {
        assert_eq!(round_to_i32(1.5), Some(2));
        assert_eq!(round_to_i32(2.5), Some(3));
        assert_eq!(round_to_i32(-1.5), Some(-2));
        assert_eq!(round_to_i32(-2.5), Some(-3));
        assert_eq!(round_to_i32(0.5), Some(1));
        assert_eq!(round_to_i32(-0.5), Some(-1));
        assert_eq!(round_to_i32(1.4), Some(1));
        assert_eq!(round_to_i32(-1.4), Some(-1));
        assert_eq!(round_to_i32(f64::from(i32::MAX)), Some(i32::MAX));
        assert_eq!(round_to_i32(f64::from(i32::MIN)), Some(i32::MIN));
        assert_eq!(round_to_i32(f64::from(i32::MAX) + 0.5), None);
        assert_eq!(round_to_i32(f64::INFINITY), None);
        let just_below_half = 0.5_f64.next_down();
        assert_eq!(round_to_i32(just_below_half), Some(0));
        assert_eq!(round_to_i32(-just_below_half), Some(0));
        assert_eq!(round_to_i32(1.5_f64.next_down()), Some(1));
        assert_eq!(round_to_i32((-1.5_f64).next_up()), Some(-1));
    }

    #[test]
    fn quantization_rounding_is_an_upper_bound_in_lsbs() {
        // 1.5 * 1 = 1.5 → 2. Reconstructed 2. Residual 0.5 LSB; ceil is 1.
        let table = corners(1.0).quantize().unwrap();
        assert_eq!(table.values, vec![vec![2, 3], vec![4, 5]]);
        assert_eq!(table.max_err_lsb, 1);
        assert_eq!(table.per_knot_lsb[0][0], -0.5);
    }

    #[test]
    fn off_knot_samples_do_not_fill_nodes_and_do_enter_the_bound() {
        let input = BakeInput::parse(
            "0 0 0\n10 0 0\n0 10 0\n10 10 0\n5 5 1\n",
            Axis::knots(vec![0, 10]),
            Axis::knots(vec![0, 10]),
            1.0,
        )
        .unwrap();
        let table = input.quantize().unwrap();
        assert_eq!(table.values, vec![vec![0, 0], vec![0, 0]]);
        assert_eq!(table.max_err_lsb, 1);
        assert_eq!(table.worst_x, 5.0);
        assert_eq!(table.worst_y, 5.0);
        let rms = (1.0_f64 / 5.0).sqrt();
        assert!((table.rms_lsb - rms).abs() < 1e-12);
        assert_eq!(table.reconstruct(5.0, 5.0), 0.0);
    }

    #[test]
    fn duplicate_on_knot_samples_with_the_same_value_are_accepted() {
        let input = BakeInput::parse(
            "0 0 1\n10 0 2\n0 5 3\n10 5 4\n0 0 1\n",
            Axis::knots(vec![0, 10]),
            Axis::knots(vec![0, 5]),
            1.0,
        )
        .unwrap();
        assert_eq!(input.quantize().unwrap().values[0][0], 1);
    }

    #[test]
    fn a_tiny_negative_reconstruct_bumps_an_integer_residual() {
        let table = BakeInput::new(
            vec![
                Sample::new(0.0, 0.0, 0.0),
                Sample::new(1.0, 0.0, -1.0),
                Sample::new(0.0, 1.0, 0.0),
                Sample::new(1.0, 1.0, -1.0),
                Sample::new(1e-300, 0.5, 1.0),
            ],
            Axis::knots(vec![0, 1]),
            Axis::knots(vec![0, 1]),
            1.0,
        )
        .unwrap()
        .quantize()
        .unwrap();
        assert_eq!(table.values, vec![vec![0, -1], vec![0, -1]]);
        assert_eq!(table.max_err_lsb, 2);
    }

    #[test]
    fn an_unaligned_lerp_does_not_collapse_the_reconstruct() {
        let table = BakeInput::new(
            vec![
                Sample::new(0.0, 0.0, 1.0),
                Sample::new(1.0, 0.0, 2.0),
                Sample::new(0.0, 1.0, 1.0),
                Sample::new(1.0, 1.0, 2.0),
                Sample::new(1e-300, 0.5, 2.0),
            ],
            Axis::knots(vec![0, 1]),
            Axis::knots(vec![0, 1]),
            1.0,
        )
        .unwrap()
        .quantize()
        .unwrap();
        assert_eq!(table.values, vec![vec![1, 2], vec![1, 2]]);
        assert_eq!(table.max_err_lsb, 1);
    }

    #[test]
    fn a_huge_same_sign_tiny_residual_is_bound_overflow() {
        let err = BakeInput::new(
            vec![
                Sample::new(0.0, 0.0, 0.0),
                Sample::new(1.0, 0.0, -1.0),
                Sample::new(0.0, 1.0, 0.0),
                Sample::new(1.0, 1.0, -1.0),
                Sample::new(1e-300, 0.5, 3_000_000_000.0),
            ],
            Axis::knots(vec![0, 1]),
            Axis::knots(vec![0, 1]),
            1.0,
        )
        .unwrap()
        .quantize();
        assert_eq!(err, Err(BakeError::BoundOverflow));
    }

    #[test]
    fn missing_node_is_a_closed_error() {
        let (x, y) = (Axis::knots(vec![0, 10]), Axis::knots(vec![0, 5]));
        assert_eq!(
            BakeInput::parse("0 0 1\n10 0 2\n0 5 3\n", x, y, 1.0)
                .unwrap()
                .quantize(),
            Err(BakeError::MissingNode { x: 10, y: 5 })
        );
    }

    #[test]
    fn ambiguous_node_is_a_closed_error() {
        let (x, y) = (Axis::knots(vec![0, 10]), Axis::knots(vec![0, 5]));
        assert_eq!(
            BakeInput::parse("0 0 1\n10 0 2\n0 5 3\n10 5 4\n0 0 9\n", x, y, 1.0)
                .unwrap()
                .quantize(),
            Err(BakeError::AmbiguousNode { x: 0, y: 0 })
        );
    }

    #[test]
    fn overflow_outside_i32_is_a_closed_error() {
        let (x, y) = (Axis::knots(vec![0, 10]), Axis::knots(vec![0, 5]));
        assert_eq!(
            BakeInput::new(
                vec![
                    Sample::new(0.0, 0.0, 1e20),
                    Sample::new(10.0, 0.0, 0.0),
                    Sample::new(0.0, 5.0, 0.0),
                    Sample::new(10.0, 5.0, 0.0),
                ],
                x,
                y,
                1e10,
            )
            .unwrap()
            .quantize(),
            Err(BakeError::QuantizeOverflow { x: 0, y: 0 })
        );
    }

    #[test]
    fn zero_scale_is_rejected_because_the_reciprocal_is_not_finite() {
        assert_eq!(corners(0.0).quantize(), Err(BakeError::NonInvertibleScale));
    }

    #[test]
    fn uniform_axes_expand_before_fill() {
        let table = BakeInput::parse(
            "0 0 1\n10 0 2\n20 0 3\n0 5 4\n10 5 5\n20 5 6\n",
            Axis::uniform(0, 10, 3),
            Axis::uniform(0, 5, 2),
            1.0,
        )
        .unwrap()
        .quantize()
        .unwrap();
        assert_eq!(table.x, vec![0, 10, 20]);
        assert_eq!(table.y, vec![0, 5]);
        assert_eq!(table.values, vec![vec![1, 2, 3], vec![4, 5, 6]]);
    }

    #[test]
    fn emit_max_err_lsb_is_a_const_fragment_not_a_report() {
        let src = emit_max_err_lsb(3);
        assert_eq!(src, "pub const MAX_ERR_LSB: i32 = 3;\n");
        assert!(src.contains("i32"));
        assert!(!src.contains("accuracy"));
        assert!(!src.contains("device"));
    }

    #[test]
    fn runtime_oracle_confirms_the_bound_at_knots_and_host_bilinear_off_knot() {
        let input = BakeInput::parse(
            "0 0 0\n10 0 10\n0 10 0\n10 10 10\n5 5 100\n",
            Axis::knots(vec![0, 10]),
            Axis::knots(vec![0, 10]),
            1.0,
        )
        .unwrap();
        let table = input.quantize().unwrap();
        assert_eq!(table.values, vec![vec![0, 10], vec![0, 10]]);
        assert_eq!(table.max_err_lsb, 95);

        static X: [u16; 2] = [0, 10];
        static Y: [u16; 2] = [0, 10];
        static VALUES: [[i32; 2]; 2] = [[0, 10], [0, 10]];
        static SURFACE: BilinearSurface<2, 2> = BilinearSurface::new(&X, &Y, &VALUES);

        for sample in input.samples() {
            let host_lsb = (sample.value - table.reconstruct(sample.x, sample.y)) * table.scale;
            assert!(host_lsb.abs() <= f64::from(table.max_err_lsb));
            if let (Some(px), Some(py)) = (super::exact_u16(sample.x), super::exact_u16(sample.y)) {
                let runtime = SURFACE.evaluate(px, py).unwrap();
                assert_eq!(runtime, table.evaluate_u16(px, py));
                let runtime_lsb = (sample.value - f64::from(runtime) / table.scale) * table.scale;
                assert!(runtime_lsb.abs() <= f64::from(table.max_err_lsb));
            }
        }
        assert_eq!(table.reconstruct(5.0, 5.0), 5.0);
        assert_eq!(SURFACE.evaluate(5, 5), Ok(5));
    }

    #[test]
    fn max_err_lsb_covers_runtime_rounding_on_u16_off_knot_samples() {
        // Host f64 bilinear at (1, 1) is 1; runtime X-then-Y rounds to 2.
        let table = BakeInput::parse(
            "0 0 0\n2 0 1\n0 2 1\n2 2 2\n1 1 1\n",
            Axis::knots(vec![0, 2]),
            Axis::knots(vec![0, 2]),
            1.0,
        )
        .unwrap()
        .quantize()
        .unwrap();
        assert_eq!(table.values, vec![vec![0, 1], vec![1, 2]]);
        assert_eq!(table.reconstruct(1.0, 1.0), 1.0);
        assert_eq!(table.evaluate_u16(1, 1), 2);
        assert_eq!(table.max_err_lsb, 1);
        static AXIS: [u16; 2] = [0, 2];
        static VALUES: [[i32; 2]; 2] = [[0, 1], [1, 2]];
        static SURFACE: BilinearSurface<2, 2> = BilinearSurface::new(&AXIS, &AXIS, &VALUES);
        assert_eq!(SURFACE.evaluate(1, 1), Ok(2));
        let runtime_lsb = 1.0 - f64::from(SURFACE.evaluate(1, 1).unwrap());
        assert!(runtime_lsb.abs() <= f64::from(table.max_err_lsb));
    }

    #[test]
    fn max_err_lsb_does_not_understate_a_residual_rounded_onto_an_integer() {
        let value = 1.0 + f64::EPSILON;
        let scale = 1.0_f64.next_down();
        let table = BakeInput::new(
            vec![
                Sample::new(0.0, 0.0, 0.0),
                Sample::new(2.0, 0.0, 0.0),
                Sample::new(0.0, 2.0, 0.0),
                Sample::new(2.0, 2.0, 0.0),
                Sample::new(1.0, 1.0, value),
            ],
            Axis::knots(vec![0, 2]),
            Axis::knots(vec![0, 2]),
            scale,
        )
        .unwrap()
        .quantize()
        .unwrap();
        let bilinear = table.reconstruct(1.0, 1.0) * table.scale;
        let computed = value.mul_add(scale, -bilinear);
        // Host `mul_add` rounds onto 1.0; the exact product is slightly above 1.
        assert_eq!(computed.abs(), 1.0);
        assert_eq!(table.max_err_lsb, 2);
    }

    #[test]
    fn ceil_of_a_residual_just_below_an_integer_does_not_jump_an_extra_lsb() {
        let value = 2.0_f64.next_down();
        let table = BakeInput::new(
            vec![
                Sample::new(0.0, 0.0, 0.0),
                Sample::new(2.0, 0.0, 0.0),
                Sample::new(0.0, 2.0, 0.0),
                Sample::new(2.0, 2.0, 0.0),
                Sample::new(1.0, 1.0, value),
            ],
            Axis::knots(vec![0, 2]),
            Axis::knots(vec![0, 2]),
            1.0,
        )
        .unwrap()
        .quantize()
        .unwrap();
        assert!(value < 2.0);
        assert_eq!(table.max_err_lsb, 2);
    }

    #[test]
    fn max_err_lsb_uses_exact_bilinear_not_host_lerp() {
        // Host lerp at (1,1) is slightly above 286/9, so the f64 residual
        // is just under 1 and a one-ULP ceil still emits 1. Exact 286/9
        // against 32.77777777777778 is slightly above 1.
        let table = BakeInput::new(
            vec![
                Sample::new(0.0, 0.0, 4.0),
                Sample::new(3.0, 0.0, 13.0),
                Sample::new(0.0, 3.0, 75.0),
                Sample::new(3.0, 3.0, 94.0),
                Sample::new(1.0, 1.0, 32.77777777777778),
            ],
            Axis::knots(vec![0, 3]),
            Axis::knots(vec![0, 3]),
            1.0,
        )
        .unwrap()
        .quantize()
        .unwrap();
        assert_eq!(table.values, vec![vec![4, 13], vec![75, 94]]);
        let host = (32.77777777777778 - table.reconstruct(1.0, 1.0)).abs();
        assert!(host < 1.0, "host lerp residual {host} would understate");
        assert_eq!(table.max_err_lsb, 2);
    }

    #[test]
    fn decimal_off_knot_coordinates_do_not_overflow_i128() {
        let table = BakeInput::new(
            vec![
                Sample::new(0.0, 0.0, 1.0),
                Sample::new(1.0, 0.0, 2.0),
                Sample::new(0.0, 1.0, 3.0),
                Sample::new(1.0, 1.0, 7.0),
                Sample::new(0.1, 0.1, 0.0),
            ],
            Axis::knots(vec![0, 1]),
            Axis::knots(vec![0, 1]),
            1.0,
        )
        .unwrap()
        .quantize()
        .unwrap();
        assert_eq!(table.values, vec![vec![1, 2], vec![3, 7]]);
        assert!(table.max_err_lsb >= 1);
    }

    #[test]
    fn tiny_finite_off_knot_sample_emits_a_bound() {
        let table = BakeInput::new(
            vec![
                Sample::new(0.0, 0.0, 0.0),
                Sample::new(1.0, 0.0, 0.0),
                Sample::new(0.0, 1.0, 0.0),
                Sample::new(1.0, 1.0, 0.0),
                Sample::new(0.1, 0.1, 1e-300),
            ],
            Axis::knots(vec![0, 1]),
            Axis::knots(vec![0, 1]),
            1.0,
        )
        .unwrap()
        .quantize()
        .unwrap();
        assert_eq!(table.max_err_lsb, 1);
    }

    #[test]
    fn a_tiny_sample_against_a_unit_table_still_emits_a_bound() {
        let table = BakeInput::new(
            vec![
                Sample::new(0.0, 0.0, 1.0),
                Sample::new(1.0, 0.0, 1.0),
                Sample::new(0.0, 1.0, 1.0),
                Sample::new(1.0, 1.0, 1.0),
                Sample::new(0.5, 0.5, 1e-300),
            ],
            Axis::knots(vec![0, 1]),
            Axis::knots(vec![0, 1]),
            1.0,
        )
        .unwrap()
        .quantize()
        .unwrap();
        assert_eq!(table.values, vec![vec![1, 1], vec![1, 1]]);
        assert_eq!(table.max_err_lsb, 1);
    }

    #[test]
    fn a_tiny_rms_does_not_underflow_to_zero() {
        let table = BakeInput::new(
            vec![
                Sample::new(0.0, 0.0, 0.0),
                Sample::new(1.0, 0.0, 0.0),
                Sample::new(0.0, 1.0, 0.0),
                Sample::new(1.0, 1.0, 0.0),
                Sample::new(0.5, 0.5, 1e-200),
            ],
            Axis::knots(vec![0, 1]),
            Axis::knots(vec![0, 1]),
            1.0,
        )
        .unwrap()
        .quantize()
        .unwrap();
        assert_eq!(table.max_err_lsb, 1);
        let expected = 1e-200 / 5.0_f64.sqrt();
        assert!(table.rms_lsb > 0.0);
        assert!((table.rms_lsb - expected).abs() / expected < 1e-9);
    }

    #[test]
    fn rms_of_a_power_of_two_subnormal_does_not_become_zero() {
        let sample = f64::MIN_POSITIVE / 2.0;
        let table = BakeInput::new(
            vec![
                Sample::new(0.0, 0.0, 0.0),
                Sample::new(1.0, 0.0, 0.0),
                Sample::new(0.0, 1.0, 0.0),
                Sample::new(1.0, 1.0, 0.0),
                Sample::new(0.5, 0.5, sample),
            ],
            Axis::knots(vec![0, 1]),
            Axis::knots(vec![0, 1]),
            1.0,
        )
        .unwrap()
        .quantize()
        .unwrap();
        let expected = sample / 5.0_f64.sqrt();
        assert!(table.rms_lsb > 0.0);
        assert!((table.rms_lsb - expected).abs() / expected < 1e-9);
    }

    #[test]
    fn a_finite_residual_too_wide_for_i32_is_bound_overflow() {
        let err = BakeInput::new(
            vec![
                Sample::new(0.0, 0.0, 0.0),
                Sample::new(1.0, 0.0, 0.0),
                Sample::new(0.0, 1.0, 0.0),
                Sample::new(1.0, 1.0, 0.0),
                Sample::new(0.5, 0.5, 3_000_000_000.0),
            ],
            Axis::knots(vec![0, 1]),
            Axis::knots(vec![0, 1]),
            1.0,
        )
        .unwrap()
        .quantize();
        assert_eq!(err, Err(BakeError::BoundOverflow));
    }

    #[test]
    fn x_then_y_host_bilinear_matches_the_evaluate_order_fixture() {
        // Same numbers as evaluate.rs: X-then-Y at (1, 1) is 1.
        let table = BakeInput::parse(
            "0 0 0\n2 0 0\n0 2 1\n2 2 3\n",
            Axis::knots(vec![0, 2]),
            Axis::knots(vec![0, 2]),
            1.0,
        )
        .unwrap()
        .quantize()
        .unwrap();
        assert_eq!(table.reconstruct(1.0, 1.0), 1.0);
        static AXIS: [u16; 2] = [0, 2];
        static VALUES: [[i32; 2]; 2] = [[0, 0], [1, 3]];
        static SURFACE: BilinearSurface<2, 2> = BilinearSurface::new(&AXIS, &AXIS, &VALUES);
        assert_eq!(SURFACE.evaluate(1, 1), Ok(1));
    }

    #[test]
    fn extreme_scale_does_not_nan_on_endpoint_differences() {
        let table = BakeInput::new(
            vec![
                Sample::new(0.0, 0.0, -1e308),
                Sample::new(2.0, 0.0, 1e308),
                Sample::new(0.0, 2.0, -1e308),
                Sample::new(2.0, 2.0, 1e308),
            ],
            Axis::knots(vec![0, 2]),
            Axis::knots(vec![0, 2]),
            2e-299,
        )
        .unwrap()
        .quantize()
        .unwrap();
        assert_eq!(
            table.values,
            vec![
                vec![-2_000_000_000, 2_000_000_000],
                vec![-2_000_000_000, 2_000_000_000]
            ]
        );
        assert!(table.reconstruct(0.0, 0.0).is_finite());
        assert!(table.reconstruct(1.0, 0.0).is_finite());
    }

    #[test]
    fn oversized_grid_product_is_rejected_before_allocation() {
        let input = BakeInput::new(
            Vec::new(),
            Axis::uniform(0, 1, 65_536),
            Axis::uniform(0, 1, 65_536),
            1.0,
        )
        .unwrap();
        assert_eq!(
            input.quantize(),
            Err(BakeError::GridTooLarge {
                nx: 65_536,
                ny: 65_536
            })
        );
    }

    #[test]
    fn a_long_uniform_axis_fills_by_binary_search() {
        let nx = 256usize;
        let mut text = String::new();
        for i in 0..nx {
            text.push_str(&format!("{i} 0 {i}\n{i} 1 {}\n", i + 1000));
        }
        let table = BakeInput::parse(&text, Axis::uniform(0, 1, nx), Axis::uniform(0, 1, 2), 1.0)
            .unwrap()
            .quantize()
            .unwrap();
        assert_eq!(table.x.len(), nx);
        assert_eq!(table.values[0][0], 0);
        assert_eq!(table.values[0][nx - 1], (nx - 1) as i32);
        assert_eq!(table.values[1][nx - 1], (nx - 1 + 1000) as i32);
        assert_eq!(table.max_err_lsb, 0);
    }
}

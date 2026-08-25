//! Fill the declared grid, quantize to i32, and measure sample deviation.
//!
//! This is not a fitter. Each declared node is taken from an on-knot sample;
//! off-knot samples participate only in the deviation measurement.

use crate::BakeInput;
use crate::error::{AxisName, BakeError};
use crate::samples::Sample;

/// Quantized row-major grid plus deviation of that table from the samples.
///
/// `max_err_lsb` is an i32 value LSB (not a color LSB). It is the maximum
/// absolute deviation between the supplied samples and the table built from
/// them: an upper bound, not a typical error, and not a device, vendor,
/// sensor, calibration, accuracy, timing, flash, or WCET claim.
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
    /// Durable bound: ceil of the maximum absolute sample deviation in LSBs.
    pub max_err_lsb: i32,
    /// RMS of sample deviations in i32 value LSBs. Operator statistic.
    pub rms_lsb: f64,
    /// X coordinate of the sample with the largest absolute LSB deviation.
    pub worst_x: f64,
    /// Y coordinate of the sample with the largest absolute LSB deviation.
    pub worst_y: f64,
    /// Signed LSB residual at each declared node, row-major `[y][x]`.
    pub per_knot_lsb: Vec<Vec<f64>>,
}

impl QuantizedTable {
    /// Host `f64` bilinear of the dequantized grid, X then Y.
    ///
    /// Grid nodes convert as `i32 as f64 / scale`. Each step is ordinary
    /// `f64` interpolation with no integer rounding: this reconstructs the
    /// quantized table as a host surface, and is not a second copy of
    /// runtime `evaluate`. Off-knot samples use this comparison because
    /// runtime evaluation takes `u16` coordinates.
    ///
    /// `x` and `y` must lie in the inclusive declared domain.
    #[must_use]
    pub fn reconstruct(&self, x: f64, y: f64) -> f64 {
        reconstruct(
            &self.x,
            &self.y,
            &dequantize(&self.values, self.scale),
            x,
            y,
        )
    }
}

/// Source fragment so later emission can take the bound as an argument.
///
/// Emits `pub const MAX_ERR_LSB: i32 = …;` — an i32 value LSB, the maximum
/// absolute deviation between supplied samples and the table built from them.
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
    /// the integer kernel.
    ///
    /// # Errors
    ///
    /// Returns [`BakeError::NonInvertibleScale`] when `1 / scale` is not
    /// finite, [`BakeError::MissingNode`] or [`BakeError::AmbiguousNode`]
    /// for the declared grid, [`BakeError::QuantizeOverflow`] when a
    /// rounded value leaves `i32`, or [`BakeError::NonFiniteDeviation`]
    /// when a residual is not a finite LSB quantity.
    pub fn quantize(&self) -> Result<QuantizedTable, BakeError> {
        if !(1.0 / self.scale()).is_finite() {
            return Err(BakeError::NonInvertibleScale);
        }
        let x = self.x().knot_list(AxisName::X)?;
        let y = self.y().knot_list(AxisName::Y)?;
        let filled = fill_nodes(self.samples(), &x, &y)?;
        let values = quantize_grid(&filled, &x, &y, self.scale())?;
        deviation(self.samples(), x, y, filled, values, self.scale())
    }
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

#[allow(clippy::float_cmp)] // exact f64 equality with f64::from(u16) is the node test
fn knot_index(knots: &[u16], coord: f64) -> Option<usize> {
    knots.iter().position(|&knot| coord == f64::from(knot))
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
fn round_to_i32(value: f64) -> Option<i32> {
    if !value.is_finite() {
        return None;
    }
    let rounded = if value >= 0.0 {
        (value + 0.5).floor()
    } else {
        (value - 0.5).ceil()
    };
    if rounded > f64::from(i32::MAX) || rounded < f64::from(i32::MIN) {
        return None;
    }
    Some(rounded as i32)
}

fn dequantize(values: &[Vec<i32>], scale: f64) -> Vec<Vec<f64>> {
    values
        .iter()
        .map(|row| row.iter().map(|&v| f64::from(v) / scale).collect())
        .collect()
}

fn deviation(
    samples: &[Sample],
    x: Vec<u16>,
    y: Vec<u16>,
    filled: Vec<Vec<f64>>,
    values: Vec<Vec<i32>>,
    scale: f64,
) -> Result<QuantizedTable, BakeError> {
    let host = dequantize(&values, scale);
    let mut per_knot_lsb = vec![vec![0.0; x.len()]; y.len()];
    for yi in 0..y.len() {
        for xi in 0..x.len() {
            per_knot_lsb[yi][xi] = (filled[yi][xi] - host[yi][xi]) * scale;
            if !per_knot_lsb[yi][xi].is_finite() {
                return Err(BakeError::NonFiniteDeviation);
            }
        }
    }
    let mut sum_sq = 0.0;
    let mut max_abs = 0.0;
    let mut worst_x = samples[0].x;
    let mut worst_y = samples[0].y;
    for sample in samples {
        let reconstructed = reconstruct(&x, &y, &host, sample.x, sample.y);
        let lsb = (sample.value - reconstructed) * scale;
        if !lsb.is_finite() {
            return Err(BakeError::NonFiniteDeviation);
        }
        let abs = lsb.abs();
        if abs > max_abs {
            max_abs = abs;
            worst_x = sample.x;
            worst_y = sample.y;
        }
        sum_sq += lsb * lsb;
    }
    Ok(QuantizedTable {
        x,
        y,
        values,
        scale,
        max_err_lsb: ceil_abs_lsb(max_abs)?,
        rms_lsb: (sum_sq / samples.len() as f64).sqrt(),
        worst_x,
        worst_y,
        per_knot_lsb,
    })
}

fn ceil_abs_lsb(max_abs: f64) -> Result<i32, BakeError> {
    if !max_abs.is_finite() {
        return Err(BakeError::NonFiniteDeviation);
    }
    let ceil = max_abs.ceil();
    if ceil > f64::from(i32::MAX) {
        return Err(BakeError::NonFiniteDeviation);
    }
    Ok(ceil as i32)
}

fn reconstruct(x_knots: &[u16], y_knots: &[u16], host: &[Vec<f64>], x: f64, y: f64) -> f64 {
    let xi = segment(x_knots, x);
    let yi = segment(y_knots, y);
    let x0 = f64::from(x_knots[xi]);
    let x1 = f64::from(x_knots[xi + 1]);
    let y0 = f64::from(y_knots[yi]);
    let y1 = f64::from(y_knots[yi + 1]);
    let lower = lerp(x, x0, x1, host[yi][xi], host[yi][xi + 1]);
    let upper = lerp(x, x0, x1, host[yi + 1][xi], host[yi + 1][xi + 1]);
    lerp(y, y0, y1, lower, upper)
}

fn segment(knots: &[u16], coord: f64) -> usize {
    let last = knots.len() - 1;
    let mut i = 0;
    for (idx, &knot) in knots.iter().enumerate() {
        if f64::from(knot) <= coord {
            i = idx;
        }
    }
    if i == last { last - 1 } else { i }
}

fn lerp(t: f64, t0: f64, t1: f64, v0: f64, v1: f64) -> f64 {
    v0 + (v1 - v0) * ((t - t0) / (t1 - t0))
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
            if let (Some(xi), Some(yi)) = (
                super::knot_index(&table.x, sample.x),
                super::knot_index(&table.y, sample.y),
            ) {
                let runtime = SURFACE.evaluate(table.x[xi], table.y[yi]).unwrap();
                let runtime_lsb = (sample.value - f64::from(runtime) / table.scale) * table.scale;
                assert!(runtime_lsb.abs() <= f64::from(table.max_err_lsb));
                assert_eq!(runtime, table.values[yi][xi]);
            }
        }
        // Off-knot (5, 5) is a u16 but not a declared knot: host bilinear, not evaluate.
        assert_eq!(table.reconstruct(5.0, 5.0), 5.0);
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
}

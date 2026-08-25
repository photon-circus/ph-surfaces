//! Closed host bake failures.

use core::fmt::{Display, Formatter, Result as FmtResult};

/// Which axis a grid descriptor refers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AxisName {
    /// The X axis.
    X,
    /// The Y axis.
    Y,
}

/// Which `f64` field of a [`Sample`](crate::Sample) failed a finite check.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SampleField {
    /// [`Sample::x`](crate::Sample::x).
    X,
    /// [`Sample::y`](crate::Sample::y).
    Y,
    /// [`Sample::value`](crate::Sample::value).
    Value,
}

/// Host bake failure.
///
/// Closed: every ingest and quantize rejection class is a variant. This enum
/// is not `#[non_exhaustive]`; exhaustive matching without a wildcard arm is
/// intended. Display strings for constructor failures copy the runtime
/// panic messages exactly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BakeError {
    /// The X axis declared fewer than two knots.
    XAxisTooShort,
    /// The Y axis declared fewer than two knots.
    YAxisTooShort,
    /// The X knots were not strictly increasing.
    XAxisNotStrictlyIncreasing,
    /// The Y knots were not strictly increasing.
    YAxisNotStrictlyIncreasing,
    /// A uniform axis declared fewer than two knots.
    UniformTooFewKnots {
        /// Which axis failed.
        axis: AxisName,
    },
    /// A uniform axis declared more than 65_536 knots.
    UniformTooManyKnots {
        /// Which axis failed.
        axis: AxisName,
    },
    /// A uniform axis declared a step of zero.
    UniformStepTooSmall {
        /// Which axis failed.
        axis: AxisName,
    },
    /// A uniform axis last knot exceeded `u16::MAX`.
    UniformLastKnotUnrepresentable {
        /// Which axis failed.
        axis: AxisName,
    },
    /// A sample X coordinate was below the first X knot.
    SampleXBelow {
        /// The rejected X coordinate.
        coordinate: f64,
        /// The first X knot: the inclusive lower bound of the X domain.
        bound: u16,
    },
    /// A sample X coordinate was above the last X knot.
    SampleXAbove {
        /// The rejected X coordinate.
        coordinate: f64,
        /// The last X knot: the inclusive upper bound of the X domain.
        bound: u16,
    },
    /// A sample Y coordinate was below the first Y knot.
    SampleYBelow {
        /// The rejected Y coordinate.
        coordinate: f64,
        /// The first Y knot: the inclusive lower bound of the Y domain.
        bound: u16,
    },
    /// A sample Y coordinate was above the last Y knot.
    SampleYAbove {
        /// The rejected Y coordinate.
        coordinate: f64,
        /// The last Y knot: the inclusive upper bound of the Y domain.
        bound: u16,
    },
    /// A sample line was not three finite numbers.
    MalformedLine {
        /// 1-based physical line number in the sample text.
        line: usize,
    },
    /// A sample coordinate or value was NaN or infinite.
    NonFiniteSample {
        /// Which sample field failed.
        field: SampleField,
    },
    /// The caller-stated output scale was NaN or infinite.
    NonFiniteScale,
    /// The output scale has no finite reciprocal, so dequantization would blow up.
    NonInvertibleScale,
    /// No sample supplied an exact on-knot value for this declared node.
    MissingNode {
        /// Declared X knot of the empty node.
        x: u16,
        /// Declared Y knot of the empty node.
        y: u16,
    },
    /// Two samples claimed the same declared node with different values.
    AmbiguousNode {
        /// Declared X knot of the conflicting node.
        x: u16,
        /// Declared Y knot of the conflicting node.
        y: u16,
    },
    /// `round(value * scale)` at this node was outside `i32`.
    QuantizeOverflow {
        /// Declared X knot of the overflowing node.
        x: u16,
        /// Declared Y knot of the overflowing node.
        y: u16,
    },
    /// A sample deviation was not a finite i32-value-LSB quantity.
    NonFiniteDeviation,
    /// A finite residual's `ceil` does not fit in `i32`.
    BoundOverflow,
    /// The declared NX × NY product exceeds [`crate::MAX_GRID_CELLS`].
    GridTooLarge {
        /// Declared X knot count.
        nx: usize,
        /// Declared Y knot count.
        ny: usize,
    },
    /// A bucketed emit axis declared `B` outside the runtime `1..=65_536` range.
    InvalidBucketCount {
        /// Which axis the illegal bucket count was requested for.
        axis: AxisName,
        /// The rejected bucket count.
        buckets: usize,
    },
}

impl Display for BakeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match *self {
            Self::XAxisTooShort => f.write_str("x axis must declare at least two knots"),
            Self::YAxisTooShort => f.write_str("y axis must declare at least two knots"),
            Self::XAxisNotStrictlyIncreasing => {
                f.write_str("x axis knots must be strictly increasing")
            }
            Self::YAxisNotStrictlyIncreasing => {
                f.write_str("y axis knots must be strictly increasing")
            }
            Self::UniformTooFewKnots { .. } => {
                f.write_str("an axis must declare at least two knots")
            }
            Self::UniformTooManyKnots { .. } => {
                f.write_str("a uniform u16 axis declares at most 65_536 knots")
            }
            Self::UniformStepTooSmall { .. } => {
                f.write_str("a uniform axis must declare a step of at least 1")
            }
            Self::UniformLastKnotUnrepresentable { .. } => {
                f.write_str("the last uniform knot must be representable in u16")
            }
            Self::SampleXBelow { coordinate, bound } => {
                write!(
                    f,
                    "x coordinate {coordinate} is below the x axis minimum {bound}"
                )
            }
            Self::SampleXAbove { coordinate, bound } => {
                write!(
                    f,
                    "x coordinate {coordinate} is above the x axis maximum {bound}"
                )
            }
            Self::SampleYBelow { coordinate, bound } => {
                write!(
                    f,
                    "y coordinate {coordinate} is below the y axis minimum {bound}"
                )
            }
            Self::SampleYAbove { coordinate, bound } => {
                write!(
                    f,
                    "y coordinate {coordinate} is above the y axis maximum {bound}"
                )
            }
            Self::MalformedLine { line } => {
                write!(f, "malformed sample line {line}: expected three numbers")
            }
            Self::NonFiniteSample { field } => match field {
                SampleField::X => f.write_str("sample x is not a finite f64"),
                SampleField::Y => f.write_str("sample y is not a finite f64"),
                SampleField::Value => f.write_str("sample value is not a finite f64"),
            },
            Self::NonFiniteScale => f.write_str("output scale is not a finite f64"),
            Self::NonInvertibleScale => f.write_str("output scale must have a finite reciprocal"),
            Self::MissingNode { x, y } => {
                write!(f, "grid node ({x}, {y}) has no sample")
            }
            Self::AmbiguousNode { x, y } => {
                write!(f, "grid node ({x}, {y}) has conflicting sample values")
            }
            Self::QuantizeOverflow { x, y } => {
                write!(f, "quantized value at ({x}, {y}) does not fit in i32")
            }
            Self::NonFiniteDeviation => {
                f.write_str("sample deviation is not a finite i32 value LSB")
            }
            Self::BoundOverflow => {
                f.write_str("finite sample residual does not fit in i32 MAX_ERR_LSB")
            }
            Self::GridTooLarge { nx, ny } => {
                write!(
                    f,
                    "declared grid is {nx} by {ny} cells; the baker accepts at most {}",
                    crate::MAX_GRID_CELLS
                )
            }
            Self::InvalidBucketCount { buckets: 0, .. } => {
                f.write_str("a bucket index must declare at least one bucket")
            }
            Self::InvalidBucketCount { .. } => {
                f.write_str("a bucket index declares at most 65_536 buckets")
            }
        }
    }
}

impl std::error::Error for BakeError {}

#[cfg(test)]
mod tests {
    use super::{AxisName, BakeError, SampleField};

    #[test]
    fn constructor_messages_match_the_runtime_vocabulary() {
        assert_eq!(
            BakeError::XAxisTooShort.to_string(),
            "x axis must declare at least two knots"
        );
        assert_eq!(
            BakeError::YAxisTooShort.to_string(),
            "y axis must declare at least two knots"
        );
        assert_eq!(
            BakeError::XAxisNotStrictlyIncreasing.to_string(),
            "x axis knots must be strictly increasing"
        );
        assert_eq!(
            BakeError::YAxisNotStrictlyIncreasing.to_string(),
            "y axis knots must be strictly increasing"
        );
        assert_eq!(
            BakeError::UniformTooFewKnots { axis: AxisName::X }.to_string(),
            "an axis must declare at least two knots"
        );
        assert_eq!(
            BakeError::UniformTooManyKnots { axis: AxisName::Y }.to_string(),
            "a uniform u16 axis declares at most 65_536 knots"
        );
        assert_eq!(
            BakeError::UniformStepTooSmall { axis: AxisName::X }.to_string(),
            "a uniform axis must declare a step of at least 1"
        );
        assert_eq!(
            BakeError::UniformLastKnotUnrepresentable { axis: AxisName::Y }.to_string(),
            "the last uniform knot must be representable in u16"
        );
    }

    #[test]
    fn sample_messages_name_the_axis_the_coordinate_and_the_bound() {
        assert_eq!(
            BakeError::SampleXBelow {
                coordinate: 3.0,
                bound: 10,
            }
            .to_string(),
            "x coordinate 3 is below the x axis minimum 10"
        );
        assert_eq!(
            BakeError::SampleXAbove {
                coordinate: 900.0,
                bound: 500,
            }
            .to_string(),
            "x coordinate 900 is above the x axis maximum 500"
        );
        assert_eq!(
            BakeError::SampleYBelow {
                coordinate: 3.0,
                bound: 10,
            }
            .to_string(),
            "y coordinate 3 is below the y axis minimum 10"
        );
        assert_eq!(
            BakeError::SampleYAbove {
                coordinate: 900.0,
                bound: 500,
            }
            .to_string(),
            "y coordinate 900 is above the y axis maximum 500"
        );
    }

    #[test]
    fn malformed_line_names_the_physical_line() {
        assert_eq!(
            BakeError::MalformedLine { line: 4 }.to_string(),
            "malformed sample line 4: expected three numbers"
        );
    }

    #[test]
    fn non_finite_variants_name_the_field() {
        assert_eq!(
            BakeError::NonFiniteSample {
                field: SampleField::X
            }
            .to_string(),
            "sample x is not a finite f64"
        );
        assert_eq!(
            BakeError::NonFiniteSample {
                field: SampleField::Y
            }
            .to_string(),
            "sample y is not a finite f64"
        );
        assert_eq!(
            BakeError::NonFiniteSample {
                field: SampleField::Value
            }
            .to_string(),
            "sample value is not a finite f64"
        );
        assert_eq!(
            BakeError::NonFiniteScale.to_string(),
            "output scale is not a finite f64"
        );
        assert_eq!(
            BakeError::NonInvertibleScale.to_string(),
            "output scale must have a finite reciprocal"
        );
        assert_eq!(
            BakeError::MissingNode { x: 10, y: 5 }.to_string(),
            "grid node (10, 5) has no sample"
        );
        assert_eq!(
            BakeError::AmbiguousNode { x: 0, y: 1 }.to_string(),
            "grid node (0, 1) has conflicting sample values"
        );
        assert_eq!(
            BakeError::QuantizeOverflow { x: 2, y: 3 }.to_string(),
            "quantized value at (2, 3) does not fit in i32"
        );
        assert_eq!(
            BakeError::NonFiniteDeviation.to_string(),
            "sample deviation is not a finite i32 value LSB"
        );
        assert_eq!(
            BakeError::BoundOverflow.to_string(),
            "finite sample residual does not fit in i32 MAX_ERR_LSB"
        );
        assert_eq!(
            BakeError::GridTooLarge {
                nx: 65_536,
                ny: 65_536
            }
            .to_string(),
            "declared grid is 65536 by 65536 cells; the baker accepts at most 1048576"
        );
        assert_eq!(
            BakeError::InvalidBucketCount {
                axis: AxisName::X,
                buckets: 0
            }
            .to_string(),
            "a bucket index must declare at least one bucket"
        );
        assert_eq!(
            BakeError::InvalidBucketCount {
                axis: AxisName::Y,
                buckets: 65_537
            }
            .to_string(),
            "a bucket index declares at most 65_536 buckets"
        );
    }

    #[test]
    fn the_error_has_no_source() {
        assert!(std::error::Error::source(&BakeError::XAxisTooShort).is_none());
    }

    #[test]
    fn uniform_variants_name_the_failing_axis() {
        let x = BakeError::UniformStepTooSmall { axis: AxisName::X };
        let y = BakeError::UniformStepTooSmall { axis: AxisName::Y };
        assert_ne!(x, y);
        assert_eq!(x, BakeError::UniformStepTooSmall { axis: AxisName::X });
    }
}

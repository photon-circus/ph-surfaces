//! Host-only baker for `ph-surfaces` tables.
//!
//! This crate requires `std` and `f64`. It must **never** be linked into
//! target firmware. The runtime crate does not depend on this package.
//!
//! # Ingest
//!
//! Sample points are host `f64` triples (X, Y, value) from delimited text.
//! The grid is explicit: a knot list per axis, or a uniform origin/step/count
//! matching the runtime `UniformAxis`. A caller-stated output scale is stored
//! and not applied. The baker does not choose knots, parse expressions, or
//! quantize values.
//!
//! ```
//! use ph_surfaces_bake::{Axis, BakeInput, Sample};
//!
//! let samples = vec![
//!     Sample::new(0.0, 0.0, 1.5),
//!     Sample::new(10.0, 5.0, 2.5),
//! ];
//! let input = BakeInput::new(
//!     samples,
//!     Axis::knots(vec![0, 10]),
//!     Axis::knots(vec![0, 5]),
//!     1000.0,
//! )
//! .unwrap();
//! assert_eq!(input.scale(), 1000.0);
//! assert_eq!(input.samples()[0].value, 1.5);
//! ```
//!
//! Quantization and the emitted deviation bound: issue #40.
//! Rust emission and the checked-in generated-source drift gate: issue #41.
//! Frozen golden vectors: issue #42.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod error;
mod grid;
mod samples;

pub use error::{AxisName, BakeError};
pub use grid::Axis;
pub use samples::{Sample, parse_samples};

/// Validated ingest: samples, an explicit grid, and a stored output scale.
#[derive(Clone, Debug, PartialEq)]
pub struct BakeInput {
    samples: Vec<Sample>,
    x: Axis,
    y: Axis,
    scale: f64,
}

impl BakeInput {
    /// Validates `x` and `y` as the runtime constructors would, then rejects
    /// any sample whose X or Y falls outside that inclusive domain.
    ///
    /// `scale` is retained and not applied to [`Sample::value`].
    ///
    /// # Errors
    ///
    /// Returns a [`BakeError`] for an axis the runtime would reject, or for a
    /// sample outside the declared grid.
    pub fn new(samples: Vec<Sample>, x: Axis, y: Axis, scale: f64) -> Result<Self, BakeError> {
        let (x_first, x_last) = x.bounds(AxisName::X)?;
        let (y_first, y_last) = y.bounds(AxisName::Y)?;
        for sample in &samples {
            in_domain(
                sample.x,
                x_first,
                x_last,
                BakeError::SampleXBelow {
                    coordinate: sample.x,
                    bound: x_first,
                },
                BakeError::SampleXAbove {
                    coordinate: sample.x,
                    bound: x_last,
                },
            )?;
            in_domain(
                sample.y,
                y_first,
                y_last,
                BakeError::SampleYBelow {
                    coordinate: sample.y,
                    bound: y_first,
                },
                BakeError::SampleYAbove {
                    coordinate: sample.y,
                    bound: y_last,
                },
            )?;
        }
        Ok(Self {
            samples,
            x,
            y,
            scale,
        })
    }

    /// Parses sample text, then validates it against `x`, `y`, and `scale`.
    ///
    /// # Errors
    ///
    /// Returns [`BakeError::MalformedLine`] for a bad sample line, or any
    /// [`BakeInput::new`] rejection.
    pub fn parse(text: &str, x: Axis, y: Axis, scale: f64) -> Result<Self, BakeError> {
        Self::new(parse_samples(text)?, x, y, scale)
    }

    /// Sample points in file order.
    #[must_use]
    pub fn samples(&self) -> &[Sample] {
        &self.samples
    }

    /// Declared X axis after validation.
    #[must_use]
    pub fn x(&self) -> &Axis {
        &self.x
    }

    /// Declared Y axis after validation.
    #[must_use]
    pub fn y(&self) -> &Axis {
        &self.y
    }

    /// Caller-stated output scale. Stored; not applied.
    #[must_use]
    pub fn scale(&self) -> f64 {
        self.scale
    }
}

fn in_domain(
    coordinate: f64,
    first: u16,
    last: u16,
    below: BakeError,
    above: BakeError,
) -> Result<(), BakeError> {
    let lo = f64::from(first);
    let hi = f64::from(last);
    if coordinate >= lo && coordinate <= hi {
        Ok(())
    } else if coordinate < lo || coordinate.is_nan() {
        Err(below)
    } else {
        Err(above)
    }
}

#[cfg(test)]
mod tests {
    use super::{Axis, BakeError, BakeInput, Sample};
    use ph_surfaces::BilinearSurface;

    fn explicit() -> (Axis, Axis) {
        (Axis::knots(vec![0, 10, 20]), Axis::knots(vec![0, 5]))
    }

    #[test]
    fn runtime_is_available_as_a_dev_dependency_oracle() {
        static AXIS: [u16; 2] = [0, 2];
        static VALUES: [[i32; 2]; 2] = [[0, 0], [1, 3]];
        static SURFACE: BilinearSurface<2, 2> = BilinearSurface::new(&AXIS, &AXIS, &VALUES);
        assert_eq!(SURFACE.evaluate(0, 0), Ok(0));
    }

    #[test]
    fn explicit_knots_accept_in_domain_samples_and_store_scale() {
        let (x, y) = explicit();
        let input = BakeInput::parse("0 0 1.5\n20 5 9\n10 2.5 4\n", x, y, 1000.0).unwrap();
        assert_eq!(input.scale(), 1000.0);
        assert_eq!(input.samples()[0].value, 1.5);
        assert_eq!(input.samples()[1].value, 9.0);
        assert_eq!(input.x(), &Axis::Knots(vec![0, 10, 20]));
        assert_eq!(input.y(), &Axis::Knots(vec![0, 5]));
    }

    #[test]
    fn uniform_axes_accept_in_domain_samples() {
        let input = BakeInput::parse(
            "0 0 1\n20 10 2\n",
            Axis::uniform(0, 10, 3),
            Axis::uniform(0, 5, 3),
            1.0,
        )
        .unwrap();
        assert_eq!(
            input.x(),
            &Axis::Uniform {
                origin: 0,
                step: 10,
                count: 3
            }
        );
        assert_eq!(input.samples().len(), 2);
        assert_eq!(input.scale(), 1.0);
    }

    #[test]
    fn mixed_explicit_and_uniform_axes_are_accepted() {
        let input = BakeInput::new(
            vec![Sample::new(10.0, 0.0, 3.0)],
            Axis::knots(vec![0, 10]),
            Axis::uniform(0, 5, 2),
            2.0,
        )
        .unwrap();
        assert!(matches!(input.x(), Axis::Knots(_)));
        assert!(matches!(input.y(), Axis::Uniform { .. }));
        assert_eq!(input.scale(), 2.0);
    }

    #[test]
    fn sample_x_below_the_first_knot_is_reported() {
        let (x, y) = explicit();
        assert_eq!(
            BakeInput::new(vec![Sample::new(-0.1, 0.0, 1.0)], x, y, 1.0),
            Err(BakeError::SampleXBelow {
                coordinate: -0.1,
                bound: 0
            })
        );
    }

    #[test]
    fn sample_x_above_the_last_knot_is_reported() {
        let (x, y) = explicit();
        assert_eq!(
            BakeInput::new(vec![Sample::new(20.1, 0.0, 1.0)], x, y, 1.0),
            Err(BakeError::SampleXAbove {
                coordinate: 20.1,
                bound: 20
            })
        );
    }

    #[test]
    fn sample_y_below_the_first_knot_is_reported() {
        let (x, y) = explicit();
        assert_eq!(
            BakeInput::new(vec![Sample::new(0.0, -1.0, 1.0)], x, y, 1.0),
            Err(BakeError::SampleYBelow {
                coordinate: -1.0,
                bound: 0
            })
        );
    }

    #[test]
    fn sample_y_above_the_last_knot_is_reported() {
        let (x, y) = explicit();
        assert_eq!(
            BakeInput::new(vec![Sample::new(0.0, 5.1, 1.0)], x, y, 1.0),
            Err(BakeError::SampleYAbove {
                coordinate: 5.1,
                bound: 5
            })
        );
    }

    #[test]
    fn inclusive_endpoints_are_in_domain() {
        let (x, y) = explicit();
        BakeInput::new(
            vec![Sample::new(0.0, 0.0, 0.0), Sample::new(20.0, 5.0, 1.0)],
            x,
            y,
            1.0,
        )
        .unwrap();
    }

    #[test]
    fn x_out_of_domain_wins_when_both_coordinates_are_outside() {
        let (x, y) = explicit();
        assert_eq!(
            BakeInput::new(vec![Sample::new(-1.0, 9.0, 1.0)], x, y, 1.0),
            Err(BakeError::SampleXBelow {
                coordinate: -1.0,
                bound: 0
            })
        );
    }

    #[test]
    fn scale_is_not_applied_to_sample_values() {
        let (x, y) = explicit();
        let input = BakeInput::parse("0 0 1.5\n", x, y, 1000.0).unwrap();
        assert_eq!(input.scale(), 1000.0);
        assert_eq!(input.samples()[0].value, 1.5);
    }
}

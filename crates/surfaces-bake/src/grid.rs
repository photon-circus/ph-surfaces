//! Explicit per-axis grid specifications.

use crate::error::{AxisName, BakeError};

/// Maximum knot count the runtime `UniformAxis::new` accepts.
///
/// Host-side copy of that bound. The runtime crate is a dev-dependency and
/// is not named here as a rustdoc link.
const MAX_UNIFORM_KNOTS: usize = 65_536;

/// One declared axis of a bake grid.
///
/// This is a host-only specification, not a firmware lookup strategy. The
/// caller chooses a knot list or a uniform origin/step/count; the baker does
/// not pick, adapt, or optimize knots.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Axis {
    /// Explicit knots that will become a stored runtime axis.
    Knots(Vec<u16>),
    /// Uniform origin, step, and count matching `UniformAxis`.
    Uniform {
        /// First knot.
        origin: u16,
        /// Spacing between consecutive knots; must be at least 1.
        step: u16,
        /// Number of knots; must be in `2..=65_536`.
        count: usize,
    },
}

impl Axis {
    /// Declares an explicit knot list. Validation happens in [`crate::BakeInput::new`].
    #[must_use]
    pub fn knots(knots: Vec<u16>) -> Self {
        Self::Knots(knots)
    }

    /// Declares a uniform axis. Validation happens in [`crate::BakeInput::new`].
    #[must_use]
    pub fn uniform(origin: u16, step: u16, count: usize) -> Self {
        Self::Uniform {
            origin,
            step,
            count,
        }
    }

    /// Inclusive domain of a descriptor the runtime constructors would accept.
    pub(crate) fn bounds(&self, name: AxisName) -> Result<(u16, u16), BakeError> {
        match self {
            Self::Knots(knots) => knot_bounds(name, knots),
            Self::Uniform {
                origin,
                step,
                count,
            } => uniform_bounds(name, *origin, *step, *count),
        }
    }

    /// Declared knot values, after the same validation as [`crate::BakeInput::new`].
    ///
    /// A uniform descriptor is expanded to `origin + i * step`. The baker does
    /// not pick, adapt, or optimize knots.
    ///
    /// # Errors
    ///
    /// Returns a [`BakeError`] for an axis the runtime constructors would reject.
    pub fn knot_list(&self, name: AxisName) -> Result<Vec<u16>, BakeError> {
        match self {
            Self::Knots(knots) => {
                knot_bounds(name, knots)?;
                Ok(knots.clone())
            }
            Self::Uniform {
                origin,
                step,
                count,
            } => {
                uniform_bounds(name, *origin, *step, *count)?;
                Ok(expand_uniform(*origin, *step, *count))
            }
        }
    }
}

fn expand_uniform(origin: u16, step: u16, count: usize) -> Vec<u16> {
    (0..count)
        .map(|i| (u32::from(origin) + i as u32 * u32::from(step)) as u16)
        .collect()
}

fn knot_bounds(name: AxisName, knots: &[u16]) -> Result<(u16, u16), BakeError> {
    if knots.len() < 2 {
        return Err(match name {
            AxisName::X => BakeError::XAxisTooShort,
            AxisName::Y => BakeError::YAxisTooShort,
        });
    }
    for pair in knots.windows(2) {
        if pair[0] >= pair[1] {
            return Err(match name {
                AxisName::X => BakeError::XAxisNotStrictlyIncreasing,
                AxisName::Y => BakeError::YAxisNotStrictlyIncreasing,
            });
        }
    }
    Ok((knots[0], knots[knots.len() - 1]))
}

fn uniform_bounds(
    name: AxisName,
    origin: u16,
    step: u16,
    count: usize,
) -> Result<(u16, u16), BakeError> {
    // Same order and messages as `UniformAxis::new`.
    if count < 2 {
        return Err(BakeError::UniformTooFewKnots { axis: name });
    }
    if count > MAX_UNIFORM_KNOTS {
        return Err(BakeError::UniformTooManyKnots { axis: name });
    }
    if step < 1 {
        return Err(BakeError::UniformStepTooSmall { axis: name });
    }
    let last = (origin as usize).checked_add((count - 1).saturating_mul(step as usize));
    match last {
        Some(last) if last <= u16::MAX as usize => Ok((origin, last as u16)),
        _ => Err(BakeError::UniformLastKnotUnrepresentable { axis: name }),
    }
}

#[cfg(test)]
mod tests {
    use super::Axis;
    use crate::BakeInput;
    use crate::error::{AxisName, BakeError};

    fn empty_x(x: Axis) -> Result<BakeInput, BakeError> {
        BakeInput::new(Vec::new(), x, Axis::knots(vec![0, 1]), 1.0)
    }

    fn empty_y(y: Axis) -> Result<BakeInput, BakeError> {
        BakeInput::new(Vec::new(), Axis::knots(vec![0, 1]), y, 1.0)
    }

    #[test]
    fn x_axis_fewer_than_two_knots_is_rejected() {
        assert_eq!(empty_x(Axis::knots(vec![])), Err(BakeError::XAxisTooShort));
        assert_eq!(empty_x(Axis::knots(vec![0])), Err(BakeError::XAxisTooShort));
    }

    #[test]
    fn y_axis_fewer_than_two_knots_is_rejected() {
        assert_eq!(empty_y(Axis::knots(vec![])), Err(BakeError::YAxisTooShort));
        assert_eq!(empty_y(Axis::knots(vec![7])), Err(BakeError::YAxisTooShort));
    }

    #[test]
    fn x_knots_that_are_not_strictly_increasing_are_rejected() {
        assert_eq!(
            empty_x(Axis::knots(vec![5, 5])),
            Err(BakeError::XAxisNotStrictlyIncreasing)
        );
        assert_eq!(
            empty_x(Axis::knots(vec![10, 0])),
            Err(BakeError::XAxisNotStrictlyIncreasing)
        );
        assert_eq!(
            empty_x(Axis::knots(vec![0, 2, 2])),
            Err(BakeError::XAxisNotStrictlyIncreasing)
        );
    }

    #[test]
    fn y_knots_that_are_not_strictly_increasing_are_rejected() {
        assert_eq!(
            empty_y(Axis::knots(vec![5, 5])),
            Err(BakeError::YAxisNotStrictlyIncreasing)
        );
        assert_eq!(
            empty_y(Axis::knots(vec![10, 0])),
            Err(BakeError::YAxisNotStrictlyIncreasing)
        );
    }

    #[test]
    fn uniform_step_less_than_one_is_rejected() {
        assert_eq!(
            empty_x(Axis::uniform(0, 0, 2)),
            Err(BakeError::UniformStepTooSmall { axis: AxisName::X })
        );
        assert_eq!(
            empty_y(Axis::uniform(0, 0, 4)),
            Err(BakeError::UniformStepTooSmall { axis: AxisName::Y })
        );
    }

    #[test]
    fn uniform_last_knot_must_fit_in_u16() {
        assert_eq!(
            empty_x(Axis::uniform(60_000, 2_000, 5)),
            Err(BakeError::UniformLastKnotUnrepresentable { axis: AxisName::X })
        );
        assert_eq!(
            empty_y(Axis::uniform(1, 1, 65_536)),
            Err(BakeError::UniformLastKnotUnrepresentable { axis: AxisName::Y })
        );
    }

    #[test]
    fn uniform_count_below_two_is_rejected() {
        assert_eq!(
            empty_x(Axis::uniform(0, 1, 0)),
            Err(BakeError::UniformTooFewKnots { axis: AxisName::X })
        );
        assert_eq!(
            empty_y(Axis::uniform(0, 1, 1)),
            Err(BakeError::UniformTooFewKnots { axis: AxisName::Y })
        );
    }

    #[test]
    fn uniform_count_above_65536_is_rejected() {
        assert_eq!(
            empty_x(Axis::uniform(0, 1, 65_537)),
            Err(BakeError::UniformTooManyKnots { axis: AxisName::X })
        );
        assert_eq!(
            empty_y(Axis::uniform(0, 1, usize::MAX)),
            Err(BakeError::UniformTooManyKnots { axis: AxisName::Y })
        );
    }

    #[test]
    fn a_full_span_uniform_u16_axis_is_accepted() {
        let input = empty_x(Axis::uniform(0, 1, 65_536)).unwrap();
        assert_eq!(
            input.x(),
            &Axis::Uniform {
                origin: 0,
                step: 1,
                count: 65_536
            }
        );
    }

    #[test]
    fn x_constructor_failure_wins_when_both_axes_are_invalid() {
        assert_eq!(
            BakeInput::new(Vec::new(), Axis::knots(vec![0]), Axis::knots(vec![0]), 1.0),
            Err(BakeError::XAxisTooShort)
        );
    }

    #[test]
    fn uniform_knot_list_expands_origin_step_count() {
        let axis = Axis::uniform(0, 10, 3);
        assert_eq!(axis.knot_list(AxisName::X).unwrap(), vec![0, 10, 20]);
    }
}

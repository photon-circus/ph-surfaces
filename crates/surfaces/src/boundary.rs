//! Boundary policy vocabulary for the declared domain of a surface.
//!
//! A surface declares a rectangular domain: the closed interval spanned by its
//! X axis crossed with the closed interval spanned by its Y axis. Each of the
//! four sides of that domain independently selects what happens to a
//! coordinate falling outside it.

/// Behaviour selected for one side of one axis.
///
/// This is the whole v0.1 vocabulary. Extrapolation is never performed under
/// either selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Boundary {
    /// Reject the coordinate and report the applicable bound.
    ///
    /// This is the default for every side.
    #[default]
    Error,
    /// Substitute the nearest declared endpoint coordinate.
    ///
    /// The substituted coordinate is a declared knot, so the result is always
    /// inside the convex hull of the stored values.
    Clamp,
}

/// The four independent [`Boundary`] selections of one surface.
///
/// Every side defaults to [`Boundary::Error`], so a surface rejects
/// out-of-domain coordinates unless the definition opts into clamping. The
/// four sides are named rather than positional: there is no tuple of booleans
/// to memorise the order of.
///
/// # Examples
///
/// ```
/// use ph_surfaces::{Boundary, BoundaryPolicy};
///
/// const POLICY: BoundaryPolicy = BoundaryPolicy::new()
///     .with_x_below(Boundary::Clamp)
///     .with_y_above(Boundary::Clamp);
///
/// assert_eq!(POLICY.x_below(), Boundary::Clamp);
/// assert_eq!(POLICY.x_above(), Boundary::Error);
/// assert_eq!(POLICY.y_below(), Boundary::Error);
/// assert_eq!(POLICY.y_above(), Boundary::Clamp);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct BoundaryPolicy {
    x_below: Boundary,
    x_above: Boundary,
    y_below: Boundary,
    y_above: Boundary,
}

impl BoundaryPolicy {
    /// Returns the policy that rejects every out-of-domain coordinate.
    ///
    /// This is the same value as [`BoundaryPolicy::default`], but it is usable
    /// in a constant or static definition.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            x_below: Boundary::Error,
            x_above: Boundary::Error,
            y_below: Boundary::Error,
            y_above: Boundary::Error,
        }
    }

    /// Returns this policy with the selection for X below the domain replaced.
    #[must_use]
    pub const fn with_x_below(self, boundary: Boundary) -> Self {
        Self {
            x_below: boundary,
            ..self
        }
    }

    /// Returns this policy with the selection for X above the domain replaced.
    #[must_use]
    pub const fn with_x_above(self, boundary: Boundary) -> Self {
        Self {
            x_above: boundary,
            ..self
        }
    }

    /// Returns this policy with the selection for Y below the domain replaced.
    #[must_use]
    pub const fn with_y_below(self, boundary: Boundary) -> Self {
        Self {
            y_below: boundary,
            ..self
        }
    }

    /// Returns this policy with the selection for Y above the domain replaced.
    #[must_use]
    pub const fn with_y_above(self, boundary: Boundary) -> Self {
        Self {
            y_above: boundary,
            ..self
        }
    }

    /// Returns the selection for an X coordinate below the first X knot.
    #[must_use]
    pub const fn x_below(&self) -> Boundary {
        self.x_below
    }

    /// Returns the selection for an X coordinate above the last X knot.
    #[must_use]
    pub const fn x_above(&self) -> Boundary {
        self.x_above
    }

    /// Returns the selection for a Y coordinate below the first Y knot.
    #[must_use]
    pub const fn y_below(&self) -> Boundary {
        self.y_below
    }

    /// Returns the selection for a Y coordinate above the last Y knot.
    #[must_use]
    pub const fn y_above(&self) -> Boundary {
        self.y_above
    }
}

#[cfg(test)]
mod tests {
    use super::{Boundary, BoundaryPolicy};
    use core::mem::size_of;

    #[test]
    fn default_policy_rejects_every_side() {
        let policy = BoundaryPolicy::default();

        assert_eq!(policy.x_below(), Boundary::Error);
        assert_eq!(policy.x_above(), Boundary::Error);
        assert_eq!(policy.y_below(), Boundary::Error);
        assert_eq!(policy.y_above(), Boundary::Error);
    }

    #[test]
    fn default_trait_matches_the_const_constructor() {
        assert_eq!(BoundaryPolicy::default(), BoundaryPolicy::new());
        assert_eq!(Boundary::default(), Boundary::Error);
    }

    #[test]
    fn each_builder_method_changes_only_its_own_side() {
        let base = BoundaryPolicy::new();

        let x_below = base.with_x_below(Boundary::Clamp);
        assert_eq!(x_below.x_below(), Boundary::Clamp);
        assert_eq!(x_below.x_above(), Boundary::Error);
        assert_eq!(x_below.y_below(), Boundary::Error);
        assert_eq!(x_below.y_above(), Boundary::Error);

        let x_above = base.with_x_above(Boundary::Clamp);
        assert_eq!(x_above.x_below(), Boundary::Error);
        assert_eq!(x_above.x_above(), Boundary::Clamp);
        assert_eq!(x_above.y_below(), Boundary::Error);
        assert_eq!(x_above.y_above(), Boundary::Error);

        let y_below = base.with_y_below(Boundary::Clamp);
        assert_eq!(y_below.x_below(), Boundary::Error);
        assert_eq!(y_below.x_above(), Boundary::Error);
        assert_eq!(y_below.y_below(), Boundary::Clamp);
        assert_eq!(y_below.y_above(), Boundary::Error);

        let y_above = base.with_y_above(Boundary::Clamp);
        assert_eq!(y_above.x_below(), Boundary::Error);
        assert_eq!(y_above.x_above(), Boundary::Error);
        assert_eq!(y_above.y_below(), Boundary::Error);
        assert_eq!(y_above.y_above(), Boundary::Clamp);
    }

    #[test]
    fn builders_are_usable_in_const_context() {
        const POLICY: BoundaryPolicy = BoundaryPolicy::new()
            .with_x_below(Boundary::Clamp)
            .with_x_above(Boundary::Clamp)
            .with_y_below(Boundary::Clamp)
            .with_y_above(Boundary::Clamp);

        assert_eq!(POLICY.x_below(), Boundary::Clamp);
        assert_eq!(POLICY.x_above(), Boundary::Clamp);
        assert_eq!(POLICY.y_below(), Boundary::Clamp);
        assert_eq!(POLICY.y_above(), Boundary::Clamp);
    }

    #[test]
    fn the_policy_is_four_single_byte_selections() {
        assert_eq!(size_of::<Boundary>(), 1);
        assert_eq!(size_of::<BoundaryPolicy>(), 4);
    }
}

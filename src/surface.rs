//! The validated static representation of a rectilinear bilinear surface.

use crate::axis::{AxisLookup, BinaryAxis, BucketedAxis, LinearAxis, UniformAxis};
use crate::boundary::BoundaryPolicy;

/// A static rectilinear `u16 × u16 → i32` surface.
///
/// The handle references static tables and never owns or copies them. `NX` is
/// the number of X knots and `NY` the number of Y knots.
///
/// # Orientation
///
/// The value grid is row-major with Y selecting the row and X selecting the
/// column, so a value is addressed as `values[y][x]`. Because the grid type is
/// `&'static [[i32; NX]; NY]`, a transposed grid is a type error rather than a
/// runtime error, and there is no reachable dimension-mismatch outcome.
///
/// # Lookup strategies
///
/// The last two type parameters select how each axis locates a coordinate, and
/// they default to [`BinaryAxis`] — the general-purpose choice, and the only one
/// [`BilinearSurface::new`] can produce. `BilinearSurface<NX, NY>` therefore
/// means the binary-knotted surface it has always meant.
///
/// A firmware that wants a different trade names it in the type and builds the
/// surface with [`BilinearSurface::from_axes`]: [`LinearAxis`](crate::LinearAxis)
/// for a tiny axis, [`UniformAxis`](crate::UniformAxis) for evenly spaced knots
/// that then need not be stored at all, or
/// [`BucketedAxis`](crate::BucketedAxis) to buy a smaller search bound on a long
/// irregular axis with a few static index bytes. The two axes choose
/// independently, and the choice is a type rather than a value: there is no
/// runtime discriminant and no branch among strategies.
///
/// Whichever strategies a surface names, it locates the same cell, evaluates
/// the same value, and reports the same errors. Only the stored bytes and the
/// search work differ.
///
/// # Validation
///
/// [`BilinearSurface::new`] is a `const fn` that rejects fewer than two knots
/// on either axis and any axis that is not strictly increasing. A definition
/// that violates those invariants fails to compile. Every axis strategy
/// validates itself the same way in its own `const fn` constructor, so a surface
/// built with [`BilinearSurface::from_axes`] is validated before it exists.
///
/// # Storage
///
/// For the default binary surface the referenced table element payload is
/// exactly `2*NX + 2*NY + 4*NX*NY` bytes. In general it is
/// `X::KNOT_BYTES + X::INDEX_BYTES + Y::KNOT_BYTES + Y::INDEX_BYTES + 4*NX*NY`,
/// which the axis strategies state exactly: see
/// [`AxisLookup::KNOT_BYTES`](crate::AxisLookup::KNOT_BYTES) and
/// [`AxisLookup::INDEX_BYTES`](crate::AxisLookup::INDEX_BYTES). That figure
/// excludes this handle, alignment, linker effects, code, stack, and binary or
/// flash placement. It is not a total memory cost.
///
/// The handle always contains the value-grid reference and the four-byte
/// boundary policy. Its remaining fields depend on the axis strategies:
/// [`UniformAxis`] stores no reference, [`LinearAxis`] and [`BinaryAxis`] each
/// store one knot-array reference, and [`BucketedAxis`] stores a knot-array and
/// an index-array reference. The default binary/binary handle is therefore
/// three thin references plus the policy and alignment padding.
///
/// # Equality
///
/// The derived [`PartialEq`] and [`Hash`] implementations compare and hash the
/// referenced tables, not their addresses. Two handles over distinct but equal
/// tables therefore compare equal, at a cost proportional to `NX * NY`.
///
/// # Examples
///
/// ```
/// use ph_surfaces::{BilinearSurface, Boundary, BoundaryPolicy};
///
/// static X: [u16; 2] = [0, 100];
/// static Y: [u16; 3] = [0, 100, 200];
/// static VALUES: [[i32; 2]; 3] = [[0, 1], [2, 3], [4, 5]];
///
/// static SURFACE: BilinearSurface<2, 3> = BilinearSurface::new(&X, &Y, &VALUES)
///     .with_policy(BoundaryPolicy::new().with_y_above(Boundary::Clamp));
///
/// assert_eq!(SURFACE.nx(), 2);
/// assert_eq!(SURFACE.ny(), 3);
/// assert_eq!(SURFACE.values()[2][1], 5);
/// assert_eq!(SURFACE.policy().y_above(), Boundary::Clamp);
/// assert_eq!(SURFACE.policy().y_below(), Boundary::Error);
/// ```
///
/// A grid whose dimensions are swapped does not compile. Here the axes call
/// for `[[i32; 2]; 3]` but the grid is `[[i32; 3]; 2]`:
///
/// ```compile_fail
/// use ph_surfaces::BilinearSurface;
///
/// static X: [u16; 2] = [0, 100];
/// static Y: [u16; 3] = [0, 100, 200];
/// static VALUES: [[i32; 3]; 2] = [[0, 1, 2], [3, 4, 5]];
///
/// static SURFACE: BilinearSurface<2, 3> = BilinearSurface::new(&X, &Y, &VALUES);
/// ```
///
/// The same declaration with the correct `[[i32; NX]; NY]` shape compiles:
///
/// ```
/// use ph_surfaces::BilinearSurface;
///
/// static X: [u16; 2] = [0, 100];
/// static Y: [u16; 3] = [0, 100, 200];
/// static VALUES: [[i32; 2]; 3] = [[0, 1], [2, 3], [4, 5]];
///
/// static SURFACE: BilinearSurface<2, 3> = BilinearSurface::new(&X, &Y, &VALUES);
/// assert_eq!(SURFACE.values()[2][1], 5);
/// ```
///
/// An axis whose knot count disagrees with the value grid does not compile
/// either, because the axis type carries that count:
///
/// ```compile_fail
/// use ph_surfaces::{BilinearSurface, BinaryAxis};
///
/// static X: [u16; 3] = [0, 50, 100];
/// static Y: [u16; 2] = [0, 100];
/// static VALUES: [[i32; 2]; 2] = [[0, 1], [2, 3]];
///
/// static SURFACE: BilinearSurface<2, 2, BinaryAxis<3>, BinaryAxis<2>> =
///     BilinearSurface::from_axes(BinaryAxis::new(&X), BinaryAxis::new(&Y), &VALUES);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BilinearSurface<
    const NX: usize,
    const NY: usize,
    X: AxisLookup<NX> = BinaryAxis<NX>,
    Y: AxisLookup<NY> = BinaryAxis<NY>,
> {
    x: X,
    y: Y,
    values: &'static [[i32; NX]; NY],
    policy: BoundaryPolicy,
}

impl<const NX: usize, const NY: usize> BilinearSurface<NX, NY, BinaryAxis<NX>, BinaryAxis<NY>> {
    /// Declares a surface over static axes and a static row-major value grid,
    /// with both axes located by binary search.
    ///
    /// This is the general-purpose constructor and the one to reach for unless
    /// an axis has a reason to choose otherwise; see
    /// [`BilinearSurface::from_axes`] for the surfaces that do.
    ///
    /// Every domain side defaults to [`Boundary::Error`](crate::Boundary::Error);
    /// use [`BilinearSurface::with_policy`] to select clamping on any side.
    ///
    /// # Panics
    ///
    /// Panics unless both axes declare at least two knots and both axes are
    /// strictly increasing. In a constant or static definition that panic is a
    /// compile error, so an invalid surface cannot be defined.
    ///
    /// A single X knot is rejected:
    ///
    /// ```compile_fail
    /// use ph_surfaces::BilinearSurface;
    ///
    /// static X: [u16; 1] = [0];
    /// static Y: [u16; 2] = [0, 10];
    /// static VALUES: [[i32; 1]; 2] = [[0], [1]];
    ///
    /// static SURFACE: BilinearSurface<1, 2> = BilinearSurface::new(&X, &Y, &VALUES);
    /// ```
    ///
    /// An empty Y axis is rejected:
    ///
    /// ```compile_fail
    /// use ph_surfaces::BilinearSurface;
    ///
    /// static X: [u16; 2] = [0, 10];
    /// static Y: [u16; 0] = [];
    /// static VALUES: [[i32; 2]; 0] = [];
    ///
    /// static SURFACE: BilinearSurface<2, 0> = BilinearSurface::new(&X, &Y, &VALUES);
    /// ```
    ///
    /// A duplicated X knot is rejected:
    ///
    /// ```compile_fail
    /// use ph_surfaces::BilinearSurface;
    ///
    /// static X: [u16; 2] = [5, 5];
    /// static Y: [u16; 2] = [0, 10];
    /// static VALUES: [[i32; 2]; 2] = [[0, 1], [2, 3]];
    ///
    /// static SURFACE: BilinearSurface<2, 2> = BilinearSurface::new(&X, &Y, &VALUES);
    /// ```
    ///
    /// A descending X axis is rejected:
    ///
    /// ```compile_fail
    /// use ph_surfaces::BilinearSurface;
    ///
    /// static X: [u16; 2] = [10, 0];
    /// static Y: [u16; 2] = [0, 10];
    /// static VALUES: [[i32; 2]; 2] = [[0, 1], [2, 3]];
    ///
    /// static SURFACE: BilinearSurface<2, 2> = BilinearSurface::new(&X, &Y, &VALUES);
    /// ```
    ///
    /// A duplicated Y knot is rejected independently of the X axis:
    ///
    /// ```compile_fail
    /// use ph_surfaces::BilinearSurface;
    ///
    /// static X: [u16; 2] = [0, 10];
    /// static Y: [u16; 2] = [5, 5];
    /// static VALUES: [[i32; 2]; 2] = [[0, 1], [2, 3]];
    ///
    /// static SURFACE: BilinearSurface<2, 2> = BilinearSurface::new(&X, &Y, &VALUES);
    /// ```
    ///
    /// A descending Y axis is rejected independently of the X axis:
    ///
    /// ```compile_fail
    /// use ph_surfaces::BilinearSurface;
    ///
    /// static X: [u16; 2] = [0, 10];
    /// static Y: [u16; 2] = [10, 0];
    /// static VALUES: [[i32; 2]; 2] = [[0, 1], [2, 3]];
    ///
    /// static SURFACE: BilinearSurface<2, 2> = BilinearSurface::new(&X, &Y, &VALUES);
    /// ```
    #[must_use]
    pub const fn new(
        x_axis: &'static [u16; NX],
        y_axis: &'static [u16; NY],
        values: &'static [[i32; NX]; NY],
    ) -> Self {
        // The axis constructors below enforce the same two rules, but they
        // cannot say which axis failed: a `const fn` panic message is a literal,
        // and the strategies are axis-neutral by design. These assertions exist
        // so that the default construction path keeps naming the offending axis.
        assert!(NX >= 2, "x axis must declare at least two knots");
        assert!(NY >= 2, "y axis must declare at least two knots");

        let mut i = 1;
        while i < NX {
            assert!(
                x_axis[i - 1] < x_axis[i],
                "x axis knots must be strictly increasing"
            );
            i += 1;
        }

        let mut i = 1;
        while i < NY {
            assert!(
                y_axis[i - 1] < y_axis[i],
                "y axis knots must be strictly increasing"
            );
            i += 1;
        }

        Self {
            x: BinaryAxis::new(x_axis),
            y: BinaryAxis::new(y_axis),
            values,
            policy: BoundaryPolicy::new(),
        }
    }
}

impl<const NX: usize, const NY: usize, X: AxisLookup<NX>, Y: AxisLookup<NY>>
    BilinearSurface<NX, NY, X, Y>
{
    /// Declares a surface over two axes that have already chosen their lookup
    /// strategies, and a static row-major value grid.
    ///
    /// Each axis validated itself when it was declared, so there is nothing left
    /// to reject here: the knot counts are carried in the axis types, so an axis
    /// that does not match the grid is a type error rather than a runtime one.
    ///
    /// Every domain side defaults to [`Boundary::Error`](crate::Boundary::Error);
    /// use [`BilinearSurface::with_policy`] to select clamping on any side.
    ///
    /// # Examples
    ///
    /// A surface whose X axis is evenly spaced — so its knots are described
    /// rather than stored — and whose Y axis keeps the default strategy:
    ///
    /// ```
    /// use ph_surfaces::{BilinearSurface, BinaryAxis, UniformAxis};
    ///
    /// static Y: [u16; 2] = [0, 10];
    /// static VALUES: [[i32; 5]; 2] = [[0, 25, 50, 75, 100], [10, 35, 60, 85, 110]];
    ///
    /// static SURFACE: BilinearSurface<5, 2, UniformAxis<5, 0, 25>, BinaryAxis<2>> =
    ///     BilinearSurface::from_axes(UniformAxis::new(), BinaryAxis::new(&Y), &VALUES);
    ///
    /// assert_eq!(SURFACE.evaluate(50, 0), Ok(50));
    /// assert_eq!(SURFACE.x_knot(1), 25);
    /// ```
    #[must_use]
    pub const fn from_axes(x: X, y: Y, values: &'static [[i32; NX]; NY]) -> Self {
        Self {
            x,
            y,
            values,
            policy: BoundaryPolicy::new(),
        }
    }

    /// Returns this surface with its boundary policy replaced.
    ///
    /// The referenced tables are unchanged; only the four domain-side
    /// selections differ.
    #[must_use]
    pub const fn with_policy(self, policy: BoundaryPolicy) -> Self {
        Self {
            x: self.x,
            y: self.y,
            values: self.values,
            policy,
        }
    }

    /// Returns the X axis together with its lookup strategy.
    pub(crate) const fn x(&self) -> &X {
        &self.x
    }

    /// Returns the Y axis together with its lookup strategy.
    pub(crate) const fn y(&self) -> &Y {
        &self.y
    }

    /// Returns the declared row-major value grid, addressed as `values[y][x]`.
    #[must_use]
    pub const fn values(&self) -> &'static [[i32; NX]; NY] {
        self.values
    }

    /// Returns the number of X knots.
    #[must_use]
    pub const fn nx(&self) -> usize {
        NX
    }

    /// Returns the number of Y knots.
    #[must_use]
    pub const fn ny(&self) -> usize {
        NY
    }

    /// Returns the four domain-side selections.
    #[must_use]
    pub const fn policy(&self) -> BoundaryPolicy {
        self.policy
    }
}

macro_rules! impl_stored_x_accessors {
    ($axis:ident) => {
        impl<const NX: usize, const NY: usize, Y: AxisLookup<NY>>
            BilinearSurface<NX, NY, $axis<NX>, Y>
        {
            /// Returns the X knot at `index`.
            ///
            /// # Panics
            ///
            /// Panics if `index >= NX`.
            #[must_use]
            pub const fn x_knot(&self, index: usize) -> u16 {
                assert!(index < NX, "knot index is outside the axis");
                self.x.knots()[index]
            }

            /// Returns the first X knot: the inclusive lower bound of the X domain.
            #[must_use]
            pub const fn x_min(&self) -> u16 {
                self.x.knots()[0]
            }

            /// Returns the last X knot: the inclusive upper bound of the X domain.
            #[must_use]
            pub const fn x_max(&self) -> u16 {
                self.x.knots()[NX - 1]
            }

            /// Returns the declared X axis.
            #[must_use]
            pub const fn x_axis(&self) -> &'static [u16; NX] {
                self.x.knots()
            }
        }
    };
}

macro_rules! impl_stored_y_accessors {
    ($axis:ident) => {
        impl<const NX: usize, const NY: usize, X: AxisLookup<NX>>
            BilinearSurface<NX, NY, X, $axis<NY>>
        {
            /// Returns the Y knot at `index`.
            ///
            /// # Panics
            ///
            /// Panics if `index >= NY`.
            #[must_use]
            pub const fn y_knot(&self, index: usize) -> u16 {
                assert!(index < NY, "knot index is outside the axis");
                self.y.knots()[index]
            }

            /// Returns the first Y knot: the inclusive lower bound of the Y domain.
            #[must_use]
            pub const fn y_min(&self) -> u16 {
                self.y.knots()[0]
            }

            /// Returns the last Y knot: the inclusive upper bound of the Y domain.
            #[must_use]
            pub const fn y_max(&self) -> u16 {
                self.y.knots()[NY - 1]
            }

            /// Returns the declared Y axis.
            #[must_use]
            pub const fn y_axis(&self) -> &'static [u16; NY] {
                self.y.knots()
            }
        }
    };
}

impl_stored_x_accessors!(BinaryAxis);
impl_stored_x_accessors!(LinearAxis);
impl_stored_y_accessors!(BinaryAxis);
impl_stored_y_accessors!(LinearAxis);

impl<const NX: usize, const NY: usize, const B: usize, Y: AxisLookup<NY>>
    BilinearSurface<NX, NY, BucketedAxis<NX, B>, Y>
{
    /// Returns the X knot at `index`.
    ///
    /// # Panics
    ///
    /// Panics if `index >= NX`.
    #[must_use]
    pub const fn x_knot(&self, index: usize) -> u16 {
        assert!(index < NX, "knot index is outside the axis");
        self.x.knots()[index]
    }

    /// Returns the first X knot: the inclusive lower bound of the X domain.
    #[must_use]
    pub const fn x_min(&self) -> u16 {
        self.x.knots()[0]
    }

    /// Returns the last X knot: the inclusive upper bound of the X domain.
    #[must_use]
    pub const fn x_max(&self) -> u16 {
        self.x.knots()[NX - 1]
    }

    /// Returns the declared X axis.
    #[must_use]
    pub const fn x_axis(&self) -> &'static [u16; NX] {
        self.x.knots()
    }
}

impl<const NX: usize, const NY: usize, const B: usize, X: AxisLookup<NX>>
    BilinearSurface<NX, NY, X, BucketedAxis<NY, B>>
{
    /// Returns the Y knot at `index`.
    ///
    /// # Panics
    ///
    /// Panics if `index >= NY`.
    #[must_use]
    pub const fn y_knot(&self, index: usize) -> u16 {
        assert!(index < NY, "knot index is outside the axis");
        self.y.knots()[index]
    }

    /// Returns the first Y knot: the inclusive lower bound of the Y domain.
    #[must_use]
    pub const fn y_min(&self) -> u16 {
        self.y.knots()[0]
    }

    /// Returns the last Y knot: the inclusive upper bound of the Y domain.
    #[must_use]
    pub const fn y_max(&self) -> u16 {
        self.y.knots()[NY - 1]
    }

    /// Returns the declared Y axis.
    #[must_use]
    pub const fn y_axis(&self) -> &'static [u16; NY] {
        self.y.knots()
    }
}

impl<const NX: usize, const NY: usize, const ORIGIN: u16, const STEP: u16, Y: AxisLookup<NY>>
    BilinearSurface<NX, NY, UniformAxis<NX, ORIGIN, STEP>, Y>
{
    /// Returns the X knot at `index`, calculated from the uniform descriptor.
    ///
    /// # Panics
    ///
    /// Panics if `index >= NX`.
    #[must_use]
    pub const fn x_knot(&self, index: usize) -> u16 {
        assert!(index < NX, "knot index is outside the axis");
        ((ORIGIN as u32) + (index as u32) * (STEP as u32)) as u16
    }

    /// Returns the first X knot: the inclusive lower bound of the X domain.
    #[must_use]
    pub const fn x_min(&self) -> u16 {
        ORIGIN
    }

    /// Returns the last X knot: the inclusive upper bound of the X domain.
    #[must_use]
    pub const fn x_max(&self) -> u16 {
        ((ORIGIN as u32) + ((NX - 1) as u32) * (STEP as u32)) as u16
    }
}

impl<const NX: usize, const NY: usize, X: AxisLookup<NX>, const ORIGIN: u16, const STEP: u16>
    BilinearSurface<NX, NY, X, UniformAxis<NY, ORIGIN, STEP>>
{
    /// Returns the Y knot at `index`, calculated from the uniform descriptor.
    ///
    /// # Panics
    ///
    /// Panics if `index >= NY`.
    #[must_use]
    pub const fn y_knot(&self, index: usize) -> u16 {
        assert!(index < NY, "knot index is outside the axis");
        ((ORIGIN as u32) + (index as u32) * (STEP as u32)) as u16
    }

    /// Returns the first Y knot: the inclusive lower bound of the Y domain.
    #[must_use]
    pub const fn y_min(&self) -> u16 {
        ORIGIN
    }

    /// Returns the last Y knot: the inclusive upper bound of the Y domain.
    #[must_use]
    pub const fn y_max(&self) -> u16 {
        ((ORIGIN as u32) + ((NY - 1) as u32) * (STEP as u32)) as u16
    }
}

#[cfg(test)]
mod tests {
    use super::BilinearSurface;
    use crate::axis::{BinaryAxis, BucketedAxis, LinearAxis, UniformAxis};
    use crate::boundary::{Boundary, BoundaryPolicy};
    use core::mem::{align_of, size_of, size_of_val};

    static X2: [u16; 2] = [0, 10];
    static Y2: [u16; 2] = [0, 20];
    static V22: [[i32; 2]; 2] = [[1, 2], [3, 4]];

    // Declaring this as a `static` is itself the evidence that a valid 2x2
    // surface is constant-evaluable without allocation: it either const-
    // evaluates or the crate does not build.
    static SURFACE: BilinearSurface<2, 2> = BilinearSurface::new(&X2, &Y2, &V22);

    // These declarations guard the pre-strategy public API: default-surface
    // axis and endpoint accessors remain usable in constant expressions.
    const CONST_SURFACE: BilinearSurface<2, 2> = BilinearSurface::new(&X2, &Y2, &V22);
    const CONST_X_AXIS: &[u16; 2] = CONST_SURFACE.x_axis();
    const CONST_Y_AXIS: &[u16; 2] = CONST_SURFACE.y_axis();
    const CONST_X_MIN: u16 = CONST_SURFACE.x_min();
    const CONST_X_MAX: u16 = CONST_SURFACE.x_max();
    const CONST_Y_MIN: u16 = CONST_SURFACE.y_min();
    const CONST_Y_MAX: u16 = CONST_SURFACE.y_max();

    // A deliberately asymmetric 3x2 fixture: NX = 3, NY = 2.
    static X3: [u16; 3] = [0, 5, 100];
    static Y2B: [u16; 2] = [7, 900];
    static V23: [[i32; 3]; 2] = [[10, 20, 30], [40, 50, 60]];
    static WIDE: BilinearSurface<3, 2> = BilinearSurface::new(&X3, &Y2B, &V23);

    // The same 2x2 tables, declared through the general constructor with named
    // strategies: one stored-knot axis and one described axis.
    static MIXED: BilinearSurface<2, 2, LinearAxis<2>, UniformAxis<2, 0, 20>> =
        BilinearSurface::from_axes(LinearAxis::new(&X2), UniformAxis::new(), &V22);

    const fn side_from_bit(bits: usize, shift: u32) -> Boundary {
        if (bits >> shift) & 1 == 0 {
            Boundary::Error
        } else {
            Boundary::Clamp
        }
    }

    const fn policy_from_bits(bits: usize) -> BoundaryPolicy {
        BoundaryPolicy::new()
            .with_x_below(side_from_bit(bits, 0))
            .with_x_above(side_from_bit(bits, 1))
            .with_y_below(side_from_bit(bits, 2))
            .with_y_above(side_from_bit(bits, 3))
    }

    const POLICIES: [BoundaryPolicy; 16] = {
        let mut out = [BoundaryPolicy::new(); 16];
        let mut bits = 0;
        while bits < 16 {
            out[bits] = policy_from_bits(bits);
            bits += 1;
        }
        out
    };

    // All sixteen selections attached to the one shared table, in const context.
    static SURFACES: [BilinearSurface<2, 2>; 16] = {
        let mut out = [BilinearSurface::new(&X2, &Y2, &V22); 16];
        let mut bits = 0;
        while bits < 16 {
            out[bits] = BilinearSurface::new(&X2, &Y2, &V22).with_policy(POLICIES[bits]);
            bits += 1;
        }
        out
    };

    #[test]
    fn two_by_two_surface_declares_as_a_static() {
        assert_eq!(SURFACE.nx(), 2);
        assert_eq!(SURFACE.ny(), 2);
        assert_eq!(SURFACE.x_axis(), &[0, 10]);
        assert_eq!(SURFACE.y_axis(), &[0, 20]);
        assert_eq!(SURFACE.values(), &[[1, 2], [3, 4]]);
    }

    #[test]
    fn values_are_addressed_row_major_by_y_then_x() {
        // Y selects the row, X selects the column.
        assert_eq!(WIDE.nx(), 3);
        assert_eq!(WIDE.ny(), 2);
        assert_eq!(WIDE.values()[0][2], 30);
        assert_eq!(WIDE.values()[1][0], 40);
    }

    #[test]
    fn new_defaults_every_side_to_error() {
        let policy = SURFACE.policy();

        assert_eq!(policy, BoundaryPolicy::new());
        assert_eq!(policy.x_below(), Boundary::Error);
        assert_eq!(policy.x_above(), Boundary::Error);
        assert_eq!(policy.y_below(), Boundary::Error);
        assert_eq!(policy.y_above(), Boundary::Error);
    }

    #[test]
    fn from_axes_defaults_every_side_to_error_as_well() {
        assert_eq!(MIXED.policy(), BoundaryPolicy::new());
        assert_eq!(MIXED.policy().x_below(), Boundary::Error);
        assert_eq!(MIXED.policy().y_above(), Boundary::Error);
    }

    #[test]
    fn endpoint_accessors_report_declared_extremes() {
        assert_eq!(WIDE.x_min(), 0);
        assert_eq!(WIDE.x_max(), 100);
        assert_eq!(WIDE.y_min(), 7);
        assert_eq!(WIDE.y_max(), 900);
    }

    #[test]
    fn default_axis_accessors_remain_const_compatible() {
        assert_eq!(CONST_X_AXIS, &X2);
        assert_eq!(CONST_Y_AXIS, &Y2);
        assert_eq!((CONST_X_MIN, CONST_X_MAX), (0, 10));
        assert_eq!((CONST_Y_MIN, CONST_Y_MAX), (0, 20));
    }

    #[test]
    fn endpoint_and_knot_accessors_do_not_depend_on_a_stored_knot_array() {
        // The Y axis of this surface stores nothing, and still answers.
        assert_eq!(MIXED.y_min(), 0);
        assert_eq!(MIXED.y_max(), 20);
        assert_eq!(MIXED.y_knot(0), 0);
        assert_eq!(MIXED.y_knot(1), 20);

        // The X axis stores its knots, so both spellings agree.
        assert_eq!(MIXED.x_knot(0), MIXED.x_axis()[0]);
        assert_eq!(MIXED.x_knot(1), MIXED.x_axis()[1]);
        assert_eq!(SURFACE.x_knot(1), 10);
        assert_eq!(SURFACE.y_knot(1), 20);
    }

    #[test]
    fn accessors_return_the_declared_tables_without_copying() {
        assert!(core::ptr::eq(SURFACE.x_axis(), &X2));
        assert!(core::ptr::eq(SURFACE.y_axis(), &Y2));
        assert!(core::ptr::eq(SURFACE.values(), &V22));
    }

    #[test]
    fn handle_size_is_independent_of_table_size() {
        // The handle stores references, not tables, so growing the grid by
        // three orders of magnitude does not grow the handle.
        assert_eq!(
            size_of::<BilinearSurface<2, 2>>(),
            size_of::<BilinearSurface<64, 64>>()
        );
    }

    /// The documented referenced element payload: `2*NX + 2*NY + 4*NX*NY`.
    const fn documented_payload(nx: usize, ny: usize) -> usize {
        2 * nx + 2 * ny + 4 * nx * ny
    }

    /// The size of the three tables a `BilinearSurface<NX, NY>` references.
    const fn referenced_payload<const NX: usize, const NY: usize>() -> usize {
        size_of::<[u16; NX]>() + size_of::<[u16; NY]>() + size_of::<[[i32; NX]; NY]>()
    }

    #[test]
    fn referenced_payload_matches_the_documented_formula() {
        // `u16` and `i32` have guaranteed sizes and arrays have no padding, so
        // this holds on every target; the fixtures below merely spell out
        // representative shapes, including the asymmetric and large ones.
        assert_eq!(referenced_payload::<2, 2>(), documented_payload(2, 2));
        assert_eq!(referenced_payload::<2, 2>(), 24);
        assert_eq!(referenced_payload::<3, 2>(), documented_payload(3, 2));
        assert_eq!(referenced_payload::<3, 2>(), 34);
        assert_eq!(referenced_payload::<2, 3>(), documented_payload(2, 3));
        assert_eq!(referenced_payload::<16, 8>(), documented_payload(16, 8));
        assert_eq!(referenced_payload::<64, 64>(), documented_payload(64, 64));
        assert_eq!(referenced_payload::<64, 64>(), 16_640);

        // The same figure measured on the live tables behind a handle.
        let measured = size_of_val(SURFACE.x_axis())
            + size_of_val(SURFACE.y_axis())
            + size_of_val(SURFACE.values());
        assert_eq!(measured, documented_payload(SURFACE.nx(), SURFACE.ny()));

        let measured =
            size_of_val(WIDE.x_axis()) + size_of_val(WIDE.y_axis()) + size_of_val(WIDE.values());
        assert_eq!(measured, documented_payload(WIDE.nx(), WIDE.ny()));
    }

    #[test]
    fn default_handle_is_three_references_plus_four_policy_bytes_and_padding() {
        // The handle holds three references to sized arrays (thin, so each is
        // one target-width pointer) and one four-byte policy. Its size is
        // therefore that sum rounded up to the handle's alignment: at least the
        // sum, less than the sum plus one alignment unit, and a multiple of
        // the alignment. Nothing here assumes a particular pointer width or a
        // particular field order.
        let reference = size_of::<&'static [u16; 2]>();
        assert_eq!(
            reference,
            size_of::<usize>(),
            "a reference to a sized array is thin"
        );
        assert_eq!(size_of::<&'static [[i32; 2]; 2]>(), reference);

        let fields = 3 * reference + size_of::<BoundaryPolicy>();
        let handle = size_of::<BilinearSurface<2, 2>>();
        let align = align_of::<BilinearSurface<2, 2>>();

        assert_eq!(size_of::<BoundaryPolicy>(), 4);
        assert!(
            handle >= fields,
            "handle {handle} smaller than its fields {fields}"
        );
        assert!(
            handle < fields + align,
            "handle {handle} carries more than alignment padding over {fields}"
        );
        assert_eq!(handle % align, 0);
    }

    fn assert_handle_layout<T>(references: usize) {
        let fields = references * size_of::<usize>() + size_of::<BoundaryPolicy>();
        let handle = size_of::<T>();
        let align = align_of::<T>();

        assert!(
            handle >= fields,
            "handle {handle} smaller than its fields {fields}"
        );
        assert!(
            handle < fields + align,
            "handle {handle} carries more than alignment padding over {fields}"
        );
        assert_eq!(handle % align, 0);
    }

    #[test]
    fn handle_reference_count_follows_the_selected_strategies() {
        type UniformUniform = BilinearSurface<2, 2, UniformAxis<2, 0, 10>, UniformAxis<2, 0, 20>>;
        type LinearUniform = BilinearSurface<2, 2, LinearAxis<2>, UniformAxis<2, 0, 20>>;
        type BinaryBinary = BilinearSurface<2, 2, BinaryAxis<2>, BinaryAxis<2>>;
        type BucketedBucketed = BilinearSurface<2, 2, BucketedAxis<2, 1>, BucketedAxis<2, 1>>;

        // Every handle has one value-grid reference. Uniform contributes no
        // axis reference, linear/binary one, and bucketed two.
        assert_handle_layout::<UniformUniform>(1);
        assert_handle_layout::<LinearUniform>(2);
        assert_handle_layout::<BinaryBinary>(3);
        assert_handle_layout::<BucketedBucketed>(5);
    }

    #[test]
    fn selecting_strategies_adds_no_discriminant_to_the_handle() {
        // The default surface is exactly the binary pairing spelled out, and a
        // surface whose axes store less is smaller rather than larger: nothing
        // was added to remember which strategy is in use.
        assert_eq!(
            size_of::<BilinearSurface<2, 2>>(),
            size_of::<BilinearSurface<2, 2, BinaryAxis<2>, BinaryAxis<2>>>()
        );
        assert!(
            size_of::<BilinearSurface<2, 2, LinearAxis<2>, UniformAxis<2, 0, 20>>>()
                < size_of::<BilinearSurface<2, 2>>()
        );
    }

    #[test]
    fn with_policy_keeps_the_declared_tables() {
        let clamped = SURFACE.with_policy(BoundaryPolicy::new().with_x_below(Boundary::Clamp));

        assert!(core::ptr::eq(clamped.x_axis(), &X2));
        assert!(core::ptr::eq(clamped.y_axis(), &Y2));
        assert!(core::ptr::eq(clamped.values(), &V22));
        assert_eq!(clamped.policy().x_below(), Boundary::Clamp);
    }

    #[test]
    fn with_policy_keeps_the_declared_strategies() {
        let clamped = MIXED.with_policy(policy_from_bits(0b1111));

        assert!(core::ptr::eq(clamped.x_axis(), &X2));
        assert_eq!(clamped.y_max(), 20);
        assert_eq!(clamped.policy(), policy_from_bits(0b1111));
    }

    #[test]
    fn all_sixteen_boundary_combinations_are_representable() {
        for (bits, surface) in SURFACES.iter().enumerate() {
            let policy = surface.policy();

            assert_eq!(policy.x_below(), side_from_bit(bits, 0));
            assert_eq!(policy.x_above(), side_from_bit(bits, 1));
            assert_eq!(policy.y_below(), side_from_bit(bits, 2));
            assert_eq!(policy.y_above(), side_from_bit(bits, 3));

            // Selecting a policy never changes the table data.
            assert!(core::ptr::eq(surface.values(), &V22));
            assert!(core::ptr::eq(surface.x_axis(), &X2));
            assert!(core::ptr::eq(surface.y_axis(), &Y2));
        }
    }

    #[test]
    fn the_sixteen_combinations_are_pairwise_distinct() {
        for (i, left) in POLICIES.iter().enumerate() {
            for (j, right) in POLICIES.iter().enumerate() {
                if i == j {
                    assert_eq!(left, right);
                } else {
                    assert_ne!(left, right);
                }
            }
        }
    }

    #[test]
    #[should_panic(expected = "x axis must declare at least two knots")]
    fn zero_knot_x_axis_is_rejected() {
        static X0: [u16; 0] = [];
        static V20: [[i32; 0]; 2] = [[], []];

        let _ = BilinearSurface::new(&X0, &Y2, &V20);
    }

    #[test]
    #[should_panic(expected = "x axis must declare at least two knots")]
    fn one_knot_x_axis_is_rejected() {
        static X1: [u16; 1] = [3];
        static V21: [[i32; 1]; 2] = [[0], [1]];

        let _ = BilinearSurface::new(&X1, &Y2, &V21);
    }

    #[test]
    #[should_panic(expected = "y axis must declare at least two knots")]
    fn zero_knot_y_axis_is_rejected() {
        static Y0: [u16; 0] = [];
        static V02: [[i32; 2]; 0] = [];

        let _ = BilinearSurface::new(&X2, &Y0, &V02);
    }

    #[test]
    #[should_panic(expected = "y axis must declare at least two knots")]
    fn one_knot_y_axis_is_rejected() {
        static Y1: [u16; 1] = [3];
        static V12: [[i32; 2]; 1] = [[0, 1]];

        let _ = BilinearSurface::new(&X2, &Y1, &V12);
    }

    #[test]
    #[should_panic(expected = "x axis knots must be strictly increasing")]
    fn duplicate_x_knots_are_rejected() {
        static DUPLICATE: [u16; 2] = [5, 5];

        let _ = BilinearSurface::new(&DUPLICATE, &Y2, &V22);
    }

    #[test]
    #[should_panic(expected = "x axis knots must be strictly increasing")]
    fn descending_x_knots_are_rejected() {
        static DESCENDING: [u16; 2] = [10, 0];

        let _ = BilinearSurface::new(&DESCENDING, &Y2, &V22);
    }

    #[test]
    #[should_panic(expected = "y axis knots must be strictly increasing")]
    fn duplicate_y_knots_are_rejected() {
        static DUPLICATE: [u16; 2] = [5, 5];

        let _ = BilinearSurface::new(&X2, &DUPLICATE, &V22);
    }

    #[test]
    #[should_panic(expected = "y axis knots must be strictly increasing")]
    fn descending_y_knots_are_rejected() {
        static DESCENDING: [u16; 2] = [20, 0];

        let _ = BilinearSurface::new(&X2, &DESCENDING, &V22);
    }

    #[test]
    #[should_panic(expected = "y axis knots must be strictly increasing")]
    fn a_valid_x_axis_does_not_mask_an_invalid_y_axis() {
        // The X axis here is the valid nonuniform three-knot axis, so the
        // reported failure can only have come from the Y axis.
        static DESCENDING: [u16; 2] = [900, 7];

        let _ = BilinearSurface::new(&X3, &DESCENDING, &V23);
    }

    #[test]
    #[should_panic(expected = "x axis knots must be strictly increasing")]
    fn a_descending_interior_x_step_is_rejected() {
        static NOT_SORTED: [u16; 3] = [0, 100, 50];

        let _ = BilinearSurface::new(&NOT_SORTED, &Y2B, &V23);
    }
}

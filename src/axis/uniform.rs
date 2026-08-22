//! The uniform strategy: an origin, a step, and a count carried in the type, so
//! the axis stores no knots at all and locates a coordinate by arithmetic.

use super::{AxisLookup, sealed};

/// An axis of `N` evenly spaced knots, described by `ORIGIN` and `STEP` rather
/// than stored.
///
/// Knot `i` is `ORIGIN + i * STEP`, so the whole axis is `ORIGIN`, `STEP`, and
/// `N` — all three in the type. The value is zero-sized: an evenly spaced axis
/// costs no static bytes, adds nothing to the surface handle, and needs no knot
/// array to walk or probe. Because the descriptor is a compile-time constant,
/// the division that locates a cell is a division by a constant in every
/// instantiation.
///
/// This is the one strategy that cannot describe an arbitrary axis. If the
/// spacing is irregular, use [`BinaryAxis`](crate::BinaryAxis), or
/// [`BucketedAxis`](crate::BucketedAxis) when a smaller search bound is worth
/// index bytes.
///
/// # Cost
///
/// No stored bytes, no index, and no strategy-specific knot comparison after
/// the endpoint checks: one subtraction and one division locate the cell
/// regardless of `N`.
///
/// # Examples
///
/// ```
/// use ph_surfaces::{BilinearSurface, BinaryAxis, UniformAxis};
///
/// // Five knots at 0, 25, 50, 75, 100 — declared, not stored.
/// static Y: [u16; 2] = [0, 10];
/// static VALUES: [[i32; 5]; 2] = [[0, 25, 50, 75, 100], [10, 35, 60, 85, 110]];
///
/// static SURFACE: BilinearSurface<5, 2, UniformAxis<5, 0, 25>, BinaryAxis<2>> =
///     BilinearSurface::from_axes(UniformAxis::new(), BinaryAxis::new(&Y), &VALUES);
///
/// assert_eq!(SURFACE.x_knot(3), 75);
/// assert_eq!(SURFACE.evaluate(60, 0), Ok(60));
/// assert_eq!(SURFACE.evaluate(100, 10), Ok(110));
/// ```
///
/// The same surface with the knots spelled out and located binarily returns the
/// same values:
///
/// ```
/// use ph_surfaces::{BilinearSurface, BinaryAxis, UniformAxis};
///
/// static X: [u16; 5] = [0, 25, 50, 75, 100];
/// static Y: [u16; 2] = [0, 10];
/// static VALUES: [[i32; 5]; 2] = [[0, 25, 50, 75, 100], [10, 35, 60, 85, 110]];
///
/// static UNIFORM: BilinearSurface<5, 2, UniformAxis<5, 0, 25>, BinaryAxis<2>> =
///     BilinearSurface::from_axes(UniformAxis::new(), BinaryAxis::new(&Y), &VALUES);
/// static STORED: BilinearSurface<5, 2> = BilinearSurface::new(&X, &Y, &VALUES);
///
/// for x in [0u16, 1, 37, 50, 99, 100] {
///     assert_eq!(UNIFORM.evaluate(x, 5), STORED.evaluate(x, 5));
/// }
/// ```
///
/// A single-knot axis does not compile:
///
/// ```compile_fail
/// use ph_surfaces::UniformAxis;
///
/// static AXIS: UniformAxis<1, 0, 25> = UniformAxis::new();
/// ```
///
/// Nor does a zero step, which would declare `N` copies of one knot:
///
/// ```compile_fail
/// use ph_surfaces::UniformAxis;
///
/// static AXIS: UniformAxis<5, 0, 0> = UniformAxis::new();
/// ```
///
/// Nor does a descriptor whose last knot leaves `u16`:
///
/// ```compile_fail
/// use ph_surfaces::UniformAxis;
///
/// static AXIS: UniformAxis<5, 60_000, 2_000> = UniformAxis::new();
/// ```
///
/// The descriptor cannot bypass [`UniformAxis::new`]; its zero-sized field is
/// private:
///
/// ```compile_fail
/// use ph_surfaces::UniformAxis;
///
/// static AXIS: UniformAxis<5, 0, 25> = UniformAxis(());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UniformAxis<const N: usize, const ORIGIN: u16, const STEP: u16>(());

impl<const N: usize, const ORIGIN: u16, const STEP: u16> Default for UniformAxis<N, ORIGIN, STEP> {
    /// Returns [`UniformAxis::new`], validating the descriptor.
    ///
    /// It is written out rather than derived on purpose: a derived `Default`
    /// would hand back an axis whose descriptor had never been checked, and the
    /// rest of the crate relies on every axis having been.
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize, const ORIGIN: u16, const STEP: u16> UniformAxis<N, ORIGIN, STEP> {
    /// Declares an evenly spaced axis.
    ///
    /// # Panics
    ///
    /// Panics unless the descriptor names at least two strictly increasing knots
    /// that are all representable: `N >= 2`, `STEP >= 1`, and
    /// `N <= 65_536`, and `ORIGIN + (N - 1) * STEP <= u16::MAX`. In a constant
    /// or static definition that panic is a compile error, so an unrepresentable
    /// axis cannot be declared.
    #[must_use]
    pub const fn new() -> Self {
        assert!(N >= 2, "an axis must declare at least two knots");
        assert!(
            N <= 65_536,
            "a uniform u16 axis declares at most 65_536 knots"
        );
        assert!(
            STEP >= 1,
            "a uniform axis must declare a step of at least 1"
        );
        assert!(
            (ORIGIN as usize) + (N - 1) * (STEP as usize) <= u16::MAX as usize,
            "the last uniform knot must be representable in u16"
        );

        Self(())
    }

    /// Returns the first knot, `ORIGIN`.
    ///
    /// The same value as [`AxisLookup::first`], available in a constant context.
    #[must_use]
    pub const fn origin(&self) -> u16 {
        ORIGIN
    }

    /// Returns the spacing between consecutive knots, `STEP`.
    #[must_use]
    pub const fn step(&self) -> u16 {
        STEP
    }

    /// The descriptor arithmetic, in its one home: `ORIGIN + index * STEP`.
    ///
    /// Private and unchecked so the always-in-range callers (`last`, the
    /// trait impl) do not carry the public accessor's assert -- an extra
    /// panic path here is measurable in the emitted-instruction snapshot.
    ///
    /// Bounded above by the last knot, which `new` proved representable.
    const fn nth(index: usize) -> u16 {
        ((ORIGIN as u32) + (index as u32) * (STEP as u32)) as u16
    }

    /// Returns the knot at `index`, computed from the descriptor.
    ///
    /// The same value as [`AxisLookup::knot`], available in a constant
    /// context.
    ///
    /// # Panics
    ///
    /// Panics if `index >= N`.
    #[must_use]
    pub const fn knot(&self, index: usize) -> u16 {
        assert!(index < N, "knot index is outside the axis");

        Self::nth(index)
    }

    /// Returns the last knot, `ORIGIN + (N - 1) * STEP`.
    ///
    /// The same value as [`AxisLookup::last`], available in a constant
    /// context. `inline(always)` because the body folds to one constant; an
    /// outlined copy would be all call overhead.
    #[must_use]
    #[inline(always)]
    pub const fn last(&self) -> u16 {
        Self::nth(N - 1)
    }
}

impl<const N: usize, const ORIGIN: u16, const STEP: u16> sealed::Sealed<N>
    for UniformAxis<N, ORIGIN, STEP>
{
    #[inline(always)]
    fn search_in_domain(&self, coordinate: u16) -> (usize, u32) {
        debug_assert!(
            ORIGIN <= coordinate && coordinate <= <Self as AxisLookup<N>>::last(self),
            "the sealed search is only called on an in-domain coordinate"
        );

        // The whole location: one subtraction and one division by a constant.
        // No knot is read and no knot is compared, which is why the reported
        // comparison count is zero rather than merely small.
        let index = ((coordinate - ORIGIN) / STEP) as usize;

        debug_assert!(index < N, "a located index must stay inside the axis");

        (index, 0)
    }
}

impl<const N: usize, const ORIGIN: u16, const STEP: u16> AxisLookup<N>
    for UniformAxis<N, ORIGIN, STEP>
{
    const KNOT_BYTES: usize = 0;
    const INDEX_BYTES: usize = 0;
    const MAX_SEARCH_COMPARISONS: u32 = 0;

    fn first(&self) -> u16 {
        ORIGIN
    }

    // Both delegate to the const inherent methods above, which own the
    // descriptor arithmetic; inherent methods win resolution, so these calls
    // are not self-recursive. `inline(always)` keeps the delegation free:
    // without it the wrappers survive as outlined 8-byte functions in the
    // measurement objects, which the code-size snapshot counts.
    #[inline(always)]
    fn last(&self) -> u16 {
        Self::last(self)
    }

    #[inline(always)]
    fn knot(&self, index: usize) -> u16 {
        Self::knot(self, index)
    }
}

#[cfg(test)]
mod tests {
    use super::UniformAxis;
    use crate::axis::{AxisLookup, BinaryAxis};
    use core::mem::size_of;

    const SMALL: UniformAxis<5, 0, 25> = UniformAxis::new();
    const OFFSET: UniformAxis<9, 100, 50> = UniformAxis::new();
    const UNIT: UniformAxis<2, 7, 1> = UniformAxis::new();
    // The widest representable uniform axis: 0 and 65_535 in one step.
    const WIDE: UniformAxis<2, 0, 65_535> = UniformAxis::new();
    // The largest representable knot count: every u16 value at unit spacing.
    const FULL_COUNT: UniformAxis<65_536, 0, 1> = UniformAxis::new();

    static SMALL_KNOTS: [u16; 5] = [0, 25, 50, 75, 100];
    static OFFSET_KNOTS: [u16; 9] = [100, 150, 200, 250, 300, 350, 400, 450, 500];
    static WIDE_KNOTS: [u16; 2] = [0, 65_535];

    #[test]
    fn the_declared_knots_are_the_stored_knots_of_the_equivalent_axis() {
        for (index, &knot) in SMALL_KNOTS.iter().enumerate() {
            assert_eq!(SMALL.knot(index), knot);
        }
        for (index, &knot) in OFFSET_KNOTS.iter().enumerate() {
            assert_eq!(OFFSET.knot(index), knot);
        }

        assert_eq!((SMALL.first(), SMALL.last()), (0, 100));
        assert_eq!((OFFSET.first(), OFFSET.last()), (100, 500));
        assert_eq!((UNIT.first(), UNIT.last()), (7, 8));
        assert_eq!((WIDE.first(), WIDE.last()), (0, 65_535));
        assert_eq!((FULL_COUNT.first(), FULL_COUNT.last()), (0, 65_535));
        assert_eq!(FULL_COUNT.knot(65_535), 65_535);
    }

    #[test]
    fn arithmetic_location_agrees_with_the_binary_search_of_the_same_axis() {
        let binary = BinaryAxis::new(&SMALL_KNOTS);
        for coordinate in 0u16..=100 {
            assert_eq!(
                SMALL.search(coordinate).0,
                binary.search(coordinate).0,
                "at {coordinate}"
            );
        }

        let binary = BinaryAxis::new(&OFFSET_KNOTS);
        for coordinate in 100u16..=500 {
            assert_eq!(
                OFFSET.search(coordinate).0,
                binary.search(coordinate).0,
                "at {coordinate}"
            );
        }

        let binary = BinaryAxis::new(&WIDE_KNOTS);
        for coordinate in [0u16, 1, 32_767, 32_768, 65_534, 65_535] {
            assert_eq!(WIDE.search(coordinate).0, binary.search(coordinate).0);
        }
    }

    #[test]
    fn a_uniform_search_compares_no_knots_at_all() {
        assert_eq!(<UniformAxis<9, 100, 50>>::MAX_SEARCH_COMPARISONS, 0);

        for coordinate in 100u16..=500 {
            assert_eq!(OFFSET.search(coordinate).1, 0);
        }
    }

    #[test]
    fn a_uniform_axis_stores_nothing() {
        assert_eq!(<UniformAxis<9, 100, 50>>::KNOT_BYTES, 0);
        assert_eq!(<UniformAxis<9, 100, 50>>::INDEX_BYTES, 0);
        assert_eq!(size_of::<UniformAxis<9, 100, 50>>(), 0);
        assert_eq!(size_of::<UniformAxis<65_536, 0, 1>>(), 0);
    }

    #[test]
    fn the_descriptor_is_readable_without_a_knot_array() {
        assert_eq!(OFFSET.origin(), 100);
        assert_eq!(OFFSET.step(), 50);
        assert_eq!(WIDE.step(), 65_535);
    }

    #[test]
    #[should_panic(expected = "an axis must declare at least two knots")]
    fn a_one_knot_descriptor_is_rejected() {
        let _ = <UniformAxis<1, 0, 25>>::new();
    }

    #[test]
    #[should_panic(expected = "a uniform axis must declare a step of at least 1")]
    fn a_zero_step_is_rejected() {
        let _ = <UniformAxis<5, 0, 0>>::new();
    }

    #[test]
    #[should_panic(expected = "the last uniform knot must be representable in u16")]
    fn a_descriptor_whose_last_knot_leaves_u16_is_rejected() {
        let _ = <UniformAxis<5, 60_000, 2_000>>::new();
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    #[should_panic(expected = "a uniform u16 axis declares at most 65_536 knots")]
    fn an_oversized_count_is_rejected_before_narrowing() {
        let _ = <UniformAxis<{ (u32::MAX as usize) + 2 }, 0, 1>>::new();
    }

    #[test]
    #[should_panic(expected = "knot index is outside the axis")]
    fn a_knot_index_outside_the_axis_is_rejected() {
        let _ = SMALL.knot(5);
    }
}

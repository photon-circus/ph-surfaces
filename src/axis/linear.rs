//! The linear strategy: stored knots, no auxiliary index, bounded scan. The
//! least machinery of the four, for axes small enough that a search is not
//! worth its own code.

use super::{AxisLookup, KnotArray, assert_valid_knots, sealed};

/// An axis of `N` stored knots located by a bounded forward scan.
///
/// It stores exactly what [`BinaryAxis`](crate::BinaryAxis) stores and accepts
/// the same arbitrary spacing; it differs only in trading the halving loop for a
/// straight walk. On a three- or four-knot axis that walk is at most two or
/// three comparisons — comparable to the search it replaces, with less code
/// behind it. On a long axis it is the wrong choice, and the bound below says so
/// plainly.
///
/// # Cost
///
/// `2*N` stored bytes, no index, and at most `N - 1` strategy-specific knot
/// comparisons after the endpoint checks. Unlike the binary strategy the count
/// is data-dependent: a coordinate near the first knot costs less than one near
/// the last.
///
/// # Examples
///
/// ```
/// use ph_surfaces::{BilinearSurface, BinaryAxis, LinearAxis};
///
/// static X: [u16; 3] = [0, 30, 100];
/// static Y: [u16; 2] = [0, 10];
/// static VALUES: [[i32; 3]; 2] = [[0, 30, 100], [10, 40, 110]];
///
/// // A tiny X axis scans; the Y axis keeps the default strategy.
/// static SURFACE: BilinearSurface<3, 2, LinearAxis<3>, BinaryAxis<2>> =
///     BilinearSurface::from_axes(LinearAxis::new(&X), BinaryAxis::new(&Y), &VALUES);
///
/// // Same answers as the all-binary surface over the same tables.
/// static DEFAULT: BilinearSurface<3, 2> = BilinearSurface::new(&X, &Y, &VALUES);
/// assert_eq!(SURFACE.evaluate(65, 5), DEFAULT.evaluate(65, 5));
/// assert_eq!(SURFACE.evaluate(65, 5), Ok(70));
/// ```
///
/// An axis of fewer than two knots does not compile:
///
/// ```compile_fail
/// use ph_surfaces::LinearAxis;
///
/// static X: [u16; 1] = [7];
/// static AXIS: LinearAxis<1> = LinearAxis::new(&X);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LinearAxis<const N: usize> {
    knots: &'static [u16; N],
}

impl<const N: usize> LinearAxis<N> {
    /// Declares a scanned axis over static knots.
    ///
    /// # Panics
    ///
    /// Panics unless the axis declares at least two strictly increasing knots.
    /// In a constant or static definition that panic is a compile error.
    #[must_use]
    pub const fn new(knots: &'static [u16; N]) -> Self {
        assert_valid_knots(knots);

        Self { knots }
    }

    /// Returns the declared knots.
    ///
    /// The same array as [`KnotArray::knots`], available in a constant context.
    #[must_use]
    pub const fn knots(&self) -> &'static [u16; N] {
        self.knots
    }
}

impl<const N: usize> sealed::Sealed<N> for LinearAxis<N> {
    #[inline(always)]
    fn search_in_domain(&self, coordinate: u16) -> (usize, u32) {
        debug_assert!(
            self.knots[0] <= coordinate && coordinate <= self.knots[N - 1],
            "the sealed search is only called on an in-domain coordinate"
        );

        let mut index = 0;
        let mut comparisons = 0;

        // Walk while the *next* knot is still at or below the coordinate. The
        // walk therefore stops on the greatest such knot, and it can never step
        // past `N - 1`.
        while index + 1 < N {
            comparisons += 1;
            if self.knots[index + 1] > coordinate {
                break;
            }
            index += 1;
        }

        debug_assert!(
            comparisons <= <Self as AxisLookup<N>>::MAX_SEARCH_COMPARISONS,
            "the scan must stay inside the documented bound"
        );
        debug_assert!(
            self.knots[index] <= coordinate,
            "the located knot must not sit above the coordinate"
        );

        (index, comparisons)
    }
}

impl<const N: usize> KnotArray<N> for LinearAxis<N> {
    fn knots(&self) -> &'static [u16; N] {
        self.knots
    }
}

impl<const N: usize> AxisLookup<N> for LinearAxis<N> {
    const KNOT_BYTES: usize = 2 * N;
    const INDEX_BYTES: usize = 0;
    // The scan stops at the last knot, so it can compare against at most the
    // `N - 1` knots above the first one.
    const MAX_SEARCH_COMPARISONS: u32 = (N - 1) as u32;

    fn first(&self) -> u16 {
        self.knots[0]
    }

    fn last(&self) -> u16 {
        self.knots[N - 1]
    }

    fn knot(&self, index: usize) -> u16 {
        self.knots[index]
    }
}

#[cfg(test)]
mod tests {
    use super::LinearAxis;
    use crate::axis::{AxisLookup, BinaryAxis, KnotArray, probes};

    static X_TINY: [u16; 2] = [7, 9];
    static X_MAIN: [u16; 5] = [10, 20, 30, 40, 50];
    static X_SPARSE: [u16; 6] = [3, 4, 5, 1_000, 40_000, 65_000];
    static X_FULL: [u16; 4] = [0, 1, 32_768, 65_535];

    const TINY: LinearAxis<2> = LinearAxis::new(&X_TINY);
    const MAIN: LinearAxis<5> = LinearAxis::new(&X_MAIN);
    const SPARSE: LinearAxis<6> = LinearAxis::new(&X_SPARSE);
    const FULL: LinearAxis<4> = LinearAxis::new(&X_FULL);

    #[test]
    fn the_scan_locates_the_same_index_as_the_binary_search() {
        macro_rules! agrees_with_binary {
            ($linear:expr, $knots:expr, $stride:expr) => {
                let linear = $linear;
                let binary = BinaryAxis::new($knots);

                for coordinate in probes($knots, $stride) {
                    assert_eq!(
                        linear.search(coordinate).0,
                        binary.search(coordinate).0,
                        "at {coordinate}"
                    );
                }
            };
        }

        agrees_with_binary!(TINY, &X_TINY, 1);
        agrees_with_binary!(MAIN, &X_MAIN, 1);
        agrees_with_binary!(SPARSE, &X_SPARSE, 97);
        agrees_with_binary!(FULL, &X_FULL, 97);
    }

    #[test]
    fn every_knot_locates_itself() {
        for (index, &knot) in X_MAIN.iter().enumerate() {
            assert_eq!(MAIN.search(knot).0, index, "at knot {knot}");
        }
        for (index, &knot) in X_SPARSE.iter().enumerate() {
            assert_eq!(SPARSE.search(knot).0, index, "at knot {knot}");
        }
    }

    #[test]
    fn the_scan_never_exceeds_its_declared_bound() {
        assert_eq!(<LinearAxis<2>>::MAX_SEARCH_COMPARISONS, 1);
        assert_eq!(<LinearAxis<5>>::MAX_SEARCH_COMPARISONS, 4);
        assert_eq!(<LinearAxis<6>>::MAX_SEARCH_COMPARISONS, 5);

        for coordinate in 10u16..=50 {
            assert!(MAIN.search(coordinate).1 <= <LinearAxis<5>>::MAX_SEARCH_COMPARISONS);
        }

        // The bound is reached only at the far end of the axis, and the cost
        // below the first interior knot is one comparison.
        assert_eq!(MAIN.search(50).1, 4);
        assert_eq!(MAIN.search(10).1, 1);
        assert_eq!(TINY.search(9).1, 1);
    }

    #[test]
    fn the_knot_array_is_referenced_and_never_copied() {
        assert!(core::ptr::eq(MAIN.knots(), &X_MAIN));
        assert!(core::ptr::eq(KnotArray::knots(&MAIN), &X_MAIN));
        assert_eq!(<LinearAxis<5>>::KNOT_BYTES, 10);
        assert_eq!(<LinearAxis<5>>::INDEX_BYTES, 0);
    }

    #[test]
    #[should_panic(expected = "an axis must declare at least two knots")]
    fn a_one_knot_axis_is_rejected() {
        static ONE: [u16; 1] = [3];

        let _ = LinearAxis::new(&ONE);
    }

    #[test]
    #[should_panic(expected = "axis knots must be strictly increasing")]
    fn a_descending_knot_is_rejected() {
        static DESCENDING: [u16; 3] = [0, 100, 50];

        let _ = LinearAxis::new(&DESCENDING);
    }
}

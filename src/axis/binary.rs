//! The binary strategy: stored knots, no auxiliary index, exact logarithmic
//! search. This is the crate's default and the general-purpose choice.

use super::{AxisLookup, KnotArray, assert_valid_knots, probe_bound, sealed};

/// An axis of `N` stored knots located by binary search.
///
/// This is the strategy a [`BilinearSurface`](crate::BilinearSurface) selects
/// when its type does not say otherwise, and it is the right answer for almost
/// every axis: it accepts arbitrary spacing, stores nothing beyond the knots
/// themselves, and its search cost grows logarithmically.
///
/// # Cost
///
/// `2*N` stored bytes, no index, and exactly `ceil(log2(N))` knot comparisons
/// per in-domain search — for a 1024-knot axis, ten comparisons where a scan
/// would take up to 1024.
///
/// The count is exact rather than a bound, and it depends only on `N`: the same
/// number of comparisons for an endpoint, for an exact interior knot, and for a
/// point strictly inside a segment. There is no early exit on an exact match,
/// deliberately, because it would trade a uniform and auditable step count for a
/// data-dependent one.
///
/// # Examples
///
/// ```
/// use ph_surfaces::{BilinearSurface, BinaryAxis};
///
/// static X: [u16; 4] = [0, 10, 90, 500];
/// static Y: [u16; 2] = [0, 100];
/// static VALUES: [[i32; 4]; 2] = [[0, 10, 90, 500], [100, 110, 190, 600]];
///
/// // Naming the strategy and letting it default are the same surface.
/// static NAMED: BilinearSurface<4, 2, BinaryAxis<4>, BinaryAxis<2>> =
///     BilinearSurface::from_axes(BinaryAxis::new(&X), BinaryAxis::new(&Y), &VALUES);
/// static DEFAULTED: BilinearSurface<4, 2> = BilinearSurface::new(&X, &Y, &VALUES);
///
/// assert_eq!(NAMED.evaluate(50, 50), DEFAULTED.evaluate(50, 50));
/// assert_eq!(NAMED, DEFAULTED);
/// ```
///
/// An axis of fewer than two knots does not compile:
///
/// ```compile_fail
/// use ph_surfaces::BinaryAxis;
///
/// static X: [u16; 1] = [7];
/// static AXIS: BinaryAxis<1> = BinaryAxis::new(&X);
/// ```
///
/// Nor does one whose knots are not strictly increasing:
///
/// ```compile_fail
/// use ph_surfaces::BinaryAxis;
///
/// static X: [u16; 3] = [0, 100, 50];
/// static AXIS: BinaryAxis<3> = BinaryAxis::new(&X);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BinaryAxis<const N: usize> {
    knots: &'static [u16; N],
}

impl<const N: usize> BinaryAxis<N> {
    /// Declares a binary-searched axis over static knots.
    ///
    /// # Panics
    ///
    /// Panics unless the axis declares at least two strictly increasing knots.
    /// In a constant or static definition that panic is a compile error, so an
    /// invalid axis cannot be declared.
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

impl<const N: usize> sealed::Sealed for BinaryAxis<N> {}

impl<const N: usize> KnotArray<N> for BinaryAxis<N> {
    fn knots(&self) -> &'static [u16; N] {
        self.knots
    }
}

impl<const N: usize> AxisLookup<N> for BinaryAxis<N> {
    const KNOT_BYTES: usize = 2 * N;
    const INDEX_BYTES: usize = 0;
    const MAX_SEARCH_COMPARISONS: u32 = probe_bound(N);

    fn first(&self) -> u16 {
        self.knots[0]
    }

    fn last(&self) -> u16 {
        self.knots[N - 1]
    }

    fn knot(&self, index: usize) -> u16 {
        self.knots[index]
    }

    fn search(&self, coordinate: u16) -> (usize, u32) {
        debug_assert!(
            self.knots[0] <= coordinate,
            "search is only called on a coordinate at or above the first knot"
        );

        let mut base = 0;
        let mut size = N;
        let mut probes = 0;

        while size > 1 {
            let half = size / 2;
            let mid = base + half;

            // The answer stays in `base ..= base + size - 1`: a knot at or below
            // the coordinate moves the window's floor up to `mid`, and a knot
            // above it leaves the floor alone. Either way the window shrinks.
            if self.knots[mid] <= coordinate {
                base = mid;
            }

            size -= half;
            probes += 1;
        }

        debug_assert_eq!(
            probes,
            Self::MAX_SEARCH_COMPARISONS,
            "the probe count must match the documented bound exactly"
        );
        debug_assert!(
            self.knots[base] <= coordinate,
            "the located knot must not sit above the coordinate"
        );

        (base, probes)
    }
}

#[cfg(test)]
mod tests {
    use super::BinaryAxis;
    use crate::axis::{AxisLookup, KnotArray, probe_bound};

    static X_MAIN: [u16; 5] = [10, 20, 30, 40, 50];
    static X_SPARSE: [u16; 6] = [3, 4, 5, 1_000, 40_000, 65_000];
    static X_FULL: [u16; 4] = [0, 1, 32_768, 65_535];
    static X_TINY: [u16; 2] = [7, 9];

    // A thousand knots, so a scan would cost up to 1024 comparisons where the
    // search costs ten.
    static X_BIG: [u16; 1024] = {
        let mut axis = [0u16; 1024];
        let mut i = 0;
        let mut knot = 0u16;

        while i < 1024 {
            axis[i] = knot;
            i += 1;

            // Guarded so the final increment cannot leave `u16`: the last knot
            // is 1023 * 64 == 65_472.
            if i < 1024 {
                knot += 64;
            }
        }

        axis
    };

    const MAIN: BinaryAxis<5> = BinaryAxis::new(&X_MAIN);
    const SPARSE: BinaryAxis<6> = BinaryAxis::new(&X_SPARSE);
    const FULL: BinaryAxis<4> = BinaryAxis::new(&X_FULL);
    const TINY: BinaryAxis<2> = BinaryAxis::new(&X_TINY);
    const BIG: BinaryAxis<1024> = BinaryAxis::new(&X_BIG);

    // An independent statement of `ceil(log2(len))`: it counts the halvings
    // rather than reading a bit length, so agreement is evidence that the
    // documented formula and the loop shape describe the same thing.
    fn ceil_log2_reference(len: usize) -> u32 {
        let mut remaining = len;
        let mut steps = 0;

        while remaining > 1 {
            remaining = remaining.div_ceil(2);
            steps += 1;
        }

        steps
    }

    #[test]
    fn the_probe_bound_matches_the_documented_formula() {
        for len in [2usize, 3, 4, 5, 6, 7, 8, 9, 16, 17, 1_024, 65_535, 65_536] {
            assert_eq!(probe_bound(len), ceil_log2_reference(len), "length {len}");
        }

        assert_eq!(probe_bound(1_024), 10);
        assert_eq!(<BinaryAxis<1024>>::MAX_SEARCH_COMPARISONS, 10);
        assert_eq!(<BinaryAxis<5>>::MAX_SEARCH_COMPARISONS, 3);
        assert_eq!(<BinaryAxis<2>>::MAX_SEARCH_COMPARISONS, 1);
    }

    #[test]
    fn the_search_probe_count_is_exact_and_data_independent() {
        macro_rules! exact_on {
            ($axis:expr) => {
                let axis = $axis;
                let expected = axis.knots().len();
                let expected = probe_bound(expected);

                for &knot in axis.knots() {
                    assert_eq!(axis.search(knot).1, expected, "at knot {knot}");

                    if knot > axis.first() {
                        assert_eq!(axis.search(knot - 1).1, expected);
                    }
                    if knot < axis.last() {
                        assert_eq!(axis.search(knot + 1).1, expected);
                    }
                }
            };
        }

        exact_on!(MAIN);
        exact_on!(SPARSE);
        exact_on!(FULL);
        exact_on!(TINY);
        exact_on!(BIG);

        // A sweep across one axis: the count never moves off the bound, which is
        // what "independent of the data" means.
        let mut coordinate = 0u16;
        while coordinate < 65_472 {
            assert_eq!(BIG.search(coordinate).1, 10);
            coordinate += 397;
        }
    }

    #[test]
    fn binary_lookup_costs_far_fewer_comparisons_than_a_scan() {
        let (_, probes) = BIG.search(40_000);
        let scan = u32::try_from(X_BIG.len()).expect("the axis length fits in u32");

        assert_eq!(probes, 10);
        assert!(
            probes * 100 < scan,
            "{probes} probes is not two orders of magnitude below {scan}"
        );
    }

    #[test]
    fn the_located_index_is_the_greatest_knot_at_or_below_the_coordinate() {
        for (index, &knot) in X_SPARSE.iter().enumerate() {
            assert_eq!(SPARSE.search(knot).0, index, "at knot {knot}");

            if knot > SPARSE.first() {
                assert_eq!(SPARSE.search(knot - 1).0, index - 1);
            }
        }

        assert_eq!(SPARSE.search(999).0, 2);
        assert_eq!(SPARSE.search(1_000).0, 3);
        assert_eq!(SPARSE.search(39_999).0, 3);
        assert_eq!(FULL.search(65_535).0, 3);
        assert_eq!(TINY.search(8).0, 0);
    }

    #[test]
    fn the_knot_array_is_referenced_and_never_copied() {
        assert!(core::ptr::eq(MAIN.knots(), &X_MAIN));
        assert!(core::ptr::eq(KnotArray::knots(&MAIN), &X_MAIN));
        assert_eq!(<BinaryAxis<5>>::KNOT_BYTES, 10);
        assert_eq!(<BinaryAxis<5>>::INDEX_BYTES, 0);
    }

    #[test]
    #[should_panic(expected = "an axis must declare at least two knots")]
    fn a_one_knot_axis_is_rejected() {
        static ONE: [u16; 1] = [3];

        let _ = BinaryAxis::new(&ONE);
    }

    #[test]
    #[should_panic(expected = "axis knots must be strictly increasing")]
    fn a_duplicated_knot_is_rejected() {
        static DUPLICATE: [u16; 2] = [5, 5];

        let _ = BinaryAxis::new(&DUPLICATE);
    }

    #[test]
    #[should_panic(expected = "axis knots must be strictly increasing")]
    fn a_descending_knot_is_rejected() {
        static DESCENDING: [u16; 3] = [0, 100, 50];

        let _ = BinaryAxis::new(&DESCENDING);
    }
}

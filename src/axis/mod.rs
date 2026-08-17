//! Per-axis lookup strategies, chosen in the type of a surface rather than at
//! runtime.
//!
//! Every strategy answers one question — *which segment of this axis contains
//! this coordinate* — and answers it identically. They differ only in what they
//! store and how much work the answer costs:
//!
//! | Strategy | Stored per axis | Strategy work after endpoint checks | Choose when |
//! | --- | --- | --- | --- |
//! | [`LinearAxis`] | `2*N` knot bytes | bounded scan, at most `N - 1` comparisons | tiny axis; minimum auxiliary structure |
//! | [`BinaryAxis`] | `2*N` knot bytes | exactly `ceil(log2(N))` comparisons | the general default |
//! | [`UniformAxis`] | none | no comparison at all | even spacing; drop knot arrays; constant location |
//! | [`BucketedAxis`] | `2*N` knot bytes plus `2*B` index bytes | one bucket read plus a bounded local scan | irregular axis; extra index bytes for a smaller local bound |
//!
//! # Why the choice is in the type
//!
//! A runtime enum would add a branch to every lookup and would keep all four
//! implementations in the image even when a firmware uses one. A Cargo feature
//! would be worse: Cargo unifies features across the whole dependency graph, so
//! an unrelated downstream crate could change which strategy a firmware
//! compiles. Selecting in the type leaves the decision where the surface is
//! declared, and a firmware that names one combination compiles exactly that
//! one.
//!
//! # What a strategy does not decide
//!
//! A strategy locates. It does not decide boundary behaviour, error identity,
//! rounding, or composition order:
//!
//! * the private lookup module owns the endpoint tests, the [`Boundary`]
//!   policy, the clamped-coordinate substitution, and the cell invariants.
//!   Every strategy reaches the evaluator through that one function.
//! * the private interpolation module owns rounding, and the evaluator owns
//!   the X-then-Y composition.
//!
//! That split is why swapping a strategy cannot change a value, an error, or a
//! boundary outcome, and why there is one numerical path rather than four.
//!
//! [`Boundary`]: crate::Boundary

mod binary;
mod bucketed;
mod linear;
mod uniform;

pub use binary::BinaryAxis;
pub use bucketed::{BucketedAxis, bucket_index, max_local_comparisons};
pub use linear::LinearAxis;
pub use uniform::UniformAxis;

/// Seals [`AxisLookup`] and [`KnotArray`] against outside implementations.
///
/// The two traits carry preconditions the crate relies on for memory safety of
/// its indexing — a located index is always in range, and a validated axis is
/// always strictly increasing — but they are checked by construction inside
/// this module rather than by the type system. A downstream implementation
/// could satisfy the signatures while violating them, so the trait is closed.
mod sealed {
    /// Implemented only by this crate's four axis strategies.
    pub trait Sealed<const N: usize> {
        /// Locates a coordinate after the shared caller has established that it
        /// is inside the inclusive axis domain.
        fn search_in_domain(&self, coordinate: u16) -> (usize, u32);
    }
}

/// One axis of `N` knots, together with the strategy that locates a coordinate
/// in it.
///
/// This trait is sealed: [`LinearAxis`], [`BinaryAxis`], [`UniformAxis`], and
/// [`BucketedAxis`] are the only implementations, and each validates its own
/// invariants in a `const fn` constructor. An invalid axis therefore fails to
/// compile rather than reaching [`search`](AxisLookup::search).
///
/// `N` is a parameter of the trait rather than an associated constant so that
/// an axis and the value grid it addresses cannot drift apart: a
/// [`BilinearSurface`](crate::BilinearSurface) requires `X: AxisLookup<NX>`, so
/// pairing a 5-knot axis with a grid of 4 columns is a type error at the
/// declaration.
///
/// # Invariants
///
/// Every implementation guarantees that `N >= 2` and that
/// `knot(0) < knot(1) < .. < knot(N - 1)`, all of them representable in `u16`.
pub trait AxisLookup<const N: usize>: sealed::Sealed<N> + Copy {
    /// Bytes of stored knots or descriptor this strategy references.
    ///
    /// This is the axis's own static payload. It excludes the value grid, the
    /// auxiliary index counted by [`INDEX_BYTES`](AxisLookup::INDEX_BYTES), the
    /// surface handle, alignment, code, and stack. It is exact and
    /// target-independent, and it is not a total memory figure.
    const KNOT_BYTES: usize;

    /// Bytes of auxiliary index this strategy references.
    ///
    /// Zero for every strategy except [`BucketedAxis`], which is the only one
    /// that buys a smaller search bound with static data.
    const INDEX_BYTES: usize;

    /// The most strategy-specific knot comparisons needed to locate an
    /// in-domain coordinate, after the inclusive endpoint checks.
    ///
    /// This counts comparisons against stored knots, not machine instructions,
    /// and it excludes the two endpoint comparisons performed before the
    /// strategy-specific search. The public [`search`](AxisLookup::search)
    /// wrapper performs those checks itself; surface evaluation reuses the
    /// endpoint checks it already needs for boundary handling. It is a work
    /// bound, never a cycle count.
    const MAX_SEARCH_COMPARISONS: u32;

    /// Returns the first knot: the inclusive lower bound of the axis domain.
    fn first(&self) -> u16;

    /// Returns the last knot: the inclusive upper bound of the axis domain.
    fn last(&self) -> u16;

    /// Returns the knot at `index`.
    ///
    /// # Panics
    ///
    /// Panics if `index >= N`. Every index this crate passes comes from a
    /// located cell, whose lower knot is at most `N - 2` and whose upper knot is
    /// therefore at most `N - 1`.
    fn knot(&self, index: usize) -> u16;

    /// Returns the greatest index whose knot is at or below `coordinate`,
    /// together with the number of knot comparisons it took.
    ///
    /// The returned count covers only the strategy-specific work and is bounded
    /// by [`MAX_SEARCH_COMPARISONS`](AxisLookup::MAX_SEARCH_COMPARISONS). It
    /// exists so tests can tell the strategies apart by their work rather than
    /// only by their answers. Callers normally discard it and an optimising
    /// build removes it.
    ///
    /// # Panics
    ///
    /// Panics in every build profile unless
    /// `first() <= coordinate <= last()`. This makes the answer set non-empty
    /// and the returned index well defined.
    ///
    /// # Cost
    ///
    /// A direct public call performs one or two endpoint comparisons before the
    /// strategy-specific work (two for an in-domain coordinate). The surface
    /// evaluator does not duplicate those comparisons: its private locator
    /// performs the same endpoint checks for boundary handling, then enters the
    /// sealed in-domain search directly.
    fn search(&self, coordinate: u16) -> (usize, u32) {
        assert!(
            self.first() <= coordinate && coordinate <= self.last(),
            "search coordinate must be inside the inclusive axis domain"
        );

        search_in_domain(self, coordinate)
    }
}

/// Enters a strategy after the shared locator has checked both domain endpoints.
#[inline(always)]
pub(crate) fn search_in_domain<const N: usize, A: AxisLookup<N>>(
    axis: &A,
    coordinate: u16,
) -> (usize, u32) {
    <A as sealed::Sealed<N>>::search_in_domain(axis, coordinate)
}

/// An axis whose knots are stored as one static array.
///
/// [`LinearAxis`], [`BinaryAxis`], and [`BucketedAxis`] implement it;
/// [`UniformAxis`] does not, because it stores no knots to hand out. It is what
/// keeps [`BilinearSurface::x_axis`](crate::BilinearSurface::x_axis) available
/// for the strategies that have an array, without inventing one for the
/// strategy that deliberately does not.
///
/// This trait is sealed for the same reason [`AxisLookup`] is.
pub trait KnotArray<const N: usize>: AxisLookup<N> {
    /// Returns the declared knots, strictly increasing and at least two long.
    fn knots(&self) -> &'static [u16; N];
}

/// Returns `ceil(log2(len))`: the number of probes [`BinaryAxis`] performs on an
/// axis of `len` knots.
///
/// This is the bit length of `len - 1`, which is target-independent even though
/// `usize::BITS` is not.
///
/// It is an exact count rather than an upper bound, because the search window
/// shrinks by `size -> ceil(size / 2)` on both branches.
///
/// # Preconditions
///
/// `len >= 2`. Every axis this crate can construct satisfies it, so `len - 1`
/// cannot underflow.
pub(crate) const fn probe_bound(len: usize) -> u32 {
    debug_assert!(len >= 2, "an axis declares at least two knots");

    usize::BITS - (len - 1).leading_zeros()
}

/// Asserts that `knots` is a usable axis: at least two knots, strictly
/// increasing.
///
/// Shared by the three stored-knot strategies so that one rule is stated once.
/// The message is axis-neutral because a `const fn` panic cannot name the axis
/// it was called for; [`BilinearSurface::new`](crate::BilinearSurface::new)
/// keeps its own X- and Y-named assertions for the default construction path.
const fn assert_valid_knots<const N: usize>(knots: &[u16; N]) {
    assert!(N >= 2, "an axis must declare at least two knots");

    let mut i = 1;
    while i < N {
        assert!(
            knots[i - 1] < knots[i],
            "axis knots must be strictly increasing"
        );
        i += 1;
    }
}

/// Every knot of an axis and its two neighbours, plus a stride across the whole
/// domain, with anything outside the domain dropped.
///
/// Those are the coordinates where a cell boundary can be got wrong, and enough
/// of the interior to catch a locator that is wrong in bulk. The strategy
/// modules share it so that they are all held to the same probe set.
#[cfg(test)]
pub(crate) fn probes<const N: usize>(
    knots: &'static [u16; N],
    stride: usize,
) -> impl Iterator<Item = u16> {
    knots
        .iter()
        .flat_map(|&knot| [knot.saturating_sub(1), knot, knot.saturating_add(1)])
        .chain((knots[0]..=knots[N - 1]).step_by(stride))
        .filter(|&coordinate| coordinate >= knots[0] && coordinate <= knots[N - 1])
}

#[cfg(test)]
mod tests {
    use super::{
        AxisLookup, BinaryAxis, BucketedAxis, LinearAxis, UniformAxis, bucket_index, probes,
    };
    use crate::boundary::{Boundary, BoundaryPolicy};
    use crate::error::SurfaceError;
    use crate::surface::BilinearSurface;
    use core::mem::size_of;

    // A uniformly spaced axis, so all four strategies can describe it and the
    // equivalence checks below have something all four can be compared on.
    static UNIFORM_KNOTS: [u16; 9] = [100, 150, 200, 250, 300, 350, 400, 450, 500];
    static UNIFORM_BUCKETS: [u16; 4] = bucket_index(&UNIFORM_KNOTS);

    // The Y axis is the same shape but shorter, so a mixed pairing is not
    // accidentally symmetric.
    static Y_KNOTS: [u16; 3] = [10, 20, 30];
    static Y_BUCKETS: [u16; 2] = bucket_index(&Y_KNOTS);

    // Strictly convex in both indices, so a wrongly located cell changes the
    // value instead of hiding behind collinear data.
    static VALUES: [[i32; 9]; 3] = [
        [0, 1, 4, 9, 16, 25, 36, 49, 64],
        [100, 102, 108, 118, 132, 150, 172, 198, 228],
        [-50, -49, -46, -41, -34, -25, -14, -1, 14],
    ];

    const LINEAR_X: LinearAxis<9> = LinearAxis::new(&UNIFORM_KNOTS);
    const BINARY_X: BinaryAxis<9> = BinaryAxis::new(&UNIFORM_KNOTS);
    const UNIFORM_X: UniformAxis<9, 100, 50> = UniformAxis::new();
    const BUCKETED_X: BucketedAxis<9, 4> = BucketedAxis::new(&UNIFORM_KNOTS, &UNIFORM_BUCKETS);

    const LINEAR_Y: LinearAxis<3> = LinearAxis::new(&Y_KNOTS);
    const BINARY_Y: BinaryAxis<3> = BinaryAxis::new(&Y_KNOTS);
    const UNIFORM_Y: UniformAxis<3, 10, 10> = UniformAxis::new();
    const BUCKETED_Y: BucketedAxis<3, 2> = BucketedAxis::new(&Y_KNOTS, &Y_BUCKETS);

    // An irregular axis: only the three stored-knot strategies can describe it.
    static IRREGULAR_KNOTS: [u16; 7] = [3, 4, 5, 1_000, 40_000, 65_000, 65_535];
    static IRREGULAR_BUCKETS: [u16; 8] = bucket_index(&IRREGULAR_KNOTS);
    static IRREGULAR_VALUES: [[i32; 7]; 3] = [
        [0, 1, 4, 9, 16, 25, 36],
        [100, 102, 108, 118, 132, 150, 172],
        [-50, -49, -46, -41, -34, -25, -14],
    ];

    const IRREGULAR_LINEAR: LinearAxis<7> = LinearAxis::new(&IRREGULAR_KNOTS);
    const IRREGULAR_BINARY: BinaryAxis<7> = BinaryAxis::new(&IRREGULAR_KNOTS);
    const IRREGULAR_BUCKETED: BucketedAxis<7, 8> =
        BucketedAxis::new(&IRREGULAR_KNOTS, &IRREGULAR_BUCKETS);

    /// Every coordinate worth probing on the uniform fixture: each knot, one
    /// unit either side of it, both endpoints, and coordinates outside the
    /// domain on both sides.
    fn uniform_probes() -> impl Iterator<Item = u16> {
        (99u16..=501).chain([0, 50, 502, 1_000, u16::MAX])
    }

    #[test]
    fn every_strategy_locates_the_same_cell_on_an_equivalent_axis() {
        for coordinate in 100u16..=500 {
            let expected = BINARY_X.search(coordinate).0;

            assert_eq!(LINEAR_X.search(coordinate).0, expected, "at {coordinate}");
            assert_eq!(UNIFORM_X.search(coordinate).0, expected, "at {coordinate}");
            assert_eq!(BUCKETED_X.search(coordinate).0, expected, "at {coordinate}");
        }
    }

    #[test]
    fn every_strategy_reports_the_same_domain_and_knots() {
        for (index, &expected) in UNIFORM_KNOTS.iter().enumerate() {
            assert_eq!(LINEAR_X.knot(index), expected);
            assert_eq!(BINARY_X.knot(index), expected);
            assert_eq!(UNIFORM_X.knot(index), expected);
            assert_eq!(BUCKETED_X.knot(index), expected);
        }

        for (first, last) in [
            (LINEAR_X.first(), LINEAR_X.last()),
            (BINARY_X.first(), BINARY_X.last()),
            (UNIFORM_X.first(), UNIFORM_X.last()),
            (BUCKETED_X.first(), BUCKETED_X.last()),
        ] {
            assert_eq!((first, last), (100, 500));
        }
    }

    #[test]
    fn the_three_stored_knot_strategies_agree_on_an_irregular_axis() {
        // A uniform axis cannot describe this one, so the comparison is over the
        // three strategies that can.
        for coordinate in probes(&IRREGULAR_KNOTS, 211) {
            let expected = IRREGULAR_BINARY.search(coordinate).0;

            assert_eq!(
                IRREGULAR_LINEAR.search(coordinate).0,
                expected,
                "at {coordinate}"
            );
            assert_eq!(
                IRREGULAR_BUCKETED.search(coordinate).0,
                expected,
                "at {coordinate}"
            );
        }
    }

    /// The sixteen boundary selections, as bits: X-below, X-above, Y-below,
    /// Y-above, a set bit meaning [`Boundary::Clamp`].
    fn policy_from_bits(bits: usize) -> BoundaryPolicy {
        let side = |shift: u32| {
            if (bits >> shift) & 1 == 0 {
                Boundary::Error
            } else {
                Boundary::Clamp
            }
        };

        BoundaryPolicy::new()
            .with_x_below(side(0))
            .with_x_above(side(1))
            .with_y_below(side(2))
            .with_y_above(side(3))
    }

    #[test]
    fn every_pairing_evaluates_identically_under_every_policy() {
        // Sixteen pairings over one equivalent axis pair. The default binary
        // surface is the baseline every other pairing must reproduce, value for
        // value and error for error.
        for bits in 0..16 {
            let policy = policy_from_bits(bits);
            let baseline =
                BilinearSurface::from_axes(BINARY_X, BINARY_Y, &VALUES).with_policy(policy);

            macro_rules! agrees {
                ($x:expr, $y:expr) => {
                    let surface = BilinearSurface::from_axes($x, $y, &VALUES).with_policy(policy);
                    for x in uniform_probes() {
                        for y in [0u16, 9, 10, 11, 20, 25, 30, 31, 100, u16::MAX] {
                            assert_eq!(
                                surface.evaluate(x, y),
                                baseline.evaluate(x, y),
                                "bits {bits} at ({x}, {y})"
                            );
                        }
                    }
                };
            }

            agrees!(LINEAR_X, LINEAR_Y);
            agrees!(LINEAR_X, BINARY_Y);
            agrees!(LINEAR_X, UNIFORM_Y);
            agrees!(LINEAR_X, BUCKETED_Y);
            agrees!(BINARY_X, LINEAR_Y);
            agrees!(BINARY_X, BINARY_Y);
            agrees!(BINARY_X, UNIFORM_Y);
            agrees!(BINARY_X, BUCKETED_Y);
            agrees!(UNIFORM_X, LINEAR_Y);
            agrees!(UNIFORM_X, BINARY_Y);
            agrees!(UNIFORM_X, UNIFORM_Y);
            agrees!(UNIFORM_X, BUCKETED_Y);
            agrees!(BUCKETED_X, LINEAR_Y);
            agrees!(BUCKETED_X, BINARY_Y);
            agrees!(BUCKETED_X, UNIFORM_Y);
            agrees!(BUCKETED_X, BUCKETED_Y);
        }
    }

    #[test]
    fn a_mixed_pairing_reproduces_the_default_surface_on_an_irregular_axis() {
        let baseline = BilinearSurface::new(&IRREGULAR_KNOTS, &Y_KNOTS, &IRREGULAR_VALUES);
        let linear_bucketed =
            BilinearSurface::from_axes(IRREGULAR_LINEAR, BUCKETED_Y, &IRREGULAR_VALUES);
        let bucketed_uniform =
            BilinearSurface::from_axes(IRREGULAR_BUCKETED, UNIFORM_Y, &IRREGULAR_VALUES);

        let mut x = 3u16;
        loop {
            for y in [10u16, 15, 20, 25, 30] {
                let expected = baseline.evaluate(x, y);
                assert_eq!(linear_bucketed.evaluate(x, y), expected, "({x}, {y})");
                assert_eq!(bucketed_uniform.evaluate(x, y), expected, "({x}, {y})");
            }

            if x == u16::MAX {
                break;
            }
            x = x.saturating_add(197);
        }
    }

    #[test]
    fn the_four_error_variants_are_invariant_across_pairings() {
        let cases: [(u16, u16, SurfaceError); 4] = [
            (
                99,
                20,
                SurfaceError::XBelow {
                    coordinate: 99,
                    bound: 100,
                },
            ),
            (
                501,
                20,
                SurfaceError::XAbove {
                    coordinate: 501,
                    bound: 500,
                },
            ),
            (
                200,
                9,
                SurfaceError::YBelow {
                    coordinate: 9,
                    bound: 10,
                },
            ),
            (
                200,
                31,
                SurfaceError::YAbove {
                    coordinate: 31,
                    bound: 30,
                },
            ),
        ];

        for (x, y, expected) in cases {
            assert_eq!(
                BilinearSurface::from_axes(UNIFORM_X, BUCKETED_Y, &VALUES).evaluate(x, y),
                Err(expected)
            );
            assert_eq!(
                BilinearSurface::from_axes(BUCKETED_X, LINEAR_Y, &VALUES).evaluate(x, y),
                Err(expected)
            );
            assert_eq!(
                BilinearSurface::from_axes(LINEAR_X, UNIFORM_Y, &VALUES).evaluate(x, y),
                Err(expected)
            );
        }
    }

    #[test]
    fn x_before_y_precedence_is_invariant_across_pairings() {
        // Both coordinates out of domain on Error sides: the X-side error wins
        // whichever strategies located them.
        let expected = Err(SurfaceError::XBelow {
            coordinate: 99,
            bound: 100,
        });

        assert_eq!(
            BilinearSurface::from_axes(UNIFORM_X, UNIFORM_Y, &VALUES).evaluate(99, 9),
            expected
        );
        assert_eq!(
            BilinearSurface::from_axes(BUCKETED_X, LINEAR_Y, &VALUES).evaluate(99, 9),
            expected
        );

        // A clamped X still leaves the Y side free to reject.
        let clamped_x = BoundaryPolicy::new()
            .with_x_below(Boundary::Clamp)
            .with_x_above(Boundary::Clamp);
        assert_eq!(
            BilinearSurface::from_axes(LINEAR_X, BUCKETED_Y, &VALUES)
                .with_policy(clamped_x)
                .evaluate(99, 9),
            Err(SurfaceError::YBelow {
                coordinate: 9,
                bound: 10,
            })
        );
    }

    #[test]
    fn clamping_never_extrapolates_under_any_strategy() {
        let all_clamp = policy_from_bits(0b1111);
        let hull = VALUES
            .iter()
            .flat_map(|row| row.iter().copied())
            .fold((i32::MAX, i32::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));

        macro_rules! clamps_into_the_hull {
            ($x:expr, $y:expr) => {
                let surface = BilinearSurface::from_axes($x, $y, &VALUES).with_policy(all_clamp);
                for x in [0u16, 1, 99, 501, u16::MAX] {
                    for y in [0u16, 9, 31, u16::MAX] {
                        let value = surface.evaluate(x, y).expect("every side clamps");
                        assert!(
                            (hull.0..=hull.1).contains(&value),
                            "({x}, {y}) extrapolated to {value}"
                        );
                    }
                }
                // A clamped edge evaluates the boundary itself.
                assert_eq!(surface.evaluate(0, 20), surface.evaluate(100, 20));
                assert_eq!(surface.evaluate(u16::MAX, 20), surface.evaluate(500, 20));
                assert_eq!(surface.evaluate(200, 0), surface.evaluate(200, 10));
                assert_eq!(surface.evaluate(200, u16::MAX), surface.evaluate(200, 30));
            };
        }

        clamps_into_the_hull!(LINEAR_X, UNIFORM_Y);
        clamps_into_the_hull!(BINARY_X, BUCKETED_Y);
        clamps_into_the_hull!(UNIFORM_X, LINEAR_Y);
        clamps_into_the_hull!(BUCKETED_X, BINARY_Y);
    }

    #[test]
    fn every_declared_knot_returns_its_stored_value_under_every_pairing() {
        macro_rules! knots_are_exact {
            ($x:expr, $y:expr) => {
                let surface = BilinearSurface::from_axes($x, $y, &VALUES);
                for (row, &y) in Y_KNOTS.iter().enumerate() {
                    for (column, &x) in UNIFORM_KNOTS.iter().enumerate() {
                        assert_eq!(surface.evaluate(x, y), Ok(VALUES[row][column]));
                    }
                }
            };
        }

        knots_are_exact!(LINEAR_X, LINEAR_Y);
        knots_are_exact!(BINARY_X, UNIFORM_Y);
        knots_are_exact!(UNIFORM_X, BUCKETED_Y);
        knots_are_exact!(BUCKETED_X, BINARY_Y);
    }

    // The locked order fixture from the accepted contract, on a uniform axis so
    // that all four strategies can describe it.
    static ORDER_KNOTS: [u16; 2] = [0, 2];
    static ORDER_VALUES: [[i32; 2]; 2] = [[0, 0], [1, 3]];
    static ORDER_BUCKETS: [u16; 2] = bucket_index(&ORDER_KNOTS);

    const ORDER_LINEAR: LinearAxis<2> = LinearAxis::new(&ORDER_KNOTS);
    const ORDER_BINARY: BinaryAxis<2> = BinaryAxis::new(&ORDER_KNOTS);
    const ORDER_UNIFORM: UniformAxis<2, 0, 2> = UniformAxis::new();
    const ORDER_BUCKETED: BucketedAxis<2, 2> = BucketedAxis::new(&ORDER_KNOTS, &ORDER_BUCKETS);

    #[test]
    fn the_locked_order_fixture_still_distinguishes_x_then_y_under_every_pairing() {
        // X first gives 1; Y first would give 2. Every pairing must give 1, so
        // no strategy can have smuggled in a second composition order.
        macro_rules! keeps_the_order {
            ($x:expr, $y:expr) => {
                let surface = BilinearSurface::from_axes($x, $y, &ORDER_VALUES);
                assert_eq!(surface.evaluate(1, 1), Ok(1));
                assert_ne!(surface.evaluate(1, 1), Ok(2));
            };
        }

        keeps_the_order!(ORDER_LINEAR, ORDER_LINEAR);
        keeps_the_order!(ORDER_LINEAR, ORDER_BINARY);
        keeps_the_order!(ORDER_LINEAR, ORDER_UNIFORM);
        keeps_the_order!(ORDER_LINEAR, ORDER_BUCKETED);
        keeps_the_order!(ORDER_BINARY, ORDER_LINEAR);
        keeps_the_order!(ORDER_BINARY, ORDER_BINARY);
        keeps_the_order!(ORDER_BINARY, ORDER_UNIFORM);
        keeps_the_order!(ORDER_BINARY, ORDER_BUCKETED);
        keeps_the_order!(ORDER_UNIFORM, ORDER_LINEAR);
        keeps_the_order!(ORDER_UNIFORM, ORDER_BINARY);
        keeps_the_order!(ORDER_UNIFORM, ORDER_UNIFORM);
        keeps_the_order!(ORDER_UNIFORM, ORDER_BUCKETED);
        keeps_the_order!(ORDER_BUCKETED, ORDER_LINEAR);
        keeps_the_order!(ORDER_BUCKETED, ORDER_BINARY);
        keeps_the_order!(ORDER_BUCKETED, ORDER_UNIFORM);
        keeps_the_order!(ORDER_BUCKETED, ORDER_BUCKETED);
    }

    #[test]
    fn no_strategy_exceeds_its_declared_search_bound() {
        for coordinate in 100u16..=500 {
            assert!(LINEAR_X.search(coordinate).1 <= <LinearAxis<9>>::MAX_SEARCH_COMPARISONS);
            assert!(BINARY_X.search(coordinate).1 <= <BinaryAxis<9>>::MAX_SEARCH_COMPARISONS);
            assert!(
                BUCKETED_X.search(coordinate).1 <= <BucketedAxis<9, 4>>::MAX_SEARCH_COMPARISONS
            );
            // A bound of zero is a claim of its own: the uniform strategy must
            // reach the same cell without looking at a knot at all.
            assert_eq!(UNIFORM_X.search(coordinate).1, 0);
        }

        // The bounds themselves, so the numbers are pinned and not merely
        // self-consistent.
        assert_eq!(<LinearAxis<9>>::MAX_SEARCH_COMPARISONS, 8);
        assert_eq!(<BinaryAxis<9>>::MAX_SEARCH_COMPARISONS, 4);
        assert_eq!(<UniformAxis<9, 100, 50>>::MAX_SEARCH_COMPARISONS, 0);
        assert_eq!(<BucketedAxis<9, 4>>::MAX_SEARCH_COMPARISONS, 8);
    }

    #[test]
    fn stored_bytes_are_exactly_what_each_strategy_declares() {
        assert_eq!(<LinearAxis<9>>::KNOT_BYTES, 18);
        assert_eq!(<LinearAxis<9>>::INDEX_BYTES, 0);
        assert_eq!(<BinaryAxis<9>>::KNOT_BYTES, 18);
        assert_eq!(<BinaryAxis<9>>::INDEX_BYTES, 0);
        assert_eq!(<UniformAxis<9, 100, 50>>::KNOT_BYTES, 0);
        assert_eq!(<UniformAxis<9, 100, 50>>::INDEX_BYTES, 0);
        assert_eq!(<BucketedAxis<9, 4>>::KNOT_BYTES, 18);
        assert_eq!(<BucketedAxis<9, 4>>::INDEX_BYTES, 8);

        // The declared figures match the arrays actually referenced.
        assert_eq!(<LinearAxis<9>>::KNOT_BYTES, size_of::<[u16; 9]>());
        assert_eq!(<BucketedAxis<9, 4>>::INDEX_BYTES, size_of::<[u16; 4]>());
    }

    #[test]
    fn a_uniform_axis_occupies_no_storage_and_the_others_are_thin_handles() {
        // The descriptor lives in the type, so the value is zero-sized: a
        // uniform axis costs no static bytes and nothing in the handle either.
        assert_eq!(size_of::<UniformAxis<9, 100, 50>>(), 0);

        assert_eq!(size_of::<LinearAxis<9>>(), size_of::<usize>());
        assert_eq!(size_of::<BinaryAxis<9>>(), size_of::<usize>());
        assert_eq!(size_of::<BucketedAxis<9, 4>>(), 2 * size_of::<usize>());
    }

    #[test]
    fn a_strategy_is_a_type_and_never_a_runtime_discriminant() {
        // Each axis is exactly its stored references: there is no tag byte to
        // select an implementation with, on any of the four.
        assert_eq!(size_of::<LinearAxis<9>>(), size_of::<&'static [u16; 9]>());
        assert_eq!(size_of::<BinaryAxis<9>>(), size_of::<&'static [u16; 9]>());
        assert_eq!(
            size_of::<BucketedAxis<9, 4>>(),
            size_of::<&'static [u16; 9]>() + size_of::<&'static [u16; 4]>()
        );

        // And the default surface is unchanged by the existence of the other
        // three: same handle as a surface that names its strategies explicitly.
        assert_eq!(
            size_of::<BilinearSurface<9, 3>>(),
            size_of::<BilinearSurface<9, 3, BinaryAxis<9>, BinaryAxis<3>>>()
        );
    }

    #[test]
    fn the_default_surface_is_the_binary_pairing() {
        // Source compatibility, stated as a test: the defaulted type parameters
        // resolve to the binary strategy on both axes, and the two spellings
        // are the same type and the same value.
        let defaulted: BilinearSurface<9, 3> =
            BilinearSurface::new(&UNIFORM_KNOTS, &Y_KNOTS, &VALUES);
        let explicit: BilinearSurface<9, 3, BinaryAxis<9>, BinaryAxis<3>> =
            BilinearSurface::from_axes(BINARY_X, BINARY_Y, &VALUES);

        assert_eq!(defaulted, explicit);
    }
}

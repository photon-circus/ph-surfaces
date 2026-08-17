//! The bucketed strategy: stored knots plus a static bucket index that narrows
//! an irregular axis to a bounded local range before scanning it.

use super::{AxisLookup, KnotArray, assert_valid_knots, sealed};

/// Returns the bucket `coordinate` falls into on an axis spanning
/// `first ..= last`, partitioned into `B` buckets.
///
/// The partition is `((coordinate - first) * B) / (span + 1)`, which puts bucket
/// boundaries at `first + ceil(b * (span + 1) / B)`. Every intermediate fits a
/// `u32`: `span + 1` is at most `65_536`, `B` is at most `65_536`, and the
/// product `(coordinate - first) * B` is at most `65_535 * 65_536`, which is
/// `u32::MAX - 65_535`.
///
/// # Preconditions
///
/// `first <= coordinate <= last` and `1 <= B <= 65_536`, both established by
/// construction. The result is then always below `B`.
const fn bucket_of(coordinate: u16, first: u16, last: u16, b: usize) -> usize {
    let span = (last - first) as u32;
    let offset = (coordinate - first) as u32;

    (offset * (b as u32) / (span + 1)) as usize
}

/// Returns the first coordinate belonging to bucket `bucket`:
/// `first + ceil(bucket * (span + 1) / B)`.
///
/// This is the inverse of [`bucket_of`] at a boundary, and it is what makes the
/// partition *nested*: the boundaries for `B` buckets are exactly the
/// even-numbered boundaries for `2*B` buckets, so doubling `B` splits buckets
/// instead of moving them.
///
/// More buckets than the axis has coordinates is allowed and harmless: the
/// surplus buckets start past the last knot, no in-domain coordinate can select
/// one, and they are reported as starting at the last knot so that the
/// arithmetic stays inside `u16`.
///
/// # Preconditions
///
/// `bucket < B <= 65_536`, so `bucket * (span + 1) + B - 1` stays inside `u32`.
const fn bucket_start(bucket: usize, first: u16, last: u16, b: usize) -> u16 {
    let span = (last - first) as u32;
    let width = span + 1;
    let numerator = (bucket as u32) * width + (b as u32) - 1;
    let start = first as u32 + numerator / (b as u32);

    if start > last as u32 {
        last
    } else {
        start as u16
    }
}

/// Validates the dimensions shared by bucket-index generation and use.
const fn assert_valid_bucket_dimensions<const N: usize, const B: usize>(knots: &[u16; N]) {
    assert_valid_knots(knots);
    assert!(
        N <= 65_536,
        "a bucketed axis declares at most 65_536 knots, so every index fits a u16"
    );
    assert!(B >= 1, "a bucket index must declare at least one bucket");
    assert!(
        B <= 65_536,
        "a bucket index declares at most 65_536 buckets"
    );
}

/// Validates that `index` is the exact bucket table derived from `knots`.
const fn assert_valid_bucket_index<const N: usize, const B: usize>(
    knots: &[u16; N],
    index: &[u16; B],
) {
    assert_valid_bucket_dimensions::<N, B>(knots);

    let first = knots[0];
    let last = knots[N - 1];

    // Bucket starts are non-decreasing, so one knot cursor validates the whole
    // table in O(B + N) rather than restarting an O(N) scan for every bucket.
    let mut knot = 0;
    let mut bucket = 0;
    while bucket < B {
        let start = bucket_start(bucket, first, last, B);
        while knot + 1 < N && knots[knot + 1] <= start {
            knot += 1;
        }

        assert!(
            index[bucket] as usize == knot,
            "the bucket index does not match its knots; build it with bucket_index"
        );
        bucket += 1;
    }
}

/// Builds the bucket index for `knots`, for use as a `static`.
///
/// Entry `b` is the index of the greatest knot at or below the first coordinate
/// of bucket `b`, which is exactly what [`BucketedAxis::new`] validates and what
/// [`BucketedAxis`] starts its local scan from. The whole table is computed at
/// compile time: there is no runtime construction and nothing to allocate.
///
/// # Panics
///
/// Panics unless `N >= 2` and `N <= 65_536`, the knots are strictly increasing,
/// `B >= 1`, and `B <= 65_536`. In a `static` or `const` definition that panic
/// is a compile error.
///
/// # Examples
///
/// ```
/// use ph_surfaces::{BucketedAxis, bucket_index};
///
/// static KNOTS: [u16; 6] = [0, 1, 2, 3, 400, 1_000];
/// static INDEX: [u16; 4] = bucket_index(&KNOTS);
/// static AXIS: BucketedAxis<6, 4> = BucketedAxis::new(&KNOTS, &INDEX);
///
/// // Buckets start at 0, 251, 501 and 751; the knots at or below those are
/// // 3 (index 3), 400 (index 4), 400 and 400.
/// assert_eq!(INDEX, [0, 3, 4, 4]);
/// ```
#[must_use]
pub const fn bucket_index<const N: usize, const B: usize>(knots: &[u16; N]) -> [u16; B] {
    assert_valid_bucket_dimensions::<N, B>(knots);

    let first = knots[0];
    let last = knots[N - 1];

    let mut index = [0u16; B];
    let mut knot = 0;
    let mut bucket = 0;

    while bucket < B {
        let start = bucket_start(bucket, first, last, B);
        while knot + 1 < N && knots[knot + 1] <= start {
            knot += 1;
        }

        index[bucket] = knot as u16;
        bucket += 1;
    }

    index
}

/// Returns the exact worst-case local scan for one bucketed axis: the most knot
/// comparisons any in-domain search can perform after the bucket read.
///
/// Bucket `b` starts its scan at `index[b]` and cannot pass `index[b + 1]` (or
/// the last knot, for the final bucket). The scan compares once per knot it
/// steps towards, so its cost is exactly that difference. This function reports
/// the largest such cost over every bucket.
///
/// It is a function rather than an associated constant because the answer
/// depends on where the knots actually fall, not only on `N` and `B`.
/// [`AxisLookup::MAX_SEARCH_COMPARISONS`] states the structural bound that holds
/// for any knots; this states the exact one for these knots.
///
/// Because the partition is nested, raising `B` to a multiple of itself can only
/// split buckets, never move a boundary, so this figure never increases when `B`
/// grows that way.
///
/// # Panics
///
/// Panics unless `N >= 2` and `N <= 65_536`, the knots are strictly increasing,
/// `1 <= B <= 65_536`, and every entry of `index` equals the entry
/// [`bucket_index`] derives for the same knots.
///
/// # Examples
///
/// ```
/// use ph_surfaces::{bucket_index, max_local_comparisons};
///
/// // Four knots clustered at the bottom and one far away.
/// static KNOTS: [u16; 5] = [0, 1, 2, 3, 1_000];
/// static COARSE: [u16; 2] = bucket_index(&KNOTS);
/// static FINE: [u16; 8] = bucket_index(&KNOTS);
///
/// // A finer index cannot make the local scan longer.
/// assert!(max_local_comparisons(&KNOTS, &FINE) <= max_local_comparisons(&KNOTS, &COARSE));
/// ```
///
/// An invalid table is rejected during constant evaluation rather than
/// producing a profile-dependent bound at runtime:
///
/// ```compile_fail
/// use ph_surfaces::max_local_comparisons;
///
/// const KNOTS: [u16; 2] = [0, 10];
/// const DESCENDING: [u16; 2] = [1, 0];
/// const INVALID_BOUND: u32 = max_local_comparisons(&KNOTS, &DESCENDING);
/// ```
#[must_use]
pub const fn max_local_comparisons<const N: usize, const B: usize>(
    knots: &[u16; N],
    index: &[u16; B],
) -> u32 {
    assert_valid_bucket_index(knots, index);

    let mut worst = 0;
    let mut bucket = 0;

    while bucket < B {
        let start = index[bucket] as u32;
        let end = if bucket + 1 < B {
            index[bucket + 1] as u32
        } else {
            (N - 1) as u32
        };

        // `end >= start` because the exact index validation above establishes
        // the same non-decreasing table as `BucketedAxis::new`.
        let cost = end - start;
        if cost > worst {
            worst = cost;
        }

        bucket += 1;
    }

    worst
}

/// An axis of `N` stored knots with a static index of `B` buckets.
///
/// The index turns an irregular axis into a bounded local problem: one division
/// selects a bucket, the bucket names the first knot that can hold the answer,
/// and a short scan finishes the job. It is the strategy to reach for when an
/// axis is long and unevenly spaced and a few index bytes are worth a smaller
/// search bound; [`UniformAxis`](crate::UniformAxis) is better when the spacing
/// is regular, and [`BinaryAxis`](crate::BinaryAxis) when no extra bytes are
/// wanted.
///
/// The index is built at compile time by [`bucket_index`] and re-derived by
/// [`BucketedAxis::new`], so a stale or hand-written table fails to compile.
/// Nothing is constructed, cached, or mutated at runtime.
///
/// # Cost
///
/// `2*N` stored knot bytes plus `2*B` index bytes. After the endpoint checks,
/// the strategy reads one bucket and then performs at most
/// [`max_local_comparisons`] knot comparisons — a figure exact for these knots,
/// which never grows when `B` is raised to a multiple of itself.
///
/// # Examples
///
/// ```
/// use ph_surfaces::{BilinearSurface, BinaryAxis, BucketedAxis, bucket_index, max_local_comparisons};
///
/// // Tightly clustered at the bottom, then a long tail.
/// static X: [u16; 6] = [0, 1, 2, 3, 400, 1_000];
/// static X_INDEX: [u16; 8] = bucket_index(&X);
/// static Y: [u16; 2] = [0, 10];
/// static VALUES: [[i32; 6]; 2] = [[0, 1, 2, 3, 400, 1_000], [10, 11, 12, 13, 410, 1_010]];
///
/// static SURFACE: BilinearSurface<6, 2, BucketedAxis<6, 8>, BinaryAxis<2>> =
///     BilinearSurface::from_axes(BucketedAxis::new(&X, &X_INDEX), BinaryAxis::new(&Y), &VALUES);
///
/// // The same answers as the default binary surface over the same tables.
/// static DEFAULT: BilinearSurface<6, 2> = BilinearSurface::new(&X, &Y, &VALUES);
/// assert_eq!(SURFACE.evaluate(700, 5), DEFAULT.evaluate(700, 5));
///
/// // Three comparisons at worst, on an axis a plain scan could take five to
/// // walk: the index paid two bytes a bucket to bound the walk.
/// assert_eq!(max_local_comparisons(&X, &X_INDEX), 3);
/// ```
///
/// A bucket table that does not match its knots does not compile:
///
/// ```compile_fail
/// use ph_surfaces::BucketedAxis;
///
/// static X: [u16; 6] = [0, 1, 2, 3, 400, 1_000];
/// static WRONG: [u16; 4] = [0, 0, 0, 0];
/// static AXIS: BucketedAxis<6, 4> = BucketedAxis::new(&X, &WRONG);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BucketedAxis<const N: usize, const B: usize> {
    knots: &'static [u16; N],
    index: &'static [u16; B],
}

impl<const N: usize, const B: usize> BucketedAxis<N, B> {
    /// Declares a bucketed axis over static knots and a static bucket index.
    ///
    /// # Panics
    ///
    /// Panics unless the axis declares at least two strictly increasing knots,
    /// `N <= 65_536` so every knot index is representable in the table,
    /// `1 <= B <= 65_536`, and every entry of `index` equals the entry
    /// [`bucket_index`] derives for the same knots. In a constant or static
    /// definition that panic is a compile error, so an axis cannot reach runtime
    /// with an index that disagrees with its knots.
    #[must_use]
    pub const fn new(knots: &'static [u16; N], index: &'static [u16; B]) -> Self {
        assert_valid_bucket_index(knots, index);

        Self { knots, index }
    }

    /// Returns the declared knots.
    ///
    /// The same array as [`KnotArray::knots`], available in a constant context.
    #[must_use]
    pub const fn knots(&self) -> &'static [u16; N] {
        self.knots
    }

    /// Returns the declared bucket index.
    #[must_use]
    pub const fn index(&self) -> &'static [u16; B] {
        self.index
    }

    /// Returns the exact worst-case local scan for this axis, in knot
    /// comparisons: [`max_local_comparisons`] over its knots and index.
    #[must_use]
    pub const fn max_local_comparisons(&self) -> u32 {
        max_local_comparisons(self.knots, self.index)
    }
}

impl<const N: usize, const B: usize> sealed::Sealed<N> for BucketedAxis<N, B> {
    #[inline(always)]
    fn search_in_domain(&self, coordinate: u16) -> (usize, u32) {
        let first = self.knots[0];
        let last = self.knots[N - 1];

        debug_assert!(
            first <= coordinate && coordinate <= last,
            "the sealed search is only called on an in-domain coordinate"
        );

        let bucket = bucket_of(coordinate, first, last, B);
        debug_assert!(bucket < B, "the partition must not name a missing bucket");

        // The bucket names where the answer starts; the next bucket names where
        // it must have been found. The coordinate is below the next bucket's
        // first coordinate, so its knot cannot be above that bucket's entry.
        let mut index = self.index[bucket] as usize;
        let end = if bucket + 1 < B {
            self.index[bucket + 1] as usize
        } else {
            N - 1
        };
        let local_bound = (end - index) as u32;
        let mut comparisons = 0;

        while index < end {
            comparisons += 1;
            if self.knots[index + 1] > coordinate {
                break;
            }
            index += 1;
        }

        debug_assert!(
            comparisons <= local_bound,
            "the local scan must stay inside its selected bucket"
        );
        debug_assert!(
            self.knots[index] <= coordinate,
            "the located knot must not sit above the coordinate"
        );

        (index, comparisons)
    }
}

impl<const N: usize, const B: usize> KnotArray<N> for BucketedAxis<N, B> {
    fn knots(&self) -> &'static [u16; N] {
        self.knots
    }
}

impl<const N: usize, const B: usize> AxisLookup<N> for BucketedAxis<N, B> {
    const KNOT_BYTES: usize = 2 * N;
    const INDEX_BYTES: usize = 2 * B;
    // The structural bound, true for any knots: a bucket can hold the whole
    // axis, and then the local scan is the whole scan. The exact bound for a
    // particular axis is `BucketedAxis::max_local_comparisons`, which is what
    // the index is bought for.
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
    use super::{BucketedAxis, bucket_index, max_local_comparisons};
    use crate::axis::{AxisLookup, BinaryAxis, KnotArray, probes};

    // Clustered at the bottom with a long tail: the shape a bucket index is for.
    static CLUSTERED: [u16; 6] = [0, 1, 2, 3, 400, 1_000];
    static CLUSTERED_2: [u16; 2] = bucket_index(&CLUSTERED);
    static CLUSTERED_4: [u16; 4] = bucket_index(&CLUSTERED);
    static CLUSTERED_8: [u16; 8] = bucket_index(&CLUSTERED);
    static CLUSTERED_16: [u16; 16] = bucket_index(&CLUSTERED);

    // Two clusters at opposite ends of the full `u16` span.
    static SPREAD: [u16; 9] = [3, 4, 5, 1_000, 40_000, 65_000, 65_500, 65_530, 65_535];
    static SPREAD_4: [u16; 4] = bucket_index(&SPREAD);
    static SPREAD_8: [u16; 8] = bucket_index(&SPREAD);
    static SPREAD_16: [u16; 16] = bucket_index(&SPREAD);

    // The minimum axis and the minimum index.
    static TINY: [u16; 2] = [7, 9];
    static TINY_1: [u16; 1] = bucket_index(&TINY);

    // Far more buckets than the axis has coordinates, at the very top of the
    // `u16` range: the surplus buckets would start past `u16::MAX` if the
    // partition arithmetic did not stop at the last knot.
    static CROWDED: [u16; 2] = [65_533, 65_535];
    static CROWDED_8: [u16; 8] = bucket_index(&CROWDED);

    const CLUSTERED_AXIS: BucketedAxis<6, 8> = BucketedAxis::new(&CLUSTERED, &CLUSTERED_8);
    const SPREAD_AXIS: BucketedAxis<9, 8> = BucketedAxis::new(&SPREAD, &SPREAD_8);
    const TINY_AXIS: BucketedAxis<2, 1> = BucketedAxis::new(&TINY, &TINY_1);

    #[test]
    fn the_generated_index_names_the_knot_at_or_below_each_bucket_start() {
        // Buckets start at 0, 251, 501, 751; the greatest knot at or below each
        // is 3 (index 3), 400 (index 4), 400, 400.
        assert_eq!(CLUSTERED_4, [0, 3, 4, 4]);
        // With one bucket the index can only name the first knot.
        assert_eq!(TINY_1, [0]);
        // Non-decreasing on every fixture, which is what makes the local range
        // well formed.
        for index in [&CLUSTERED_8[..], &SPREAD_16[..]] {
            assert!(index.windows(2).all(|w| w[0] <= w[1]));
        }
    }

    #[test]
    fn a_bucketed_search_locates_the_same_index_as_the_binary_search() {
        macro_rules! agrees_with_binary {
            ($axis:expr, $knots:expr, $stride:expr) => {
                let axis = $axis;
                let binary = BinaryAxis::new($knots);

                for coordinate in probes($knots, $stride) {
                    assert_eq!(
                        axis.search(coordinate).0,
                        binary.search(coordinate).0,
                        "at {coordinate}"
                    );
                }
            };
        }

        agrees_with_binary!(CLUSTERED_AXIS, &CLUSTERED, 1);
        agrees_with_binary!(SPREAD_AXIS, &SPREAD, 97);
        agrees_with_binary!(TINY_AXIS, &TINY, 1);

        // Every bucket count on the same knots, so the partition itself is
        // exercised rather than one convenient value of `B`.
        agrees_with_binary!(BucketedAxis::new(&CLUSTERED, &CLUSTERED_2), &CLUSTERED, 1);
        agrees_with_binary!(BucketedAxis::new(&CLUSTERED, &CLUSTERED_4), &CLUSTERED, 1);
        agrees_with_binary!(BucketedAxis::new(&CLUSTERED, &CLUSTERED_16), &CLUSTERED, 1);
        agrees_with_binary!(BucketedAxis::new(&SPREAD, &SPREAD_4), &SPREAD, 97);
        agrees_with_binary!(BucketedAxis::new(&SPREAD, &SPREAD_16), &SPREAD, 97);
    }

    #[test]
    fn raising_the_bucket_count_never_worsens_the_local_bound() {
        let clustered = [
            max_local_comparisons(&CLUSTERED, &CLUSTERED_2),
            max_local_comparisons(&CLUSTERED, &CLUSTERED_4),
            max_local_comparisons(&CLUSTERED, &CLUSTERED_8),
            max_local_comparisons(&CLUSTERED, &CLUSTERED_16),
        ];
        assert!(
            clustered.windows(2).all(|w| w[1] <= w[0]),
            "nested bucket counts worsened the bound: {clustered:?}"
        );

        let spread = [
            max_local_comparisons(&SPREAD, &SPREAD_4),
            max_local_comparisons(&SPREAD, &SPREAD_8),
            max_local_comparisons(&SPREAD, &SPREAD_16),
        ];
        assert!(
            spread.windows(2).all(|w| w[1] <= w[0]),
            "nested bucket counts worsened the bound: {spread:?}"
        );

        // And the index actually buys something: the coarsest table on the
        // clustered axis is worse than the finest.
        assert!(clustered[3] < clustered[0]);
    }

    #[test]
    fn the_local_scan_stays_inside_the_exact_bound() {
        for axis in [CLUSTERED_AXIS] {
            let bound = axis.max_local_comparisons();
            for coordinate in axis.first()..=axis.last() {
                assert!(axis.search(coordinate).1 <= bound, "at {coordinate}");
            }
        }

        let bound = SPREAD_AXIS.max_local_comparisons();
        let mut coordinate = SPREAD_AXIS.first();
        while coordinate < SPREAD_AXIS.last() {
            assert!(SPREAD_AXIS.search(coordinate).1 <= bound, "at {coordinate}");
            coordinate = coordinate.saturating_add(101);
        }

        // A one-bucket index degenerates to a scan of the whole axis, and says
        // so rather than pretending otherwise.
        assert_eq!(max_local_comparisons(&TINY, &TINY_1), 1);
    }

    #[test]
    fn more_buckets_than_coordinates_stays_inside_u16_and_still_locates() {
        // Declaring this index at all is the arithmetic evidence: every bucket
        // start is computed during const evaluation, and one past `u16::MAX`
        // would fail the build rather than reach this assertion.
        let axis = BucketedAxis::new(&CROWDED, &CROWDED_8);
        let binary = BinaryAxis::new(&CROWDED);

        assert_eq!(CROWDED_8, [0, 0, 0, 1, 1, 1, 1, 1]);
        for coordinate in 65_533u16..=65_535 {
            assert_eq!(
                axis.search(coordinate).0,
                binary.search(coordinate).0,
                "at {coordinate}"
            );
        }
        assert_eq!(max_local_comparisons(&CROWDED, &CROWDED_8), 1);
    }

    #[test]
    fn a_bucket_index_costs_exactly_two_bytes_per_bucket() {
        assert_eq!(<BucketedAxis<6, 8>>::KNOT_BYTES, 12);
        assert_eq!(<BucketedAxis<6, 8>>::INDEX_BYTES, 16);
        assert_eq!(<BucketedAxis<9, 16>>::INDEX_BYTES, 32);
        assert_eq!(<BucketedAxis<2, 1>>::INDEX_BYTES, 2);
    }

    #[test]
    fn the_tables_are_referenced_and_never_copied() {
        assert!(core::ptr::eq(CLUSTERED_AXIS.knots(), &CLUSTERED));
        assert!(core::ptr::eq(KnotArray::knots(&CLUSTERED_AXIS), &CLUSTERED));
        assert!(core::ptr::eq(CLUSTERED_AXIS.index(), &CLUSTERED_8));
    }

    #[test]
    #[should_panic(expected = "the bucket index does not match its knots")]
    fn an_index_that_does_not_match_its_knots_is_rejected() {
        static WRONG: [u16; 4] = [0, 0, 0, 0];

        let _ = BucketedAxis::new(&CLUSTERED, &WRONG);
    }

    #[test]
    #[should_panic(expected = "the bucket index does not match its knots")]
    fn an_index_built_for_different_knots_is_rejected() {
        static OTHER: [u16; 6] = [0, 200, 400, 600, 800, 1_000];
        static OTHER_INDEX: [u16; 4] = bucket_index(&OTHER);

        // The table is internally valid, but not for these knots.
        let _ = BucketedAxis::new(&CLUSTERED, &OTHER_INDEX);
    }

    #[test]
    #[should_panic(expected = "an axis must declare at least two knots")]
    fn a_one_knot_axis_is_rejected() {
        static ONE: [u16; 1] = [3];
        static ONE_INDEX: [u16; 1] = [0];

        let _ = BucketedAxis::new(&ONE, &ONE_INDEX);
    }

    #[test]
    #[should_panic(expected = "a bucket index must declare at least one bucket")]
    fn an_empty_bucket_index_is_rejected() {
        static NONE: [u16; 0] = [];

        let _ = BucketedAxis::new(&CLUSTERED, &NONE);
    }
}

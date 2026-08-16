//! Private axis lookup: bracketing a coordinate in a validated axis and
//! applying the boundary policy declared for that side.
//!
//! One algorithm serves both axes. It works on `&[u16]` rather than on
//! `&[u16; NX]`, so it is compiled once instead of once per `NX` and once per
//! `NY`, and the two axes cannot drift apart. Axis identity re-enters only in
//! the two thin wrappers at the bottom of this module, which turn the neutral
//! [`Side`] the locator reports into the axis-specific [`SurfaceError`] variant
//! the caller sees.
//!
//! # Cost
//!
//! An out-of-domain coordinate is decided by one or two comparisons against the
//! endpoint knots and never searches. An in-domain coordinate costs those two
//! comparisons plus exactly `ceil(log2(len))` probes.
//!
//! The probe count depends only on the axis length: never on the stored knots
//! and never on the coordinate. The search takes the same number of steps for
//! an endpoint, for an exact interior knot, and for a point strictly inside a
//! segment. There is no early exit on an exact match, deliberately, because it
//! would trade a uniform and auditable step count for a data-dependent one.
//!
//! Everything here is core integer arithmetic over indices. There is no
//! allocation, no floating point, and no extrapolation path.

use crate::boundary::Boundary;
use crate::error::SurfaceError;
use crate::surface::BilinearSurface;

/// One resolved axis segment: the coordinate to interpolate at, and the index
/// of the segment's lower knot.
///
/// The upper knot is always `lower + 1`. Because `lower` is at most `len - 2`,
/// that index is inside the axis and inside every row of the value grid, so the
/// evaluator needs no bounds reasoning of its own.
///
/// `coordinate` is the coordinate *after* any clamping. For an in-domain
/// coordinate it is the caller's value unchanged, including at both inclusive
/// endpoints and at every exact interior knot. For a clamped coordinate it is
/// the endpoint knot itself, which is what makes a clamped lookup evaluate the
/// boundary value rather than project past it.
///
/// The two knot coordinates are deliberately not stored here. The evaluator
/// already holds the axis, and copying them into this type would create a
/// second thing to keep consistent with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Cell {
    coordinate: u16,
    lower: usize,
}

impl Cell {
    /// Returns the coordinate to interpolate at, after any clamping.
    pub(crate) const fn coordinate(&self) -> u16 {
        self.coordinate
    }

    /// Returns the index of the segment's lower knot, at most `len - 2`.
    pub(crate) const fn lower(&self) -> usize {
        self.lower
    }

    /// Returns the index of the segment's upper knot, always `lower + 1`.
    ///
    /// This cannot overflow and cannot leave the axis: `lower` is at most
    /// `len - 2`, and an axis is a validated static array whose length is a
    /// `usize`.
    pub(crate) const fn upper(&self) -> usize {
        self.lower + 1
    }
}

/// Which end of an axis a coordinate fell off.
///
/// The shared locator is axis-neutral so that X and Y run the same code. This
/// type carries the only axis-independent fact the locator learns. The two
/// wrappers below turn it into the correct one of the four public
/// [`SurfaceError`] variants. Widening this type to name axes would duplicate
/// the algorithm; dropping it would collapse four public outcomes into one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    /// The coordinate was below the first knot.
    Below,
    /// The coordinate was above the last knot.
    Above,
}

/// Returns the number of knot probes [`search`] performs on an axis of `len`
/// knots: `ceil(log2(len))`.
///
/// This is the bit length of `len - 1`, which is target-independent even though
/// `usize::BITS` is not.
///
/// It is an exact count rather than an upper bound, because the search window
/// shrinks by `size -> ceil(size / 2)` on both branches. A `debug_assert_eq!`
/// in [`search`] holds the implementation to it.
///
/// # Preconditions
///
/// `len >= 2`. Every axis this crate can construct satisfies it, because
/// `BilinearSurface::new` rejects a shorter one at definition time, so `len - 1`
/// cannot underflow.
const fn probe_bound(len: usize) -> u32 {
    debug_assert!(len >= 2, "an axis declares at least two knots");

    usize::BITS - (len - 1).leading_zeros()
}

/// Returns the number of knot comparisons a full in-domain lookup performs on
/// an axis of `len` knots: two endpoint comparisons plus [`probe_bound`] probes.
///
/// An out-of-domain coordinate costs strictly less and never searches: one
/// comparison below the domain, two above it.
///
/// For a 1024-knot axis this is 12. A linear scan of the same axis would cost
/// up to 1024 comparisons. That gap is what the focused tests measure, and it is
/// the documented logarithmic bound this module promises.
///
/// # Preconditions
///
/// `len >= 2`, as for [`probe_bound`].
const fn comparison_bound(len: usize) -> u32 {
    probe_bound(len) + 2
}

/// Returns the greatest index whose knot is at or below `coordinate`, together
/// with the number of knot comparisons it took.
///
/// # Preconditions
///
/// `axis` holds at least two strictly increasing knots, and
/// `axis[0] <= coordinate`. The first is established by `BilinearSurface::new`
/// at definition time and the second by the endpoint test in [`locate`], so both
/// are internal invariants rather than runtime inputs and are checked only by
/// `debug_assert!`. Because they make the answer set non-empty, the returned
/// index is always well defined and no downstream subtraction can underflow.
///
/// # Cost
///
/// Exactly `probe_bound(axis.len())` comparisons, independent of the stored
/// knots and of `coordinate`. The window shrinks by `size -> ceil(size / 2)`
/// whichever branch a probe takes, which is why the count is uniform and why
/// there is no early exit on an exact match.
///
/// The returned count exists so tests can distinguish this search from a full
/// axis scan. Callers discard it, and an optimizing build removes it.
fn search(axis: &[u16], coordinate: u16) -> (usize, u32) {
    debug_assert!(axis.len() >= 2, "an axis declares at least two knots");
    debug_assert!(
        axis[0] <= coordinate,
        "search is only called on a coordinate at or above the first knot"
    );

    let mut base = 0;
    let mut size = axis.len();
    let mut probes = 0;

    while size > 1 {
        let half = size / 2;
        let mid = base + half;

        // The answer stays in `base ..= base + size - 1`: a knot at or below the
        // coordinate moves the window's floor up to `mid`, and a knot above it
        // leaves the floor alone. Either way the window shrinks.
        if axis[mid] <= coordinate {
            base = mid;
        }

        size -= half;
        probes += 1;
    }

    debug_assert_eq!(
        probes,
        probe_bound(axis.len()),
        "the probe count must match the documented bound"
    );
    debug_assert!(
        axis[base] <= coordinate,
        "the located knot must not sit above the coordinate"
    );

    (base, probes)
}

/// Resolves `coordinate` against one axis under that axis's two boundary
/// selections, and reports the comparisons it took.
///
/// Below the first knot the `below` selection decides, and above the last knot
/// the `above` selection decides. The two sides never consult each other, which
/// is what makes the four sides of a surface independent.
///
/// A clamped coordinate resolves to the endpoint cell directly: cell `0` below
/// the domain and cell `len - 2` above it. Clamping therefore neither searches
/// nor produces an index the axis or the value grid cannot address, and the
/// substituted coordinate is the endpoint knot itself, so no branch of this
/// function can extrapolate.
///
/// The rejection is reported as a neutral [`Side`] so that this one function
/// serves both axes. Naming the axis is the caller's job.
///
/// # Preconditions
///
/// `axis` holds at least two strictly increasing knots, established by
/// `BilinearSurface::new` and checked only by `debug_assert!`.
fn locate(
    axis: &[u16],
    coordinate: u16,
    below: Boundary,
    above: Boundary,
) -> (Result<Cell, Side>, u32) {
    debug_assert!(axis.len() >= 2, "an axis declares at least two knots");

    // `len >= 2`, so `last >= 1` and `last - 1` is the first cell's index at
    // worst. Neither subtraction below can underflow.
    let last = axis.len() - 1;

    if coordinate < axis[0] {
        return match below {
            Boundary::Error => (Err(Side::Below), 1),
            Boundary::Clamp => (
                Ok(Cell {
                    coordinate: axis[0],
                    lower: 0,
                }),
                1,
            ),
        };
    }

    if coordinate > axis[last] {
        return match above {
            Boundary::Error => (Err(Side::Above), 2),
            Boundary::Clamp => (
                Ok(Cell {
                    coordinate: axis[last],
                    lower: last - 1,
                }),
                2,
            ),
        };
    }

    let (found, probes) = search(axis, coordinate);

    // The last knot has no segment of its own, so it shares the last cell. That
    // is also what makes an exact knot interpolate to its own stored value: the
    // offset into the cell is zero at the lower knot and the full span at the
    // upper one.
    let cell = Cell {
        coordinate,
        lower: found.min(last - 1),
    };

    // These are exactly the preconditions of the scalar interpolation helper.
    debug_assert!(cell.upper() < axis.len(), "the upper knot must be in range");
    debug_assert!(
        axis[cell.lower()] <= cell.coordinate(),
        "the coordinate must not sit below its cell"
    );
    debug_assert!(
        cell.coordinate() <= axis[cell.upper()],
        "the coordinate must not sit above its cell"
    );

    let comparisons = probes + 2;
    debug_assert_eq!(
        comparisons,
        comparison_bound(axis.len()),
        "the comparison count must match the documented bound"
    );

    (Ok(cell), comparisons)
}

impl<const NX: usize, const NY: usize> BilinearSurface<NX, NY> {
    /// Resolves an X coordinate to a cell of the X axis, under the X-below and
    /// X-above selections of this surface's policy.
    ///
    /// # Errors
    ///
    /// [`SurfaceError::XBelow`] or [`SurfaceError::XAbove`] when the coordinate
    /// leaves the X domain on a side selecting [`Boundary::Error`], carrying the
    /// coordinate as supplied and the applicable first or last X knot.
    ///
    /// This never reports a Y outcome. Composing the two axes, and deciding
    /// which error wins when both are out of domain, belongs to the evaluator.
    pub(crate) fn locate_x(&self, x: u16) -> Result<Cell, SurfaceError> {
        let policy = self.policy();
        let (outcome, _comparisons) = locate(self.x_axis(), x, policy.x_below(), policy.x_above());

        outcome.map_err(|side| match side {
            Side::Below => SurfaceError::XBelow {
                coordinate: x,
                bound: self.x_min(),
            },
            Side::Above => SurfaceError::XAbove {
                coordinate: x,
                bound: self.x_max(),
            },
        })
    }

    /// Resolves a Y coordinate to a cell of the Y axis, under the Y-below and
    /// Y-above selections of this surface's policy.
    ///
    /// # Errors
    ///
    /// [`SurfaceError::YBelow`] or [`SurfaceError::YAbove`] when the coordinate
    /// leaves the Y domain on a side selecting [`Boundary::Error`], carrying the
    /// coordinate as supplied and the applicable first or last Y knot.
    ///
    /// This never reports an X outcome, for the same reason [`Self::locate_x`]
    /// never reports a Y one.
    pub(crate) fn locate_y(&self, y: u16) -> Result<Cell, SurfaceError> {
        let policy = self.policy();
        let (outcome, _comparisons) = locate(self.y_axis(), y, policy.y_below(), policy.y_above());

        outcome.map_err(|side| match side {
            Side::Below => SurfaceError::YBelow {
                coordinate: y,
                bound: self.y_min(),
            },
            Side::Above => SurfaceError::YAbove {
                coordinate: y,
                bound: self.y_max(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Cell, Side, comparison_bound, locate, probe_bound, search};
    use crate::boundary::{Boundary, BoundaryPolicy};
    use crate::error::SurfaceError;
    use crate::interp::interpolate_segment;
    use crate::surface::BilinearSurface;

    // Every fixture except the full-span pair keeps its bounds strictly inside
    // `u16`, so that `min - 1` and `max + 1` are representable and the two
    // boundary sides can be probed one unit out.

    // Uniform, five knots.
    static X_MAIN: [u16; 5] = [10, 20, 30, 40, 50];
    // Nonuniform, four knots, steps of 1, 399, and 29_500.
    static Y_MAIN: [u16; 4] = [100, 101, 500, 30_000];
    static V_MAIN: [[i32; 5]; 4] = [
        [0, 1, 2, 3, 4],
        [10, 11, 12, 13, 14],
        [20, 21, 22, 23, 24],
        [30, 31, 32, 33, 34],
    ];
    static MAIN: BilinearSurface<5, 4> = BilinearSurface::new(&X_MAIN, &Y_MAIN, &V_MAIN);

    // Nonuniform on X, with unit steps next to a 25_000-unit step.
    static X_SPARSE: [u16; 6] = [3, 4, 5, 1_000, 40_000, 65_000];
    static Y_SPARSE: [u16; 2] = [12, 13];
    static V_SPARSE: [[i32; 6]; 2] = [[0; 6]; 2];
    static SPARSE: BilinearSurface<6, 2> = BilinearSurface::new(&X_SPARSE, &Y_SPARSE, &V_SPARSE);

    // The full `0..=u16::MAX` span on both axes: no coordinate is out of domain.
    static X_FULL: [u16; 4] = [0, 1, 32_768, 65_535];
    static Y_FULL: [u16; 2] = [0, 65_535];
    static V_FULL: [[i32; 4]; 2] = [[0; 4]; 2];
    static FULL: BilinearSurface<4, 2> = BilinearSurface::new(&X_FULL, &Y_FULL, &V_FULL);

    // The minimum legal axis on both sides: `len - 2 == 0`, so every cell index
    // is zero and any underflow in the clamp path would show up here.
    static X_TINY: [u16; 2] = [7, 9];
    static Y_TINY: [u16; 2] = [20_000, 20_001];
    static V_TINY: [[i32; 2]; 2] = [[1, 2], [3, 4]];
    static TINY: BilinearSurface<2, 2> = BilinearSurface::new(&X_TINY, &Y_TINY, &V_TINY);

    // One shared table on both axes: the same algorithm must produce the same
    // cell for X and Y while still producing axis-specific errors.
    static V_SAME: [[i32; 5]; 5] = [[0; 5]; 5];
    static SAME: BilinearSurface<5, 5> = BilinearSurface::new(&X_MAIN, &X_MAIN, &V_SAME);

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
    static V_BIG: [[i32; 1024]; 2] = [[0; 1024]; 2];
    static BIG: BilinearSurface<1024, 2> = BilinearSurface::new(&X_BIG, &Y_TINY, &V_BIG);

    const AXES: &[&[u16]] = &[
        &X_MAIN, &Y_MAIN, &X_SPARSE, &X_FULL, &Y_FULL, &X_TINY, &X_BIG,
    ];

    // Bit 0 selects X-below, bit 1 X-above, bit 2 Y-below, bit 3 Y-above; a set
    // bit means Clamp. Mirrors the helper in `src/surface.rs`.
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

    const fn all_clamp() -> BoundaryPolicy {
        policy_from_bits(0b1111)
    }

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
        assert_eq!(comparison_bound(1_024), 12);
    }

    #[test]
    fn the_search_probe_count_is_exact_and_data_independent() {
        for &axis in AXES {
            let expected = probe_bound(axis.len());

            for &knot in axis {
                assert_eq!(search(axis, knot).1, expected, "at knot {knot}");

                if knot > axis[0] {
                    assert_eq!(search(axis, knot - 1).1, expected);
                }
                if knot < u16::MAX {
                    assert_eq!(search(axis, knot + 1).1, expected);
                }
            }
        }

        // A sweep across one axis: the count never moves off the bound, which is
        // what "independent of the data" means.
        let mut coordinate = 0u16;
        while coordinate < 65_472 {
            assert_eq!(search(&X_BIG, coordinate).1, 10);
            coordinate += 397;
        }
    }

    #[test]
    fn binary_lookup_costs_far_fewer_comparisons_than_a_scan() {
        let (_, probes) = search(&X_BIG, 40_000);
        let scan = u32::try_from(X_BIG.len()).expect("the axis length fits in u32");

        assert_eq!(probes, 10);
        assert!(
            probes * 100 < scan,
            "{probes} probes is not two orders of magnitude below {scan}"
        );
    }

    #[test]
    fn a_full_lookup_costs_two_endpoint_comparisons_plus_the_probes() {
        let (outcome, comparisons) = locate(&X_BIG, 40_000, Boundary::Error, Boundary::Error);

        assert!(outcome.is_ok());
        assert_eq!(comparisons, comparison_bound(X_BIG.len()));
        assert_eq!(comparisons, 12);
    }

    #[test]
    fn an_out_of_domain_coordinate_never_searches() {
        for below in [Boundary::Error, Boundary::Clamp] {
            for above in [Boundary::Error, Boundary::Clamp] {
                assert_eq!(locate(&X_MAIN, 9, below, above).1, 1);
                assert_eq!(locate(&X_MAIN, 51, below, above).1, 2);

                // Two comparisons against twelve for an in-domain coordinate on
                // the same axis: the boundary path cannot be falling into the
                // search.
                assert_eq!(locate(&X_BIG, 65_500, below, above).1, 2);
            }
        }
    }

    #[test]
    fn both_endpoint_knots_resolve_exactly() {
        for &axis in AXES {
            let last = axis.len() - 1;

            let (outcome, _) = locate(axis, axis[0], Boundary::Error, Boundary::Error);
            assert_eq!(
                outcome,
                Ok(Cell {
                    coordinate: axis[0],
                    lower: 0,
                })
            );

            let (outcome, _) = locate(axis, axis[last], Boundary::Error, Boundary::Error);
            assert_eq!(
                outcome,
                Ok(Cell {
                    coordinate: axis[last],
                    lower: last - 1,
                })
            );
        }

        // On the minimum axis both endpoints share the one cell.
        assert_eq!(
            TINY.locate_x(7),
            Ok(Cell {
                coordinate: 7,
                lower: 0
            })
        );
        assert_eq!(
            TINY.locate_x(9),
            Ok(Cell {
                coordinate: 9,
                lower: 0
            })
        );
    }

    #[test]
    fn endpoints_are_accepted_under_every_policy() {
        for policy in POLICIES {
            let surface = MAIN.with_policy(policy);

            assert_eq!(
                surface.locate_x(10),
                Ok(Cell {
                    coordinate: 10,
                    lower: 0,
                })
            );
            assert_eq!(
                surface.locate_x(50),
                Ok(Cell {
                    coordinate: 50,
                    lower: 3,
                })
            );
            assert_eq!(
                surface.locate_y(100),
                Ok(Cell {
                    coordinate: 100,
                    lower: 0,
                })
            );
            assert_eq!(
                surface.locate_y(30_000),
                Ok(Cell {
                    coordinate: 30_000,
                    lower: 2,
                })
            );
        }
    }

    #[test]
    fn every_exact_knot_keeps_its_coordinate_and_names_its_own_cell() {
        for &axis in AXES {
            let last_cell = axis.len() - 2;

            for (index, &knot) in axis.iter().enumerate() {
                let (outcome, _) = locate(axis, knot, Boundary::Error, Boundary::Error);
                let cell = outcome.expect("a declared knot is inside the domain");

                assert_eq!(cell.coordinate(), knot, "knot {knot} was rewritten");
                assert_eq!(cell.lower(), index.min(last_cell));
                assert_eq!(cell.upper(), cell.lower() + 1);
            }
        }
    }

    #[test]
    fn an_exact_knot_cell_reproduces_the_stored_knot_value() {
        for (index, &knot) in X_MAIN.iter().enumerate() {
            let cell = MAIN.locate_x(knot).expect("a declared knot is in domain");
            let value = interpolate_segment(
                cell.coordinate(),
                X_MAIN[cell.lower()],
                X_MAIN[cell.upper()],
                V_MAIN[0][cell.lower()],
                V_MAIN[0][cell.upper()],
            );

            assert_eq!(value, V_MAIN[0][index], "at knot {knot}");
        }
    }

    #[test]
    fn x_below_error_reports_the_x_minimum() {
        assert_eq!(
            MAIN.locate_x(9),
            Err(SurfaceError::XBelow {
                coordinate: 9,
                bound: 10,
            })
        );
        assert_eq!(
            MAIN.locate_x(0),
            Err(SurfaceError::XBelow {
                coordinate: 0,
                bound: 10,
            })
        );
        assert_eq!(
            SPARSE.locate_x(2),
            Err(SurfaceError::XBelow {
                coordinate: 2,
                bound: 3,
            })
        );
        assert_eq!(
            TINY.locate_x(6),
            Err(SurfaceError::XBelow {
                coordinate: 6,
                bound: 7,
            })
        );
    }

    #[test]
    fn x_above_error_reports_the_x_maximum() {
        assert_eq!(
            MAIN.locate_x(51),
            Err(SurfaceError::XAbove {
                coordinate: 51,
                bound: 50,
            })
        );
        assert_eq!(
            MAIN.locate_x(u16::MAX),
            Err(SurfaceError::XAbove {
                coordinate: u16::MAX,
                bound: 50,
            })
        );
        assert_eq!(
            SPARSE.locate_x(65_001),
            Err(SurfaceError::XAbove {
                coordinate: 65_001,
                bound: 65_000,
            })
        );
        assert_eq!(
            TINY.locate_x(10),
            Err(SurfaceError::XAbove {
                coordinate: 10,
                bound: 9,
            })
        );
    }

    #[test]
    fn y_below_error_reports_the_y_minimum() {
        assert_eq!(
            MAIN.locate_y(99),
            Err(SurfaceError::YBelow {
                coordinate: 99,
                bound: 100,
            })
        );
        assert_eq!(
            MAIN.locate_y(0),
            Err(SurfaceError::YBelow {
                coordinate: 0,
                bound: 100,
            })
        );
        assert_eq!(
            TINY.locate_y(19_999),
            Err(SurfaceError::YBelow {
                coordinate: 19_999,
                bound: 20_000,
            })
        );
    }

    #[test]
    fn y_above_error_reports_the_y_maximum() {
        assert_eq!(
            MAIN.locate_y(30_001),
            Err(SurfaceError::YAbove {
                coordinate: 30_001,
                bound: 30_000,
            })
        );
        assert_eq!(
            MAIN.locate_y(u16::MAX),
            Err(SurfaceError::YAbove {
                coordinate: u16::MAX,
                bound: 30_000,
            })
        );
        assert_eq!(
            TINY.locate_y(20_002),
            Err(SurfaceError::YAbove {
                coordinate: 20_002,
                bound: 20_001,
            })
        );
    }

    #[test]
    fn x_below_clamp_resolves_to_the_first_cell() {
        let clamped = MAIN.with_policy(all_clamp());

        assert_eq!(
            clamped.locate_x(9),
            Ok(Cell {
                coordinate: 10,
                lower: 0,
            })
        );
        assert_eq!(
            clamped.locate_x(0),
            Ok(Cell {
                coordinate: 10,
                lower: 0,
            })
        );
        assert_eq!(
            TINY.with_policy(all_clamp()).locate_x(0),
            Ok(Cell {
                coordinate: 7,
                lower: 0,
            })
        );
    }

    #[test]
    fn x_above_clamp_resolves_to_the_last_cell() {
        let clamped = MAIN.with_policy(all_clamp());

        assert_eq!(
            clamped.locate_x(51),
            Ok(Cell {
                coordinate: 50,
                lower: 3,
            })
        );
        assert_eq!(
            clamped.locate_x(u16::MAX),
            Ok(Cell {
                coordinate: 50,
                lower: 3,
            })
        );
        assert_eq!(
            TINY.with_policy(all_clamp()).locate_x(u16::MAX),
            Ok(Cell {
                coordinate: 9,
                lower: 0,
            })
        );
    }

    #[test]
    fn y_below_clamp_resolves_to_the_first_cell() {
        let clamped = MAIN.with_policy(all_clamp());

        assert_eq!(
            clamped.locate_y(99),
            Ok(Cell {
                coordinate: 100,
                lower: 0,
            })
        );
        assert_eq!(
            TINY.with_policy(all_clamp()).locate_y(0),
            Ok(Cell {
                coordinate: 20_000,
                lower: 0,
            })
        );
    }

    #[test]
    fn y_above_clamp_resolves_to_the_last_cell() {
        let clamped = MAIN.with_policy(all_clamp());

        assert_eq!(
            clamped.locate_y(30_001),
            Ok(Cell {
                coordinate: 30_000,
                lower: 2,
            })
        );
        assert_eq!(
            clamped.locate_y(u16::MAX),
            Ok(Cell {
                coordinate: 30_000,
                lower: 2,
            })
        );
        assert_eq!(
            TINY.with_policy(all_clamp()).locate_y(u16::MAX),
            Ok(Cell {
                coordinate: 20_001,
                lower: 0,
            })
        );
    }

    #[test]
    fn each_side_selects_its_own_policy() {
        for (bits, &policy) in POLICIES.iter().enumerate() {
            let surface = MAIN.with_policy(policy);

            assert_eq!(
                surface.locate_x(9).is_ok(),
                bits & 0b0001 != 0,
                "bits {bits}"
            );
            assert_eq!(
                surface.locate_x(51).is_ok(),
                bits & 0b0010 != 0,
                "bits {bits}"
            );
            assert_eq!(
                surface.locate_y(99).is_ok(),
                bits & 0b0100 != 0,
                "bits {bits}"
            );
            assert_eq!(
                surface.locate_y(30_001).is_ok(),
                bits & 0b1000 != 0,
                "bits {bits}"
            );
        }
    }

    #[test]
    fn an_x_policy_never_changes_a_y_outcome() {
        let surface = MAIN.with_policy(BoundaryPolicy::new().with_x_below(Boundary::Clamp));

        assert_eq!(
            surface.locate_x(9),
            Ok(Cell {
                coordinate: 10,
                lower: 0,
            })
        );
        assert_eq!(
            surface.locate_x(51),
            Err(SurfaceError::XAbove {
                coordinate: 51,
                bound: 50,
            })
        );
        assert_eq!(
            surface.locate_y(99),
            Err(SurfaceError::YBelow {
                coordinate: 99,
                bound: 100,
            })
        );
    }

    #[test]
    fn both_axes_run_the_same_algorithm_with_distinct_errors() {
        for coordinate in [10u16, 11, 25, 30, 49, 50] {
            assert_eq!(SAME.locate_x(coordinate), SAME.locate_y(coordinate));
        }

        let x = SAME.locate_x(9).expect_err("9 is below the shared axis");
        let y = SAME.locate_y(9).expect_err("9 is below the shared axis");

        assert_eq!(
            x,
            SurfaceError::XBelow {
                coordinate: 9,
                bound: 10,
            }
        );
        assert_eq!(
            y,
            SurfaceError::YBelow {
                coordinate: 9,
                bound: 10,
            }
        );
        assert_ne!(x, y);
        assert_eq!(x.coordinate(), y.coordinate());
        assert_eq!(x.bound(), y.bound());
    }

    #[test]
    fn nonuniform_axes_bracket_each_unequal_segment() {
        let expected: &[(u16, usize)] = &[
            (3, 0),
            (4, 1),
            (5, 2),
            (6, 2),
            (999, 2),
            (1_000, 3),
            (39_999, 3),
            (40_000, 4),
            (64_999, 4),
            (65_000, 4),
        ];

        for &(coordinate, lower) in expected {
            let cell = SPARSE.locate_x(coordinate).expect("in domain");

            assert_eq!(cell.lower(), lower, "at {coordinate}");
            assert_eq!(cell.coordinate(), coordinate);
        }

        let expected: &[(u16, usize)] = &[
            (100, 0),
            (101, 1),
            (300, 1),
            (499, 1),
            (500, 2),
            (29_999, 2),
            (30_000, 2),
        ];

        for &(coordinate, lower) in expected {
            let cell = MAIN.locate_y(coordinate).expect("in domain");

            assert_eq!(cell.lower(), lower, "at {coordinate}");
            assert_eq!(cell.coordinate(), coordinate);
        }
    }

    #[test]
    fn a_full_span_axis_has_no_out_of_domain_coordinate() {
        for coordinate in [0u16, 1, 2, 32_767, 32_768, 32_769, 65_534, 65_535] {
            assert!(FULL.locate_x(coordinate).is_ok(), "x {coordinate}");
            assert!(FULL.locate_y(coordinate).is_ok(), "y {coordinate}");
        }

        let mut coordinate = 0u16;
        loop {
            assert!(FULL.locate_x(coordinate).is_ok());
            assert!(FULL.locate_y(coordinate).is_ok());

            if coordinate > u16::MAX - 997 {
                break;
            }
            coordinate += 997;
        }

        assert_eq!(
            FULL.locate_x(65_535),
            Ok(Cell {
                coordinate: 65_535,
                lower: 2,
            })
        );
        assert_eq!(
            FULL.locate_y(65_535),
            Ok(Cell {
                coordinate: 65_535,
                lower: 0,
            })
        );
    }

    #[test]
    fn the_widest_and_narrowest_segments_both_resolve() {
        // The widest declared segments: 32_767 units and the whole u16 span.
        assert_eq!(FULL.locate_x(16_384).expect("in domain").lower(), 1);
        assert_eq!(FULL.locate_y(32_768).expect("in domain").lower(), 0);

        // The narrowest possible segment: one unit, so only its endpoints exist.
        assert_eq!(SPARSE.locate_x(4).expect("in domain").lower(), 1);
        assert_eq!(TINY.locate_y(20_001).expect("in domain").lower(), 0);
    }

    // Generic so the same checks run over fixtures of different shapes.
    fn clamped_cells_stay_in_range<const NX: usize, const NY: usize>(
        surface: &BilinearSurface<NX, NY>,
    ) {
        let clamped = surface.with_policy(all_clamp());

        for coordinate in [0u16, 1, 32_768, u16::MAX - 1, u16::MAX] {
            let x = clamped
                .locate_x(coordinate)
                .expect("clamping never rejects");
            let y = clamped
                .locate_y(coordinate)
                .expect("clamping never rejects");

            assert_eq!(x.upper(), x.lower() + 1);
            assert_eq!(y.upper(), y.lower() + 1);
            assert!(x.upper() < surface.nx());
            assert!(y.upper() < surface.ny());
            assert!(x.coordinate() >= surface.x_min() && x.coordinate() <= surface.x_max());
            assert!(y.coordinate() >= surface.y_min() && y.coordinate() <= surface.y_max());

            // Index every corner the evaluator would touch. A cell that could
            // run off either axis or off the grid would panic here.
            let _ = surface.x_axis()[x.upper()];
            let _ = surface.y_axis()[y.upper()];
            let _ = surface.values()[y.lower()][x.lower()];
            let _ = surface.values()[y.upper()][x.upper()];
        }
    }

    #[test]
    fn clamped_cells_stay_inside_the_axis_and_the_value_grid() {
        clamped_cells_stay_in_range(&MAIN);
        clamped_cells_stay_in_range(&SPARSE);
        clamped_cells_stay_in_range(&FULL);
        clamped_cells_stay_in_range(&TINY);
        clamped_cells_stay_in_range(&SAME);
        clamped_cells_stay_in_range(&BIG);
    }

    #[test]
    fn clamping_never_extrapolates() {
        let clamped = MAIN.with_policy(all_clamp());
        let last = X_MAIN.len() - 1;

        let cell = clamped.locate_x(u16::MAX).expect("clamping never rejects");
        assert_eq!(cell.coordinate(), MAIN.x_max());
        assert_eq!(
            interpolate_segment(
                cell.coordinate(),
                X_MAIN[cell.lower()],
                X_MAIN[cell.upper()],
                V_MAIN[0][cell.lower()],
                V_MAIN[0][cell.upper()],
            ),
            V_MAIN[0][last]
        );

        let cell = clamped.locate_x(0).expect("clamping never rejects");
        assert_eq!(cell.coordinate(), MAIN.x_min());
        assert_eq!(
            interpolate_segment(
                cell.coordinate(),
                X_MAIN[cell.lower()],
                X_MAIN[cell.upper()],
                V_MAIN[0][cell.lower()],
                V_MAIN[0][cell.upper()],
            ),
            V_MAIN[0][0]
        );
    }

    #[test]
    fn a_rejected_coordinate_is_reported_as_a_neutral_side() {
        assert_eq!(
            locate(&X_MAIN, 9, Boundary::Error, Boundary::Error).0,
            Err(Side::Below)
        );
        assert_eq!(
            locate(&X_MAIN, 51, Boundary::Error, Boundary::Error).0,
            Err(Side::Above)
        );
        assert_ne!(Side::Below, Side::Above);
    }
}

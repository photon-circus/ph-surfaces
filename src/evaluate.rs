//! The public evaluator: composing axis lookup and scalar interpolation into
//! one deterministic two-dimensional value.
//!
//! This module owns the composition order of the crate. It is the only place
//! that decides which axis is resolved first, which error wins when both are
//! out of domain, and in which order the three scalar interpolations run. It
//! introduces no arithmetic and no search of its own: rounding lives in
//! [`crate::interp`] and axis location lives in [`crate::lookup`].
//!
//! # Why the order is part of the contract
//!
//! Each scalar step rounds to an `i32` before the next one runs, so composing
//! X first and composing Y first are observably different functions rather than
//! two spellings of the same one. The crate therefore fixes one order and makes
//! it normative rather than offering a choice.

use crate::axis::AxisLookup;
use crate::error::SurfaceError;
use crate::interp::interpolate_segment;
use crate::lookup::Cell;
use crate::surface::BilinearSurface;

impl<const NX: usize, const NY: usize, X: AxisLookup<NX>, Y: AxisLookup<NY>>
    BilinearSurface<NX, NY, X, Y>
{
    /// Evaluates this surface at `(x, y)` with deterministic X-then-Y bilinear
    /// interpolation.
    ///
    /// # Order
    ///
    /// The composition is normative, not an implementation detail:
    ///
    /// 1. interpolate along X on the lower-Y row;
    /// 2. interpolate along X on the upper-Y row;
    /// 3. interpolate those two *already rounded* results along Y.
    ///
    /// Because every step rounds to nearest with exact half-way values away
    /// from zero, a Y-then-X implementation would return different values, so
    /// this order is observable. For the axes `[0, 2]` with rows
    /// `[[0, 0], [1, 3]]`, this order returns `1` at `(1, 1)` where Y-then-X
    /// would return `2`:
    ///
    /// ```
    /// use ph_surfaces::BilinearSurface;
    ///
    /// static AXIS: [u16; 2] = [0, 2];
    /// static VALUES: [[i32; 2]; 2] = [[0, 0], [1, 3]];
    /// static SURFACE: BilinearSurface<2, 2> = BilinearSurface::new(&AXIS, &AXIS, &VALUES);
    ///
    /// // X on the lower row: 0. X on the upper row: (1 + 3) / 2 = 2.
    /// // Y between them: (0 + 2) / 2 = 1.
    /// assert_eq!(SURFACE.evaluate(1, 1), Ok(1));
    /// ```
    ///
    /// # Domain
    ///
    /// X is resolved before Y. If both coordinates leave the domain on sides
    /// selecting [`Boundary::Error`](crate::Boundary::Error), the X-side error
    /// is the one reported. If the X side clamps, Y is still resolved under its
    /// own two selections, so a clamped X can be followed by a Y error.
    ///
    /// A clamped coordinate is replaced by the nearest declared endpoint knot
    /// and then evaluated through this same path. Nothing extrapolates: the
    /// result of a clamped evaluation is a value the surface actually declares
    /// on its boundary.
    ///
    /// ```
    /// use ph_surfaces::{BilinearSurface, Boundary, BoundaryPolicy};
    ///
    /// static X: [u16; 2] = [0, 10];
    /// static Y: [u16; 2] = [0, 10];
    /// static VALUES: [[i32; 2]; 2] = [[0, 100], [200, 300]];
    ///
    /// static SURFACE: BilinearSurface<2, 2> = BilinearSurface::new(&X, &Y, &VALUES)
    ///     .with_policy(BoundaryPolicy::new().with_x_above(Boundary::Clamp));
    ///
    /// // X clamps to 10 and evaluates the boundary column, never past it.
    /// assert_eq!(SURFACE.evaluate(4_000, 0), Ok(100));
    ///
    /// // Y still errors on its own side, even though X clamped.
    /// assert_eq!(
    ///     SURFACE.evaluate(4_000, 11),
    ///     Err(ph_surfaces::SurfaceError::YAbove { coordinate: 11, bound: 10 }),
    /// );
    /// ```
    ///
    /// # Errors
    ///
    /// Returns the [`SurfaceError`] variant naming the side the coordinate fell
    /// off, carrying the coordinate as supplied and the applicable first or
    /// last knot of that axis. Only a side selecting
    /// [`Boundary::Error`](crate::Boundary::Error) can produce one.
    ///
    /// # Cost
    ///
    /// A successful evaluation performs exactly three scalar interpolations and
    /// exactly four reads of the value grid. Each in-domain axis lookup costs
    /// two endpoint comparisons plus the search work of that axis's strategy —
    /// `ceil(log2(len))` probes for the default
    /// [`BinaryAxis`](crate::BinaryAxis), and at most
    /// [`AxisLookup::MAX_SEARCH_COMPARISONS`](crate::AxisLookup::MAX_SEARCH_COMPARISONS)
    /// comparisons for any of them; a clamped lookup
    /// costs one or two endpoint comparisons and performs no probes. A rejected
    /// coordinate returns before any interpolation or value-grid read, and an X
    /// rejection also skips the Y lookup because X is resolved first.
    ///
    /// The value grid is never scanned and its size affects only in-domain
    /// lookup cost. Evaluation allocates nothing, keeps no state, and has no
    /// warm-up, reset, cache, or lifecycle behaviour: the same handle and the
    /// same coordinates always produce the same result.
    ///
    /// The arithmetic cannot overflow for any surface this crate can define.
    /// That is the bound proven for the private scalar helper: both weights are
    /// nonnegative and sum to a span of at most `65_535`, and each rounded
    /// result stays inside the convex hull of its two endpoints. The Y step
    /// therefore receives two `i32` values drawn from the hull of the four
    /// corner values and returns one from the same hull, so there is no
    /// overflow outcome to report.
    ///
    /// # Examples
    ///
    /// ```
    /// use ph_surfaces::{BilinearSurface, SurfaceError};
    ///
    /// static X: [u16; 3] = [0, 10, 30];
    /// static Y: [u16; 2] = [0, 100];
    /// static VALUES: [[i32; 3]; 2] = [[0, 10, 30], [100, 110, 130]];
    ///
    /// static SURFACE: BilinearSurface<3, 2> = BilinearSurface::new(&X, &Y, &VALUES);
    ///
    /// // A declared knot returns its stored value exactly.
    /// assert_eq!(SURFACE.evaluate(10, 100), Ok(110));
    ///
    /// // An interior point of the plane.
    /// assert_eq!(SURFACE.evaluate(20, 50), Ok(70));
    ///
    /// // Out of domain on the default Error policy.
    /// assert_eq!(
    ///     SURFACE.evaluate(31, 0),
    ///     Err(SurfaceError::XAbove { coordinate: 31, bound: 30 }),
    /// );
    /// ```
    pub fn evaluate(&self, x: u16, y: u16) -> Result<i32, SurfaceError> {
        // X is resolved first, and this `?` is the whole precedence rule: an
        // X-side Error returns before Y is ever consulted. An X-side Clamp
        // yields a cell instead, so Y is then resolved normally under its own
        // two selections.
        let x_cell = self.locate_x(x)?;
        let y_cell = self.locate_y(y)?;

        // Both rows of the located cell. `Cell::upper` is `lower + 1` with
        // `lower` at most `len - 2`, so every index below is inside its array
        // and no bounds reasoning is repeated here.
        let lower_row = &self.values()[y_cell.lower()];
        let upper_row = &self.values()[y_cell.upper()];

        let at_lower_y = self.interpolate_row(x_cell, lower_row);
        let at_upper_y = self.interpolate_row(x_cell, upper_row);

        // Step three: interpolate the two already-rounded row results along Y.
        Ok(interpolate_segment(
            y_cell.coordinate(),
            self.y().knot(y_cell.lower()),
            self.y().knot(y_cell.upper()),
            at_lower_y,
            at_upper_y,
        ))
    }

    /// Interpolates one row of the value grid along X, at the coordinate the X
    /// cell resolved to.
    ///
    /// Both X steps of the composition are this same function, so the lower-Y
    /// and upper-Y rows cannot be treated differently by accident.
    fn interpolate_row(&self, x_cell: Cell, row: &[i32; NX]) -> i32 {
        interpolate_segment(
            x_cell.coordinate(),
            self.x().knot(x_cell.lower()),
            self.x().knot(x_cell.upper()),
            row[x_cell.lower()],
            row[x_cell.upper()],
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::boundary::{Boundary, BoundaryPolicy};
    use crate::error::SurfaceError;
    use crate::surface::BilinearSurface;

    // On "no evaluator path scans the value grid, allocates, uses floating
    // point, uses unsafe, or calls `ph-curves`": a unit test cannot observe the
    // absence of a code path. That evidence is mechanical, as it is for
    // `src/interp.rs` — `#![forbid(unsafe_code)]` in the crate root, the
    // `integer only` grep and the `-Z build-std=core` build in `scripts/ci.sh`,
    // and the `deny.toml` ban on the crate name. The absence of a scan is
    // structural: `evaluate` reads exactly the four corner values of one
    // located cell, and the only search in the crate is the counted binary
    // lookup whose exact comparison bound `src/lookup.rs` asserts. The tests
    // below carry the numerical and boundary contract instead.

    // The locked order fixture from the accepted contract.
    static ORDER_AXIS: [u16; 2] = [0, 2];
    static ORDER_VALUES: [[i32; 2]; 2] = [[0, 0], [1, 3]];
    static ORDER: BilinearSurface<2, 2> =
        BilinearSurface::new(&ORDER_AXIS, &ORDER_AXIS, &ORDER_VALUES);

    // A hand-computable plane: value = 10 * x + 100 * y over the axes below.
    static PLANE_X: [u16; 2] = [0, 10];
    static PLANE_Y: [u16; 2] = [0, 10];
    static PLANE_VALUES: [[i32; 2]; 2] = [[0, 100], [1_000, 1_100]];
    static PLANE: BilinearSurface<2, 2> = BilinearSurface::new(&PLANE_X, &PLANE_Y, &PLANE_VALUES);

    // A nonuniform 3x3 grid with an asymmetric step on both axes.
    static GRID_X: [u16; 3] = [0, 10, 110];
    static GRID_Y: [u16; 3] = [0, 4, 8];
    static GRID_VALUES: [[i32; 3]; 3] = [[0, 100, 300], [10, 110, 310], [-40, 60, 260]];
    static GRID: BilinearSurface<3, 3> = BilinearSurface::new(&GRID_X, &GRID_Y, &GRID_VALUES);

    // Rows chosen for their shapes: positive increasing, negative decreasing,
    // flat, and zero-crossing.
    static SHAPE_X: [u16; 3] = [0, 100, 200];
    static SHAPE_Y: [u16; 4] = [0, 10, 20, 30];
    static SHAPE_VALUES: [[i32; 3]; 4] = [
        [10, 20, 30],    // positive, increasing
        [-10, -20, -30], // negative, decreasing
        [7, 7, 7],       // flat
        [-100, 0, 100],  // zero-crossing
    ];
    static SHAPES: BilinearSurface<3, 4> = BilinearSurface::new(&SHAPE_X, &SHAPE_Y, &SHAPE_VALUES);

    // The full `u16` span on both axes with the extreme `i32` corners: the
    // widest operands a v0.1 surface can present to the arithmetic.
    static FULL_AXIS: [u16; 2] = [0, u16::MAX];
    static EXTREME_VALUES: [[i32; 2]; 2] = [[i32::MIN, i32::MAX], [i32::MAX, i32::MIN]];
    static EXTREME: BilinearSurface<2, 2> =
        BilinearSurface::new(&FULL_AXIS, &FULL_AXIS, &EXTREME_VALUES);

    // A surface whose domain has room on every side, so each of the four sides
    // can be probed one unit out.
    static INSET_X: [u16; 3] = [10, 20, 40];
    static INSET_Y: [u16; 3] = [100, 200, 400];
    static INSET_VALUES: [[i32; 3]; 3] = [[0, 1, 2], [10, 11, 12], [20, 21, 22]];

    const fn inset(policy: BoundaryPolicy) -> BilinearSurface<3, 3> {
        BilinearSurface::new(&INSET_X, &INSET_Y, &INSET_VALUES).with_policy(policy)
    }

    static INSET: BilinearSurface<3, 3> = inset(BoundaryPolicy::new());

    /// Interpolates one segment under the crate's rounding policy, written out
    /// here so the reference below shares no code with the implementation.
    fn segment(t: u16, t0: u16, t1: u16, v0: i32, v1: i32) -> i32 {
        let span = i64::from(t1) - i64::from(t0);
        let offset = i64::from(t) - i64::from(t0);
        let numerator = i64::from(v0) * (span - offset) + i64::from(v1) * offset;
        let half = span / 2;

        let rounded = if numerator >= 0 {
            (numerator + half) / span
        } else {
            (numerator - half) / span
        };

        rounded as i32
    }

    /// Independent reference for the *rejected* composition order: Y first on
    /// each column, then X across the two rounded results.
    ///
    /// It exists so the order fixture is shown to discriminate between the two
    /// orders rather than merely to agree with one of them.
    fn y_then_x(surface: &BilinearSurface<2, 2>, x: u16, y: u16) -> i32 {
        let axis_x = surface.x_axis();
        let axis_y = surface.y_axis();
        let values = surface.values();

        let column =
            |index: usize| segment(y, axis_y[0], axis_y[1], values[0][index], values[1][index]);

        segment(x, axis_x[0], axis_x[1], column(0), column(1))
    }

    #[test]
    fn the_locked_order_fixture_returns_the_x_then_y_value() {
        assert_eq!(ORDER.evaluate(1, 1), Ok(1));
    }

    #[test]
    fn the_locked_order_fixture_discriminates_between_the_two_orders() {
        // X first: rows interpolate to 0 and to (1 + 3) / 2 = 2, and Y between
        // them gives 1. Y first: the columns interpolate to (0 + 1) / 2 = 1 and
        // to (0 + 3) / 2 = 2 — each rounded before X sees it — and X between
        // those gives 2. The fixture therefore separates the two orders instead
        // of merely agreeing with one.
        assert_eq!(y_then_x(&ORDER, 1, 1), 2);
        assert_ne!(ORDER.evaluate(1, 1), Ok(y_then_x(&ORDER, 1, 1)));
    }

    #[test]
    fn every_knot_returns_its_stored_value_exactly() {
        for (row, &y) in GRID.y_axis().iter().enumerate() {
            for (column, &x) in GRID.x_axis().iter().enumerate() {
                assert_eq!(
                    GRID.evaluate(x, y),
                    Ok(GRID.values()[row][column]),
                    "knot ({x}, {y}) at values[{row}][{column}]"
                );
            }
        }
    }

    #[test]
    fn every_knot_of_a_nonuniform_shape_grid_returns_its_stored_value() {
        for (row, &y) in SHAPES.y_axis().iter().enumerate() {
            for (column, &x) in SHAPES.x_axis().iter().enumerate() {
                assert_eq!(SHAPES.evaluate(x, y), Ok(SHAPES.values()[row][column]));
            }
        }
    }

    #[test]
    fn a_hand_computable_plane_evaluates_at_interior_points() {
        // value = 10 * x + 100 * y on this fixture.
        assert_eq!(PLANE.evaluate(5, 0), Ok(50));
        assert_eq!(PLANE.evaluate(0, 5), Ok(500));
        assert_eq!(PLANE.evaluate(5, 5), Ok(550));
        assert_eq!(PLANE.evaluate(3, 7), Ok(730));
        assert_eq!(PLANE.evaluate(10, 10), Ok(1_100));
    }

    #[test]
    fn a_nonuniform_grid_evaluates_at_interior_points() {
        // Cell x in [0, 10], y in [0, 4]. At x = 5: lower row (0, 100) -> 50,
        // upper row (10, 110) -> 60. At y = 2: (50 + 60) / 2 = 55.
        assert_eq!(GRID.evaluate(5, 2), Ok(55));

        // The wide X segment [10, 110] at x = 60: lower row (100, 300) -> 200,
        // upper row (110, 310) -> 210. At y = 2: 205.
        assert_eq!(GRID.evaluate(60, 2), Ok(205));

        // The upper Y segment [4, 8] at y = 6, x = 10: column values 110 and
        // 60 give 85.
        assert_eq!(GRID.evaluate(10, 6), Ok(85));

        // A point interior on both axes: x = 60, y = 6. Lower row (y = 4):
        // (110 + 310) / 2 = 210. Upper row (y = 8): (60 + 260) / 2 = 160.
        // Y midway: 185.
        assert_eq!(GRID.evaluate(60, 6), Ok(185));
    }

    #[test]
    fn rows_of_every_sign_and_slope_compose_correctly() {
        // Positive increasing row, X midway.
        assert_eq!(SHAPES.evaluate(50, 0), Ok(15));
        // Negative decreasing row, X midway.
        assert_eq!(SHAPES.evaluate(50, 10), Ok(-15));
        // Flat row: every X returns the same value.
        for x in [0, 1, 50, 99, 100, 101, 199, 200] {
            assert_eq!(SHAPES.evaluate(x, 20), Ok(7));
        }
        // Zero-crossing row: the crossing lands exactly on the middle knot.
        assert_eq!(SHAPES.evaluate(100, 30), Ok(0));
        assert_eq!(SHAPES.evaluate(50, 30), Ok(-50));
        assert_eq!(SHAPES.evaluate(150, 30), Ok(50));
    }

    #[test]
    fn a_row_pair_of_opposite_sign_interpolates_through_zero_along_y() {
        // Between the positive row (y = 0) and the negative row (y = 10), the
        // Y midpoint of a symmetric pair is zero.
        assert_eq!(SHAPES.evaluate(0, 5), Ok(0));
        assert_eq!(SHAPES.evaluate(100, 5), Ok(0));
        assert_eq!(SHAPES.evaluate(200, 5), Ok(0));
    }

    #[test]
    fn half_way_values_round_away_from_zero_through_the_composition() {
        static X: [u16; 2] = [0, 2];
        static Y: [u16; 2] = [0, 2];
        // X midpoints are 0.5 and -0.5 on the two rows before rounding.
        static VALUES: [[i32; 2]; 2] = [[0, 1], [0, -1]];
        static SURFACE: BilinearSurface<2, 2> = BilinearSurface::new(&X, &Y, &VALUES);

        // Lower row at x = 1: (0 + 1) / 2 = 0.5 -> 1. Upper row: -0.5 -> -1.
        assert_eq!(SURFACE.evaluate(1, 0), Ok(1));
        assert_eq!(SURFACE.evaluate(1, 2), Ok(-1));
        // Y between the two rounded results: (1 + -1) / 2 = 0.
        assert_eq!(SURFACE.evaluate(1, 1), Ok(0));
    }

    #[test]
    fn extreme_operands_evaluate_without_overflow() {
        let mid = u16::MAX / 2;

        assert_eq!(EXTREME.evaluate(0, 0), Ok(i32::MIN));
        assert_eq!(EXTREME.evaluate(u16::MAX, 0), Ok(i32::MAX));
        assert_eq!(EXTREME.evaluate(0, u16::MAX), Ok(i32::MAX));
        assert_eq!(EXTREME.evaluate(u16::MAX, u16::MAX), Ok(i32::MIN));

        // The centre of this saddle. Both X rows round to +/- 32_768 and the Y
        // step between them rounds a numerator of -32_768 over a span of
        // 65_535 away from zero.
        assert_eq!(EXTREME.evaluate(mid, mid), Ok(-1));

        // Sweeping the widest span exercises the largest numerators the
        // arithmetic can see. Every intermediate stays in the convex hull of
        // its two endpoints, which is what `interpolate_segment` asserts in a
        // debug build, so an overflow or a truncating cast here would panic
        // rather than pass silently.
        for x in [0, 1, mid, mid + 1, u16::MAX - 1, u16::MAX] {
            for y in [0, 1, mid, u16::MAX] {
                assert!(EXTREME.evaluate(x, y).is_ok(), "({x}, {y}) left the domain");
            }
        }
    }

    #[test]
    fn an_error_side_reports_the_coordinate_and_the_bound() {
        assert_eq!(
            INSET.evaluate(9, 100),
            Err(SurfaceError::XBelow {
                coordinate: 9,
                bound: 10,
            })
        );
        assert_eq!(
            INSET.evaluate(41, 100),
            Err(SurfaceError::XAbove {
                coordinate: 41,
                bound: 40,
            })
        );
        assert_eq!(
            INSET.evaluate(10, 99),
            Err(SurfaceError::YBelow {
                coordinate: 99,
                bound: 100,
            })
        );
        assert_eq!(
            INSET.evaluate(10, 401),
            Err(SurfaceError::YAbove {
                coordinate: 401,
                bound: 400,
            })
        );
    }

    #[test]
    fn error_error_corners_report_the_x_side() {
        // All four corners with both axes out of domain and every side an
        // Error: the X-side error wins in each.
        let cases = [
            (
                9,
                99,
                SurfaceError::XBelow {
                    coordinate: 9,
                    bound: 10,
                },
            ),
            (
                9,
                401,
                SurfaceError::XBelow {
                    coordinate: 9,
                    bound: 10,
                },
            ),
            (
                41,
                99,
                SurfaceError::XAbove {
                    coordinate: 41,
                    bound: 40,
                },
            ),
            (
                41,
                401,
                SurfaceError::XAbove {
                    coordinate: 41,
                    bound: 40,
                },
            ),
        ];

        for (x, y, expected) in cases {
            assert_eq!(INSET.evaluate(x, y), Err(expected), "corner ({x}, {y})");
        }
    }

    #[test]
    fn a_clamped_x_does_not_suppress_a_y_error() {
        static CLAMP_X: BilinearSurface<3, 3> = inset(
            BoundaryPolicy::new()
                .with_x_below(Boundary::Clamp)
                .with_x_above(Boundary::Clamp),
        );

        // X clamps on both sides, so the surviving error can only be the Y one.
        assert_eq!(
            CLAMP_X.evaluate(0, 99),
            Err(SurfaceError::YBelow {
                coordinate: 99,
                bound: 100,
            })
        );
        assert_eq!(
            CLAMP_X.evaluate(65_535, 401),
            Err(SurfaceError::YAbove {
                coordinate: 401,
                bound: 400,
            })
        );
    }

    #[test]
    fn a_clamped_y_does_not_suppress_an_x_error() {
        static CLAMP_Y: BilinearSurface<3, 3> = inset(
            BoundaryPolicy::new()
                .with_y_below(Boundary::Clamp)
                .with_y_above(Boundary::Clamp),
        );

        assert_eq!(
            CLAMP_Y.evaluate(9, 0),
            Err(SurfaceError::XBelow {
                coordinate: 9,
                bound: 10,
            })
        );
        assert_eq!(
            CLAMP_Y.evaluate(41, 65_535),
            Err(SurfaceError::XAbove {
                coordinate: 41,
                bound: 40,
            })
        );
    }

    #[test]
    fn clamped_edges_evaluate_the_boundary_without_extrapolating() {
        static CLAMPED: BilinearSurface<3, 3> = inset(
            BoundaryPolicy::new()
                .with_x_below(Boundary::Clamp)
                .with_x_above(Boundary::Clamp)
                .with_y_below(Boundary::Clamp)
                .with_y_above(Boundary::Clamp),
        );

        // An X-clamped edge evaluates the boundary column at the given Y.
        assert_eq!(CLAMPED.evaluate(0, 100), CLAMPED.evaluate(10, 100));
        assert_eq!(CLAMPED.evaluate(65_535, 300), CLAMPED.evaluate(40, 300));
        // A Y-clamped edge evaluates the boundary row at the given X.
        assert_eq!(CLAMPED.evaluate(20, 0), CLAMPED.evaluate(20, 100));
        assert_eq!(CLAMPED.evaluate(30, 65_535), CLAMPED.evaluate(30, 400));

        // No clamped result can leave the hull of the declared values.
        for x in [0, 1, 9, 41, 65_535] {
            for y in [0, 99, 401, 65_535] {
                let value = CLAMPED.evaluate(x, y).expect("every side clamps");
                assert!(
                    (0..=22).contains(&value),
                    "({x}, {y}) extrapolated to {value}"
                );
            }
        }
    }

    #[test]
    fn clamp_clamp_corners_return_the_corner_value() {
        static CLAMPED: BilinearSurface<3, 3> = inset(
            BoundaryPolicy::new()
                .with_x_below(Boundary::Clamp)
                .with_x_above(Boundary::Clamp)
                .with_y_below(Boundary::Clamp)
                .with_y_above(Boundary::Clamp),
        );

        assert_eq!(CLAMPED.evaluate(0, 0), Ok(INSET_VALUES[0][0]));
        assert_eq!(CLAMPED.evaluate(65_535, 0), Ok(INSET_VALUES[0][2]));
        assert_eq!(CLAMPED.evaluate(0, 65_535), Ok(INSET_VALUES[2][0]));
        assert_eq!(CLAMPED.evaluate(65_535, 65_535), Ok(INSET_VALUES[2][2]));
    }

    #[test]
    fn each_side_selects_independently_of_the_other_three() {
        // Only X-below clamps. The other three sides still reject.
        static ONE_SIDE: BilinearSurface<3, 3> =
            inset(BoundaryPolicy::new().with_x_below(Boundary::Clamp));

        assert_eq!(ONE_SIDE.evaluate(0, 100), Ok(INSET_VALUES[0][0]));
        assert_eq!(
            ONE_SIDE.evaluate(41, 100),
            Err(SurfaceError::XAbove {
                coordinate: 41,
                bound: 40,
            })
        );
        assert_eq!(
            ONE_SIDE.evaluate(0, 99),
            Err(SurfaceError::YBelow {
                coordinate: 99,
                bound: 100,
            })
        );
        assert_eq!(
            ONE_SIDE.evaluate(0, 401),
            Err(SurfaceError::YAbove {
                coordinate: 401,
                bound: 400,
            })
        );
    }

    #[test]
    fn all_sixteen_policies_agree_in_domain() {
        // A policy selects what happens outside the domain and nothing else,
        // so no combination may change an in-domain value.
        let expected = INSET.evaluate(25, 250);

        for bits in 0..16u8 {
            let side = |shift: u8| {
                if (bits >> shift) & 1 == 0 {
                    Boundary::Error
                } else {
                    Boundary::Clamp
                }
            };
            let surface = inset(
                BoundaryPolicy::new()
                    .with_x_below(side(0))
                    .with_x_above(side(1))
                    .with_y_below(side(2))
                    .with_y_above(side(3)),
            );

            assert_eq!(surface.evaluate(25, 250), expected, "policy bits {bits}");
        }
    }

    #[test]
    fn repeated_calls_are_identical_and_mutate_no_state() {
        let before = GRID;
        let first = GRID.evaluate(60, 6);

        for _ in 0..64 {
            assert_eq!(GRID.evaluate(60, 6), first);
        }

        // Interleaving other coordinates cannot disturb the result either:
        // there is no cached cell to invalidate.
        for x in [0, 10, 55, 110] {
            for y in [0, 4, 7, 8] {
                let _ = GRID.evaluate(x, y);
            }
        }

        assert_eq!(GRID.evaluate(60, 6), first);
        assert_eq!(GRID, before);
        assert_eq!(GRID.values(), before.values());
    }

    #[test]
    fn evaluation_is_free_of_warm_up_behaviour() {
        // A freshly declared handle over the same tables answers exactly as one
        // that has already been evaluated many times.
        static FRESH: BilinearSurface<3, 3> = inset(BoundaryPolicy::new());

        let warmed = INSET.evaluate(25, 250);
        assert_eq!(FRESH.evaluate(25, 250), warmed);
    }

    #[test]
    fn a_two_knot_axis_pairs_with_a_longer_one() {
        // `len - 2 == 0` on Y: every clamp resolves to cell zero, and the
        // composition still reads two distinct rows.
        static X: [u16; 4] = [0, 1, 2, 3];
        static Y: [u16; 2] = [0, 1];
        static VALUES: [[i32; 4]; 2] = [[0, 10, 20, 30], [100, 110, 120, 130]];
        static SURFACE: BilinearSurface<4, 2> = BilinearSurface::new(&X, &Y, &VALUES);

        for (row, &y) in Y.iter().enumerate() {
            for (column, &x) in X.iter().enumerate() {
                assert_eq!(SURFACE.evaluate(x, y), Ok(VALUES[row][column]));
            }
        }
    }
}

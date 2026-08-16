//! The outcome type reported when a coordinate leaves the declared domain.

use core::fmt::{Display, Formatter, Result as FmtResult};

/// A coordinate fell outside the declared domain on a side selecting
/// [`Boundary::Error`](crate::Boundary::Error).
///
/// The four variants distinguish the four sides, so a caller never has to
/// infer which axis rejected the input. Each carries the coordinate that was
/// supplied and the bound that applied to it.
///
/// This enum is deliberately not `#[non_exhaustive]`. The v0.1 contract fixes
/// exactly these four outcomes: shape mismatches are type errors, axis
/// invariants are checked when the surface is defined, and the arithmetic
/// cannot overflow for valid operands. Exhaustive matching without a wildcard
/// arm is therefore both possible and intended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SurfaceError {
    /// The X coordinate was below the first X knot.
    XBelow {
        /// The rejected X coordinate.
        coordinate: u16,
        /// The first X knot: the inclusive lower bound of the X domain.
        bound: u16,
    },
    /// The X coordinate was above the last X knot.
    XAbove {
        /// The rejected X coordinate.
        coordinate: u16,
        /// The last X knot: the inclusive upper bound of the X domain.
        bound: u16,
    },
    /// The Y coordinate was below the first Y knot.
    YBelow {
        /// The rejected Y coordinate.
        coordinate: u16,
        /// The first Y knot: the inclusive lower bound of the Y domain.
        bound: u16,
    },
    /// The Y coordinate was above the last Y knot.
    YAbove {
        /// The rejected Y coordinate.
        coordinate: u16,
        /// The last Y knot: the inclusive upper bound of the Y domain.
        bound: u16,
    },
}

impl SurfaceError {
    /// Returns the coordinate that was rejected, whichever side reported it.
    ///
    /// # Examples
    ///
    /// ```
    /// use ph_surfaces::SurfaceError;
    ///
    /// let error = SurfaceError::YAbove {
    ///     coordinate: 900,
    ///     bound: 500,
    /// };
    ///
    /// assert_eq!(error.coordinate(), 900);
    /// assert_eq!(error.bound(), 500);
    /// ```
    #[must_use]
    pub const fn coordinate(&self) -> u16 {
        match *self {
            Self::XBelow { coordinate, .. }
            | Self::XAbove { coordinate, .. }
            | Self::YBelow { coordinate, .. }
            | Self::YAbove { coordinate, .. } => coordinate,
        }
    }

    /// Returns the bound that applied, whichever side reported it.
    ///
    /// For the two below-domain variants this is the first knot of the axis;
    /// for the two above-domain variants it is the last knot.
    #[must_use]
    pub const fn bound(&self) -> u16 {
        match *self {
            Self::XBelow { bound, .. }
            | Self::XAbove { bound, .. }
            | Self::YBelow { bound, .. }
            | Self::YAbove { bound, .. } => bound,
        }
    }
}

impl Display for SurfaceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match *self {
            Self::XBelow { coordinate, bound } => {
                write!(
                    f,
                    "x coordinate {coordinate} is below the x axis minimum {bound}"
                )
            }
            Self::XAbove { coordinate, bound } => {
                write!(
                    f,
                    "x coordinate {coordinate} is above the x axis maximum {bound}"
                )
            }
            Self::YBelow { coordinate, bound } => {
                write!(
                    f,
                    "y coordinate {coordinate} is below the y axis minimum {bound}"
                )
            }
            Self::YAbove { coordinate, bound } => {
                write!(
                    f,
                    "y coordinate {coordinate} is above the y axis maximum {bound}"
                )
            }
        }
    }
}

impl core::error::Error for SurfaceError {}

#[cfg(test)]
mod tests {
    use super::SurfaceError;
    use core::fmt::Write;

    /// A fixed-capacity `core::fmt::Write` sink.
    ///
    /// The crate denies `clippy::std_instead_of_core`, so tests render
    /// `Display` output without `std::format!`.
    struct Sink {
        buffer: [u8; 96],
        used: usize,
    }

    impl Sink {
        const fn new() -> Self {
            Self {
                buffer: [0; 96],
                used: 0,
            }
        }

        fn as_str(&self) -> &str {
            core::str::from_utf8(&self.buffer[..self.used]).expect("sink holds valid utf-8")
        }
    }

    impl Write for Sink {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            let bytes = s.as_bytes();
            let end = self.used + bytes.len();
            assert!(end <= self.buffer.len(), "sink overflowed");
            self.buffer[self.used..end].copy_from_slice(bytes);
            self.used = end;
            Ok(())
        }
    }

    fn rendered(error: SurfaceError) -> Sink {
        let mut sink = Sink::new();
        write!(sink, "{error}").expect("writing to the sink cannot fail");
        sink
    }

    #[test]
    fn each_side_reports_its_coordinate_and_bound() {
        let cases = [
            SurfaceError::XBelow {
                coordinate: 3,
                bound: 10,
            },
            SurfaceError::XAbove {
                coordinate: 900,
                bound: 500,
            },
            SurfaceError::YBelow {
                coordinate: 0,
                bound: 7,
            },
            SurfaceError::YAbove {
                coordinate: u16::MAX,
                bound: 1,
            },
        ];
        let expected = [(3, 10), (900, 500), (0, 7), (u16::MAX, 1)];

        for (error, (coordinate, bound)) in cases.into_iter().zip(expected) {
            assert_eq!(error.coordinate(), coordinate);
            assert_eq!(error.bound(), bound);
        }
    }

    #[test]
    fn sides_are_not_equal_to_each_other() {
        let x_below = SurfaceError::XBelow {
            coordinate: 4,
            bound: 9,
        };
        let x_above = SurfaceError::XAbove {
            coordinate: 4,
            bound: 9,
        };
        let y_below = SurfaceError::YBelow {
            coordinate: 4,
            bound: 9,
        };
        let y_above = SurfaceError::YAbove {
            coordinate: 4,
            bound: 9,
        };

        assert_ne!(x_below, x_above);
        assert_ne!(x_below, y_below);
        assert_ne!(x_above, y_above);
        assert_ne!(y_below, y_above);
        assert_eq!(
            x_below,
            SurfaceError::XBelow {
                coordinate: 4,
                bound: 9,
            }
        );
    }

    #[test]
    fn display_names_the_axis_the_direction_and_the_bound() {
        let x_below = rendered(SurfaceError::XBelow {
            coordinate: 3,
            bound: 10,
        });
        assert_eq!(
            x_below.as_str(),
            "x coordinate 3 is below the x axis minimum 10"
        );

        let x_above = rendered(SurfaceError::XAbove {
            coordinate: 900,
            bound: 500,
        });
        assert_eq!(
            x_above.as_str(),
            "x coordinate 900 is above the x axis maximum 500"
        );

        let y_below = rendered(SurfaceError::YBelow {
            coordinate: 3,
            bound: 10,
        });
        assert_eq!(
            y_below.as_str(),
            "y coordinate 3 is below the y axis minimum 10"
        );

        let y_above = rendered(SurfaceError::YAbove {
            coordinate: 900,
            bound: 500,
        });
        assert_eq!(
            y_above.as_str(),
            "y coordinate 900 is above the y axis maximum 500"
        );
    }

    #[test]
    fn the_error_has_no_source() {
        let error = SurfaceError::XBelow {
            coordinate: 1,
            bound: 2,
        };

        assert!(core::error::Error::source(&error).is_none());
    }
}

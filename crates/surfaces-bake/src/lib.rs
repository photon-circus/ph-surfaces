//! Host-only baker for `ph-surfaces` tables.
//!
//! This crate requires `std` and `f64`. It must **never** be linked into
//! target firmware. The runtime crate does not depend on this package.

#![forbid(unsafe_code)]

// Sample ingest of delimited points and an explicit grid: issue #39.
// Quantization and the emitted deviation bound: issue #40.
// Rust emission and the checked-in generated-source drift gate: issue #41.
// Frozen golden vectors: issue #42.

#[cfg(test)]
mod tests {
    use ph_surfaces::BilinearSurface;

    #[test]
    fn runtime_is_available_as_a_dev_dependency_oracle() {
        static AXIS: [u16; 2] = [0, 2];
        static VALUES: [[i32; 2]; 2] = [[0, 0], [1, 3]];
        static SURFACE: BilinearSurface<2, 2> = BilinearSurface::new(&AXIS, &AXIS, &VALUES);
        assert_eq!(SURFACE.evaluate(0, 0), Ok(0));
    }
}

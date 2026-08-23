//! The ph-surfaces verification gate, as a library so that `tests/mutation.rs`
//! can call individual checks directly.
//!
//! The retired `scripts/guard-selftest.sh` had to re-enter the whole gate as a
//! subprocess to exercise one guard, which is why a `REQUIRE_NO_SKIPS=1` parent
//! run could make a guard appear to fire for the wrong reason. A check is an
//! ordinary function here, so a test calls it with an explicit `Ctx` and there
//! is no ambient environment to leak.

pub mod checks;
pub mod config;
pub mod proc;
pub mod runner;
pub mod text;

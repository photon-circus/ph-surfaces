//! Host baker CLI. Derivation stays in this crate; the target never links it.
//!
//! This crate requires `std` and `f64`. It must **never** be linked into
//! target firmware.

#![forbid(unsafe_code)]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [] => not_implemented_in_s2(),
        [a] if a == "--help" || a == "-h" => print_help(),
        [a] if a == "--emit-rust" => not_implemented_in_s2(),
        [a] if a == "--emit-golden" => not_implemented_in_s2(),
        _ => {
            eprintln!("ph-surfaces-bake: unknown args. Try --help");
            std::process::exit(2);
        }
    }
}

fn print_help() {
    println!(
        "ph-surfaces-bake — host-only baker\n\
         \n\
         This crate requires std and f64 and must never be linked into target firmware.\n\
         \n\
         Usage:\n\
         ph-surfaces-bake --help\n\
         ph-surfaces-bake --emit-rust\n\
         ph-surfaces-bake --emit-golden\n\
         \n\
         --emit-rust and --emit-golden are not implemented in S2."
    );
}

fn not_implemented_in_s2() {
    eprintln!("ph-surfaces-bake: not implemented in S2");
    std::process::exit(1);
}

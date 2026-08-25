//! Host baker CLI. Derivation stays in this crate; the target never links it.
//!
//! This crate requires `std` and `f64`. It must **never** be linked into
//! target firmware.

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::ExitCode;

use ph_surfaces_bake::{Axis, BakeError, BakeInput, EmitAxis, QuantizedTable, emit_rust_with};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match dispatch(&args) {
        Ok(message) => {
            print!("{message}");
            ExitCode::SUCCESS
        }
        Err((code, message)) => {
            eprint!("{message}");
            ExitCode::from(code)
        }
    }
}

fn dispatch(args: &[String]) -> Result<String, (u8, String)> {
    match args {
        [] => Err((2, usage_error("missing args"))),
        [a] if a == "--help" || a == "-h" => Ok(help()),
        [a] if a == "--emit-golden" => emit_golden(None),
        [a, b] if a == "--emit-golden" && b == "--out" => {
            Err((2, usage_error("missing --out value")))
        }
        [a, b, c] if a == "--emit-golden" && b == "--out" => emit_golden(Some(PathBuf::from(c))),
        [a, ..] if a == "--emit-golden" => Err((2, usage_error("unknown args"))),
        _ => ingest(args),
    }
}

fn ingest(args: &[String]) -> Result<String, (u8, String)> {
    let parsed = parse_ingest(args).map_err(|message| (2, usage_error(&message)))?;
    let text = std::fs::read_to_string(&parsed.samples).map_err(|error| {
        (
            1,
            format!(
                "ph-surfaces-bake: could not read {}: {error}\n",
                parsed.samples.display()
            ),
        )
    })?;
    match BakeInput::parse(&text, parsed.x, parsed.y, parsed.scale) {
        Ok(input) => match input.quantize() {
            Ok(table) if parsed.emit_rust => emit_rust_with(&table, parsed.x_axis, parsed.y_axis)
                .map_err(|error| (1, bake_error(error))),
            Ok(table) => Ok(summary(&input, &table)),
            Err(error) => Err((1, bake_error(error))),
        },
        Err(error) => Err((1, bake_error(error))),
    }
}

#[derive(Debug)]
struct IngestArgs {
    samples: PathBuf,
    x: Axis,
    y: Axis,
    scale: f64,
    emit_rust: bool,
    x_axis: EmitAxis,
    y_axis: EmitAxis,
}

fn parse_ingest(args: &[String]) -> Result<IngestArgs, String> {
    let mut samples = None;
    let mut x_knots = None;
    let mut y_knots = None;
    let mut x_uniform = None;
    let mut y_uniform = None;
    let mut scale = None;
    let mut emit_rust = false;
    let mut x_buckets = None;
    let mut y_buckets = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--samples" => {
                samples = Some(PathBuf::from(take_value(args, &mut index, "--samples")?))
            }
            "--x-knots" => {
                x_knots = Some(parse_knots(take_value(args, &mut index, "--x-knots")?)?);
            }
            "--y-knots" => {
                y_knots = Some(parse_knots(take_value(args, &mut index, "--y-knots")?)?);
            }
            "--x-uniform" => {
                x_uniform = Some(parse_uniform(take_value(args, &mut index, "--x-uniform")?)?);
            }
            "--y-uniform" => {
                y_uniform = Some(parse_uniform(take_value(args, &mut index, "--y-uniform")?)?);
            }
            "--scale" => {
                scale = Some(parse_scale(take_value(args, &mut index, "--scale")?)?);
            }
            "--emit-rust" => emit_rust = true,
            "--x-bucketed" => {
                x_buckets = Some(parse_buckets(take_value(
                    args,
                    &mut index,
                    "--x-bucketed",
                )?)?);
            }
            "--y-bucketed" => {
                y_buckets = Some(parse_buckets(take_value(
                    args,
                    &mut index,
                    "--y-bucketed",
                )?)?);
            }
            _ => return Err("unknown args".to_string()),
        }
        index += 1;
    }
    let samples = samples.ok_or_else(|| "missing --samples".to_string())?;
    let x = axis_from_flags("--x-knots", x_knots, "--x-uniform", x_uniform)?;
    let y = axis_from_flags("--y-knots", y_knots, "--y-uniform", y_uniform)?;
    let scale = scale.ok_or_else(|| "missing --scale".to_string())?;
    if (x_buckets.is_some() || y_buckets.is_some()) && !emit_rust {
        return Err("--x-bucketed/--y-bucketed require --emit-rust".to_string());
    }
    Ok(IngestArgs {
        samples,
        x,
        y,
        scale,
        emit_rust,
        x_axis: emit_axis(x_buckets),
        y_axis: emit_axis(y_buckets),
    })
}

fn emit_axis(buckets: Option<usize>) -> EmitAxis {
    match buckets {
        Some(buckets) => EmitAxis::Bucketed { buckets },
        None => EmitAxis::Binary,
    }
}

fn parse_buckets(raw: &str) -> Result<usize, String> {
    let buckets: usize = raw
        .parse()
        .map_err(|_| "invalid bucket count".to_string())?;
    if !(1..=EmitAxis::MAX_BUCKETS).contains(&buckets) {
        Err("invalid bucket count".to_string())
    } else {
        Ok(buckets)
    }
}

fn take_value<'a>(args: &'a [String], index: &mut usize, flag: &str) -> Result<&'a str, String> {
    *index += 1;
    match args.get(*index) {
        Some(value) if !value.starts_with("--") => Ok(value.as_str()),
        _ => Err(format!("missing {flag} value")),
    }
}

fn parse_knots(raw: &str) -> Result<Vec<u16>, String> {
    let knots: Result<Vec<u16>, _> = split_fields(raw).map(str::parse).collect();
    knots.map_err(|_| "expected comma-separated u16 knots".to_string())
}

fn parse_uniform(raw: &str) -> Result<(u16, u16, usize), String> {
    let mut parts = split_fields(raw);
    let origin = parse_part(parts.next(), "uniform origin")?;
    let step = parse_part(parts.next(), "uniform step")?;
    let count = parse_part(parts.next(), "uniform count")?;
    if parts.next().is_some() {
        return Err("expected origin,step,count".to_string());
    }
    Ok((origin, step, count))
}

fn parse_part<T: std::str::FromStr>(part: Option<&str>, what: &str) -> Result<T, String> {
    part.ok_or_else(|| format!("expected {what}"))?
        .parse()
        .map_err(|_| format!("invalid {what}"))
}

fn parse_scale(raw: &str) -> Result<f64, String> {
    let scale: f64 = raw.parse().map_err(|_| "invalid --scale".to_string())?;
    if scale.is_finite() {
        Ok(scale)
    } else {
        Err("invalid --scale".to_string())
    }
}

fn split_fields(raw: &str) -> impl Iterator<Item = &str> {
    raw.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|part| !part.is_empty())
}

fn axis_from_flags(
    knots_flag: &str,
    knots: Option<Vec<u16>>,
    uniform_flag: &str,
    uniform: Option<(u16, u16, usize)>,
) -> Result<Axis, String> {
    match (knots, uniform) {
        (Some(knots), None) => Ok(Axis::knots(knots)),
        (None, Some((origin, step, count))) => Ok(Axis::uniform(origin, step, count)),
        (Some(_), Some(_)) => Err(format!(
            "{knots_flag} and {uniform_flag} cannot both be set; the baker does not choose a grid"
        )),
        (None, None) => Err(format!("missing {knots_flag} or {uniform_flag}")),
    }
}

fn summary(input: &BakeInput, table: &QuantizedTable) -> String {
    let mut out = format!(
        "ingested {} samples; {}; {}; scale {}\n",
        input.samples().len(),
        axis_summary("x", input.x()),
        axis_summary("y", input.y()),
        input.scale()
    );
    out.push_str(&format!(
        "deviation from supplied samples: MAX_ERR_LSB={} (i32 value LSBs, upper bound)\n",
        table.max_err_lsb
    ));
    out.push_str(&format!(
        "rms_lsb={} worst_sample=({}, {})\n",
        table.rms_lsb, table.worst_x, table.worst_y
    ));
    out.push_str("per-knot residual (i32 value LSBs, row-major):\n");
    for row in &table.per_knot_lsb {
        out.push_str(&format!("  {row:?}\n"));
    }
    out
}

fn axis_summary(name: &str, axis: &Axis) -> String {
    match axis {
        Axis::Knots(knots) => format!("{name} knots {knots:?}"),
        Axis::Uniform {
            origin,
            step,
            count,
        } => format!("{name} uniform origin={origin} step={step} count={count}"),
    }
}

fn help() -> String {
    "ph-surfaces-bake — host-only baker\n\
     \n\
     This crate requires std and f64 and must never be linked into target firmware.\n\
     \n\
     Usage:\n\
     ph-surfaces-bake --help\n\
     ph-surfaces-bake --samples PATH --x-knots LIST --y-knots LIST --scale N\n\
     ph-surfaces-bake --samples PATH --x-uniform ORIGIN,STEP,COUNT --y-uniform ORIGIN,STEP,COUNT --scale N\n\
     ph-surfaces-bake --emit-rust --samples PATH --x-knots LIST --y-knots LIST --scale N\n\
     ph-surfaces-bake --emit-golden\n\
     ph-surfaces-bake --emit-golden --out DIR\n\
     \n\
     --samples      delimited text: one X Y value point per line (whitespace and/or comma)\n\
     --x-knots      explicit X knots as comma-separated u16 values\n\
     --y-knots      explicit Y knots as comma-separated u16 values\n\
     --x-uniform    X axis as origin,step,count (runtime UniformAxis)\n\
     --y-uniform    Y axis as origin,step,count (runtime UniformAxis)\n\
     --scale        output scale for the i32 value domain (applied at quantize)\n\
     --emit-rust    write static Rust tables to stdout (BinaryAxis × BinaryAxis)\n\
     --x-bucketed   emit X as BucketedAxis with B in 1..=65536 (requires --emit-rust)\n\
     --y-bucketed   emit Y as BucketedAxis with B in 1..=65536 (requires --emit-rust)\n\
     --emit-golden  write frozen CSV under crates/surfaces/tests/conformance/golden/\n\
                    located from the working directory, or --out DIR\n\
     \n\
     Each axis takes either a knot list or a uniform descriptor, never both.\n\
     The baker does not choose a grid.\n"
        .to_string()
}

fn emit_golden(out: Option<PathBuf>) -> Result<String, (u8, String)> {
    let dir = match out {
        Some(dir) => dir,
        None => golden_dir_from_cwd()?,
    };
    ph_surfaces_bake::write_goldens(&dir).map_err(|error| {
        (
            1,
            format!(
                "ph-surfaces-bake: could not write goldens under {}: {error}\n",
                dir.display()
            ),
        )
    })?;
    Ok(format!("wrote goldens under {}\n", dir.display()))
}

fn golden_dir_from_cwd() -> Result<PathBuf, (u8, String)> {
    let mut dir = std::env::current_dir().map_err(|error| {
        (
            1,
            format!("ph-surfaces-bake: could not read working directory: {error}\n"),
        )
    })?;
    loop {
        if dir.join("crates/surfaces/Cargo.toml").is_file() {
            return Ok(dir.join("crates/surfaces/tests/conformance/golden"));
        }
        if !dir.pop() {
            break;
        }
    }
    Err((
        2,
        usage_error("could not locate crates/surfaces from the working directory; pass --out DIR"),
    ))
}

fn usage_error(detail: &str) -> String {
    format!("ph-surfaces-bake: {detail}. Try --help\n")
}

fn bake_error(error: BakeError) -> String {
    format!("ph-surfaces-bake: {error}\n")
}

#[cfg(test)]
mod tests {
    use super::{dispatch, parse_ingest};
    use ph_surfaces_bake::{Axis, EmitAxis, emit_rust};

    #[test]
    fn help_is_available() {
        let text = dispatch(&["--help".to_string()]).unwrap();
        assert!(text.contains("--samples"));
        assert!(text.contains("--scale"));
        assert!(text.contains("write static Rust tables to stdout"));
        assert!(text.contains("write frozen CSV under crates/surfaces/tests/conformance/golden/"));
        assert_eq!(dispatch(&["-h".to_string()]).unwrap(), text);
    }

    #[test]
    fn unknown_args_exit_2() {
        let err = dispatch(&["--wat".to_string()]).unwrap_err();
        assert_eq!(err.0, 2);
        assert!(err.1.contains("unknown args"));
    }

    #[test]
    fn emit_rust_alone_needs_ingest_flags() {
        let err = dispatch(&["--emit-rust".to_string()]).unwrap_err();
        assert_eq!(err.0, 2);
        assert!(err.1.contains("missing --samples"));
    }

    #[test]
    fn emit_golden_out_does_not_rewrite_the_freeze() {
        let freeze = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../surfaces/tests/conformance/golden/rounding.csv");
        let before = std::fs::read_to_string(&freeze).unwrap();
        let dir = std::env::temp_dir().join(format!(
            "ph-surfaces-bake-cli-golden-{}",
            std::process::id()
        ));
        let message = dispatch(&[
            "--emit-golden".to_string(),
            "--out".to_string(),
            dir.display().to_string(),
        ])
        .unwrap();
        assert!(message.contains("wrote goldens under"));
        let after = std::fs::read_to_string(&freeze).unwrap();
        assert_eq!(before, after);
        let written = std::fs::read_to_string(dir.join("rounding.csv")).unwrap();
        assert_eq!(written, ph_surfaces_bake::rounding_csv());
    }

    #[test]
    fn emit_golden_out_without_a_directory_is_usage() {
        let err = dispatch(&["--emit-golden".to_string(), "--out".to_string()]).unwrap_err();
        assert_eq!(err.0, 2);
        assert!(err.1.contains("missing --out value"));
    }

    #[test]
    fn missing_args_exit_2() {
        let err = dispatch(&[]).unwrap_err();
        assert_eq!(err.0, 2);
        assert!(err.1.contains("missing args"));
    }

    #[test]
    fn ingest_flags_parse_explicit_knots_and_scale() {
        let parsed = parse_ingest(&[
            "--samples".to_string(),
            "points.txt".to_string(),
            "--x-knots".to_string(),
            "0,10,20".to_string(),
            "--y-knots".to_string(),
            "0,5".to_string(),
            "--scale".to_string(),
            "1000".to_string(),
        ])
        .unwrap();
        assert_eq!(parsed.samples.as_os_str(), "points.txt");
        assert_eq!(parsed.x, Axis::knots(vec![0, 10, 20]));
        assert_eq!(parsed.y, Axis::knots(vec![0, 5]));
        assert_eq!(parsed.scale, 1000.0);
        assert!(!parsed.emit_rust);
        assert_eq!(parsed.x_axis, EmitAxis::Binary);
        assert_eq!(parsed.y_axis, EmitAxis::Binary);
    }

    #[test]
    fn ingest_flags_parse_uniform_axes() {
        let parsed = parse_ingest(&[
            "--scale".to_string(),
            "1".to_string(),
            "--y-uniform".to_string(),
            "0,5,3".to_string(),
            "--samples".to_string(),
            "p.txt".to_string(),
            "--x-uniform".to_string(),
            "0,10,3".to_string(),
        ])
        .unwrap();
        assert_eq!(parsed.x, Axis::uniform(0, 10, 3));
        assert_eq!(parsed.y, Axis::uniform(0, 5, 3));
    }

    #[test]
    fn both_knot_and_uniform_flags_on_one_axis_are_rejected() {
        let err = parse_ingest(&[
            "--samples".to_string(),
            "p.txt".to_string(),
            "--x-knots".to_string(),
            "0,10".to_string(),
            "--x-uniform".to_string(),
            "0,1,2".to_string(),
            "--y-knots".to_string(),
            "0,1".to_string(),
            "--scale".to_string(),
            "1".to_string(),
        ])
        .unwrap_err();
        assert!(err.contains("cannot both be set"));
    }

    #[test]
    fn ingest_command_summarises_a_valid_file() {
        let path = std::env::temp_dir().join("ph-surfaces-bake-s4-points.txt");
        std::fs::write(&path, "0 0 1.5\n10 0 2.5\n0 5 3.5\n10 5 4.5\n").unwrap();
        let args = [
            "--samples",
            path.to_str().unwrap(),
            "--x-knots",
            "0,10",
            "--y-knots",
            "0,5",
            "--scale",
            "1000",
        ]
        .map(String::from);
        let out = dispatch(&args).unwrap();
        assert!(out.contains("ingested 4 samples"));
        assert!(out.contains("x knots [0, 10]"));
        assert!(out.contains("y knots [0, 5]"));
        assert!(out.contains("scale 1000"));
        assert!(out.contains("MAX_ERR_LSB=0"));
        assert!(out.contains("i32 value LSBs"));
        assert!(out.contains("deviation from supplied samples"));
        assert!(!out.contains("accuracy"));
        assert!(!out.contains("device"));
    }

    #[test]
    fn ingest_command_reports_missing_nodes() {
        let path = std::env::temp_dir().join("ph-surfaces-bake-s4-missing.txt");
        std::fs::write(&path, "0 0 1.5\n10 5 2\n").unwrap();
        let args = [
            "--samples",
            path.to_str().unwrap(),
            "--x-knots",
            "0,10",
            "--y-knots",
            "0,5",
            "--scale",
            "1000",
        ]
        .map(String::from);
        let err = dispatch(&args).unwrap_err();
        assert_eq!(err.0, 1);
        assert!(err.1.contains("grid node (10, 0) has no sample"));
    }

    #[test]
    fn ingest_command_reports_bake_errors() {
        let path = std::env::temp_dir().join("ph-surfaces-bake-s3-ood.txt");
        std::fs::write(&path, "11 0 1\n").unwrap();
        let args = [
            "--samples",
            path.to_str().unwrap(),
            "--x-knots",
            "0,10",
            "--y-knots",
            "0,5",
            "--scale",
            "1",
        ]
        .map(String::from);
        let err = dispatch(&args).unwrap_err();
        assert_eq!(err.0, 1);
        assert!(
            err.1
                .contains("x coordinate 11 is above the x axis maximum 10")
        );
    }

    fn emit_args(path: &std::path::Path) -> [String; 9] {
        [
            "--emit-rust",
            "--samples",
            path.to_str().unwrap(),
            "--x-knots",
            "0,2",
            "--y-knots",
            "0,2",
            "--scale",
            "1",
        ]
        .map(String::from)
    }

    #[test]
    fn emit_rust_writes_the_same_bytes_as_the_library() {
        let path = std::env::temp_dir().join("ph-surfaces-bake-s5-emit.txt");
        std::fs::write(&path, "0 0 0\n2 0 1\n0 2 1\n2 2 2\n1 1 1\n").unwrap();
        let args = emit_args(&path);
        let a = dispatch(&args).unwrap();
        let b = dispatch(&args).unwrap();
        assert_eq!(a, b);
        assert!(!a.contains('\r'));
        let table = ph_surfaces_bake::BakeInput::parse(
            "0 0 0\n2 0 1\n0 2 1\n2 2 2\n1 1 1\n",
            Axis::knots(vec![0, 2]),
            Axis::knots(vec![0, 2]),
            1.0,
        )
        .unwrap()
        .quantize()
        .unwrap();
        assert_eq!(a, emit_rust(&table));
        assert!(a.contains("pub const MAX_ERR_LSB: i32 = 1;"));
        assert!(a.contains("pub const PAYLOAD_BYTES: usize = 24;"));
        assert!(a.contains("deviation from supplied samples"));
    }

    #[test]
    fn emit_rust_flag_order_does_not_change_bytes() {
        let path = std::env::temp_dir().join("ph-surfaces-bake-s5-emit-order.txt");
        std::fs::write(&path, "0 0 0\n2 0 1\n0 2 1\n2 2 2\n1 1 1\n").unwrap();
        let first = dispatch(&emit_args(&path)).unwrap();
        let rotated = [
            "--samples",
            path.to_str().unwrap(),
            "--scale",
            "1",
            "--y-knots",
            "0,2",
            "--emit-rust",
            "--x-knots",
            "0,2",
        ]
        .map(String::from);
        assert_eq!(first, dispatch(&rotated).unwrap());
    }

    #[test]
    fn emit_rust_bucketed_flag_calls_bucket_index() {
        let path = std::env::temp_dir().join("ph-surfaces-bake-s5-bucketed.txt");
        std::fs::write(&path, "0 0 0\n10 0 1\n0 5 2\n10 5 3\n").unwrap();
        let args = [
            "--samples",
            path.to_str().unwrap(),
            "--x-knots",
            "0,10",
            "--y-knots",
            "0,5",
            "--scale",
            "1",
            "--emit-rust",
            "--x-bucketed",
            "2",
        ]
        .map(String::from);
        let out = dispatch(&args).unwrap();
        assert!(out.contains("bucket_index(&X)"));
        assert!(out.contains("Pairing: BucketedAxis × BinaryAxis."));
        assert!(!out.contains("Y_INDEX"));
    }

    #[test]
    fn bucket_count_above_the_runtime_limit_is_rejected() {
        let err = parse_ingest(&[
            "--samples".to_string(),
            "p.txt".to_string(),
            "--x-knots".to_string(),
            "0,10".to_string(),
            "--y-knots".to_string(),
            "0,5".to_string(),
            "--scale".to_string(),
            "1".to_string(),
            "--emit-rust".to_string(),
            "--x-bucketed".to_string(),
            "65537".to_string(),
        ])
        .unwrap_err();
        assert!(err.contains("invalid bucket count"));
    }

    #[test]
    fn max_runtime_bucket_count_is_accepted() {
        let parsed = parse_ingest(&[
            "--samples".to_string(),
            "p.txt".to_string(),
            "--x-knots".to_string(),
            "0,10".to_string(),
            "--y-knots".to_string(),
            "0,5".to_string(),
            "--scale".to_string(),
            "1".to_string(),
            "--emit-rust".to_string(),
            "--y-bucketed".to_string(),
            "65536".to_string(),
        ])
        .unwrap();
        assert_eq!(parsed.y_axis, EmitAxis::Bucketed { buckets: 65_536 });
    }

    #[test]
    fn bucketed_without_emit_rust_is_rejected() {
        let err = parse_ingest(&[
            "--samples".to_string(),
            "p.txt".to_string(),
            "--x-knots".to_string(),
            "0,10".to_string(),
            "--y-knots".to_string(),
            "0,5".to_string(),
            "--scale".to_string(),
            "1".to_string(),
            "--x-bucketed".to_string(),
            "2".to_string(),
        ])
        .unwrap_err();
        assert!(err.contains("require --emit-rust"));
    }
}

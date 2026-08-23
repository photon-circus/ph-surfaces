//! Native verification commands for ph-surfaces.

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use clap::{Args, Parser, Subcommand};

use xtask::config::Config;
use xtask::runner::{self, Ctx, Profile};

#[derive(Parser)]
#[command(name = "xtask", about = "The ph-surfaces verification gate")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run the verification matrix.
    Ci(CiArgs),
    /// Re-measure the code-size snapshot.
    CodeSize(WriteArgs),
    /// Re-disassemble the emitted-instruction snapshots.
    Asm(WriteArgs),
    /// Print the configured check registry.
    List,
}

#[derive(Args)]
struct CiArgs {
    /// Which checks to run: dev, full, or release.
    #[arg(long, default_value = "full", value_parser = ["dev", "full", "release"])]
    profile: String,
    /// Run one named check; repeatable and forbidden for release evidence.
    #[arg(long)]
    only: Vec<String>,
    /// Toolchain for core-only proofs.
    #[arg(long, default_value = "nightly")]
    nightly: String,
    /// Skip ordinary embedded builds and code-size measurement.
    #[arg(long)]
    skip_embedded: bool,
    /// Stop at the first failure.
    #[arg(long)]
    fail_fast: bool,
    /// Measure host test coverage with cargo-llvm-cov.
    #[arg(long)]
    coverage: bool,
    /// Operate on another checkout.
    #[arg(long)]
    root: Option<PathBuf>,
}

impl Default for CiArgs {
    fn default() -> Self {
        Self {
            profile: "full".to_string(),
            only: Vec::new(),
            nightly: "nightly".to_string(),
            skip_embedded: false,
            fail_fast: false,
            coverage: false,
            root: None,
        }
    }
}

#[derive(Args)]
struct WriteArgs {
    /// Rewrite the committed snapshot instead of printing it.
    #[arg(long)]
    write: bool,
}

fn main() -> ExitCode {
    let command = Cli::parse()
        .command
        .unwrap_or(Command::Ci(CiArgs::default()));
    match dispatch(command) {
        Ok(code) => ExitCode::from(code),
        Err(message) => {
            eprintln!("xtask: {message}");
            ExitCode::from(2)
        }
    }
}

fn dispatch(command: Command) -> Result<u8, String> {
    match command {
        Command::Ci(args) => run_ci(args),
        Command::CodeSize(args) => run_code_size(args.write),
        Command::Asm(args) => run_asm(args.write),
        Command::List => {
            let ctx = context(
                current_root()?,
                Profile::Full,
                "nightly".into(),
                false,
                false,
            )?;
            runner::list(&ctx.config.checks);
            Ok(0)
        }
    }
}

fn current_root() -> Result<PathBuf, String> {
    let here = env::current_dir().map_err(|error| error.to_string())?;
    runner::find_root(&here)
        .ok_or_else(|| format!("no ph-surfaces checkout at or above {}", here.display()))
}

fn context(
    root: PathBuf,
    profile: Profile,
    nightly: String,
    skip_embedded: bool,
    coverage: bool,
) -> Result<Ctx, String> {
    let config = Arc::new(Config::load(&root)?);
    Ok(Ctx {
        root,
        profile,
        nightly,
        skip_embedded,
        coverage,
        config,
    })
}

fn run_code_size(write: bool) -> Result<u8, String> {
    let ctx = context(
        current_root()?,
        Profile::Full,
        "nightly".into(),
        false,
        false,
    )?;
    let snapshot = xtask::checks::code_size::measure(&ctx)?;
    if write {
        write_snapshot(&ctx, &ctx.config.code_size.snapshot, &snapshot)?;
    } else {
        print!("{snapshot}");
    }
    Ok(0)
}

fn run_asm(write: bool) -> Result<u8, String> {
    let ctx = context(
        current_root()?,
        Profile::Full,
        "nightly".into(),
        false,
        false,
    )?;
    for (relative, snapshot) in xtask::checks::code_size::emit_asm(&ctx)? {
        if write {
            write_snapshot(&ctx, &relative, &snapshot)?;
        } else {
            print!("{snapshot}");
        }
    }
    Ok(0)
}

fn write_snapshot(ctx: &Ctx, relative: &str, contents: &str) -> Result<(), String> {
    let path = ctx.path(relative);
    std::fs::write(&path, contents).map_err(|error| error.to_string())?;
    println!("wrote {}", path.display());
    Ok(())
}

fn run_ci(args: CiArgs) -> Result<u8, String> {
    let profile = Profile::parse(&args.profile).expect("clap validated profile");
    runner::validate_release(profile, &args.only, &args.nightly)?;
    let root = match args.root {
        Some(root) => root,
        None => current_root()?,
    };
    let ctx = context(
        root,
        profile,
        args.nightly,
        args.skip_embedded,
        args.coverage,
    )?;

    println!("profile: {}", ctx.profile);
    println!("root:    {}", ctx.root.display());

    Ok(runner::run(&ctx, &ctx.config.checks, &args.only, args.fail_fast) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn cli_preserves_commands_and_repeatable_only() {
        let parsed = Cli::try_parse_from([
            "xtask",
            "ci",
            "--profile",
            "dev",
            "--only",
            "fmt",
            "--only",
            "test",
            "--coverage",
        ])
        .unwrap();
        let Some(Command::Ci(args)) = parsed.command else {
            panic!("expected ci")
        };
        assert_eq!(args.profile, "dev");
        assert_eq!(args.only, ["fmt", "test"]);
        assert!(args.coverage);
    }

    #[test]
    fn unknown_options_are_usage_errors() {
        assert!(Cli::try_parse_from(["xtask", "ci", "--unknown"]).is_err());
    }
}

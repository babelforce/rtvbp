use std::ffi::OsString;
use std::path::PathBuf;

use thiserror::Error;

use crate::emit::{Target, UnknownTarget};
use crate::write::{WriteError, check_owned_files, synchronize_files};
use crate::{GenerateError, generate};

const USAGE: &str = "usage: rtvbp-spec-gen --emit=<manifest|go|rust|typescript|docs|vectors> [--out=<dir>] [--check]\n       rtvbp-spec-gen --check";

#[derive(Debug, Error)]
pub enum CliError {
    #[error("{USAGE}")]
    Help,
    #[error("{0}\n{USAGE}")]
    Arguments(String),
    #[error(transparent)]
    Target(#[from] UnknownTarget),
    #[error(transparent)]
    Generate(#[from] GenerateError),
    #[error(transparent)]
    Write(#[from] WriteError),
}

#[derive(Debug)]
struct Options {
    target: Option<Target>,
    out_dir: Option<PathBuf>,
    check: bool,
}

pub fn run(args: impl IntoIterator<Item = OsString>) -> Result<(), CliError> {
    let options = parse(args)?;
    if let Some(target) = options.target {
        run_target(target, options.out_dir, options.check)?;
    } else {
        for target in Target::ALL {
            run_target(target, None, true)?;
        }
    }
    Ok(())
}

fn run_target(target: Target, out_dir: Option<PathBuf>, check: bool) -> Result<(), CliError> {
    let out_dir = out_dir.unwrap_or_else(|| PathBuf::from(target.canonical_out_dir()));
    let files = generate(target)?;
    if check {
        check_owned_files(&out_dir, &files, |path| target.owns_output_path(path))?;
    } else {
        synchronize_files(&out_dir, &files, |path| target.owns_output_path(path))?;
    }
    Ok(())
}

fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Options, CliError> {
    let mut args = args.into_iter();
    let mut target = None;
    let mut out_dir = None;
    let mut check = false;
    while let Some(argument) = args.next() {
        let argument = argument
            .into_string()
            .map_err(|_| CliError::Arguments("arguments must be valid UTF-8".to_owned()))?;
        if let Some(value) = argument.strip_prefix("--emit=") {
            set_once(&mut target, value.parse()?, "--emit")?;
        } else if argument == "--emit" {
            let value = next_value(&mut args, "--emit")?;
            set_once(&mut target, value.parse()?, "--emit")?;
        } else if let Some(value) = argument.strip_prefix("--out=") {
            set_once(&mut out_dir, PathBuf::from(value), "--out")?;
        } else if argument == "--out" {
            let value = next_value(&mut args, "--out")?;
            set_once(&mut out_dir, PathBuf::from(value), "--out")?;
        } else if argument == "--check" {
            check = true;
        } else if argument == "--help" || argument == "-h" {
            return Err(CliError::Help);
        } else {
            return Err(CliError::Arguments(format!(
                "unknown argument {argument:?}"
            )));
        }
    }

    match (&target, check, &out_dir) {
        (None, false, _) => {
            return Err(CliError::Arguments(
                "either --emit or --check is required".to_owned(),
            ));
        }
        (None, true, Some(_)) => {
            return Err(CliError::Arguments(
                "--out requires an explicit --emit target".to_owned(),
            ));
        }
        _ => {}
    }
    Ok(Options {
        target,
        out_dir,
        check,
    })
}

fn next_value(args: &mut impl Iterator<Item = OsString>, flag: &str) -> Result<String, CliError> {
    args.next()
        .ok_or_else(|| CliError::Arguments(format!("{flag} requires a value")))?
        .into_string()
        .map_err(|_| CliError::Arguments(format!("{flag} value must be valid UTF-8")))
}

fn set_once<T>(slot: &mut Option<T>, value: T, flag: &str) -> Result<(), CliError> {
    if slot.replace(value).is_some() {
        Err(CliError::Arguments(format!(
            "{flag} may only be supplied once"
        )))
    } else {
        Ok(())
    }
}

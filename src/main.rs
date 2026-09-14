mod lint;
mod semver;

use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    // No paths at all means read stdin, same as an explicit "-".
    let paths: Vec<&str> = if args.is_empty() {
        vec!["-"]
    } else {
        args.iter().map(String::as_str).collect()
    };

    let mut found_any = false;
    let mut had_error = false;

    for path in paths {
        match lint_path(path) {
            Ok(found) => found_any |= found,
            Err(()) => had_error = true,
        }
    }

    if had_error {
        ExitCode::from(2)
    } else if found_any {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// Lints a single path, printing any I/O error to stderr and reporting it
/// as `Err(())` rather than aborting, so one bad path in a multi-file
/// invocation doesn't stop the rest from being checked.
fn lint_path(path: &str) -> Result<bool, ()> {
    if path == "-" {
        return run(io::stdin().lock(), "<stdin>").map_err(|err| {
            eprintln!("semver-lint: {err}");
        });
    }

    let file = File::open(path).map_err(|err| {
        eprintln!("semver-lint: cannot open {path}: {err}");
    })?;
    run(BufReader::new(file), path).map_err(|err| {
        eprintln!("semver-lint: {path}: {err}");
    })
}

/// Runs the linter over `reader`, printing each finding as it is found.
/// Returns `Ok(true)` if at least one finding was reported.
fn run<R: BufRead>(reader: R, source: &str) -> io::Result<bool> {
    let mut found_any = false;
    lint::lint(reader, |finding| {
        found_any = true;
        println!("{source}:{finding}");
    })?;
    Ok(found_any)
}

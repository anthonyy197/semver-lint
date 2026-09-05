mod lint;
mod semver;

use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::process::ExitCode;

fn main() -> ExitCode {
    let path = env::args().nth(1);

    let outcome = match path.as_deref() {
        None | Some("-") => run(io::stdin().lock(), "<stdin>"),
        Some(p) => match File::open(p) {
            Ok(file) => run(BufReader::new(file), p),
            Err(err) => {
                eprintln!("semver-lint: cannot open {p}: {err}");
                return ExitCode::from(2);
            }
        },
    };

    match outcome {
        Ok(true) => ExitCode::from(1),
        Ok(false) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("semver-lint: {err}");
            ExitCode::from(2)
        }
    }
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

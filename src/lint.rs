//! Scans lines of text for tokens that look like version numbers and
//! reports the ones that fail to parse as valid semver.
//!
//! `lint` takes anything that implements `BufRead` and a callback, and
//! reads it one line at a time. Nothing about the input is buffered beyond
//! the current line, so this can run against a file of any size, or a
//! never-ending stream on stdin, in constant memory.

use std::fmt;
use std::io::{self, BufRead};

use crate::semver;

#[derive(Debug)]
pub struct Finding {
    pub line: usize,
    pub column: usize,
    pub token: String,
    pub message: String,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}: {} ({})",
            self.line, self.column, self.message, self.token
        )
    }
}

/// Reads `reader` line by line, calling `on_finding` for every malformed
/// version token encountered. Line numbers are 1-based.
pub fn lint<R: BufRead>(reader: R, mut on_finding: impl FnMut(Finding)) -> io::Result<()> {
    for (idx, line) in reader.lines().enumerate() {
        let line = line?;
        for finding in lint_line(idx + 1, &line) {
            on_finding(finding);
        }
    }
    Ok(())
}

fn lint_line(line_no: usize, line: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (column, token) in tokenize(line) {
        let candidate = strip_v_prefix(token);
        if !looks_like_version_attempt(candidate) {
            continue;
        }
        if let Err(err) = semver::parse(candidate) {
            findings.push(Finding {
                line: line_no,
                column,
                token: token.to_string(),
                message: err.to_string(),
            });
        }
    }
    findings
}

/// Splits a line into maximal runs of characters that a version string is
/// allowed to contain, returning each run with its 1-based column.
fn tokenize(line: &str) -> Vec<(usize, &str)> {
    let mut tokens = Vec::new();
    let mut start: Option<(usize, usize)> = None; // (byte offset, 1-based column)
    let mut col = 0usize;

    for (byte_idx, ch) in line.char_indices() {
        col += 1;
        if is_token_char(ch) {
            if start.is_none() {
                start = Some((byte_idx, col));
            }
        } else if let Some((start_byte, start_col)) = start.take() {
            tokens.push((start_col, &line[start_byte..byte_idx]));
        }
    }
    if let Some((start_byte, start_col)) = start {
        tokens.push((start_col, &line[start_byte..]));
    }

    tokens
}

fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '+'
}

fn strip_v_prefix(token: &str) -> &str {
    token
        .strip_prefix('v')
        .or_else(|| token.strip_prefix('V'))
        .unwrap_or(token)
}

/// Heuristic gate so we don't try to parse every stray word in a line: a
/// candidate must start with a digit and contain at least one dot, which
/// covers the shapes semver strings actually take while skipping plain
/// prose. It will also flag non-semver dotted numbers (IP addresses, dates
/// written as 2024.01.05.1) - see README for known false positives.
fn looks_like_version_attempt(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_digit()) && s.contains('.')
}

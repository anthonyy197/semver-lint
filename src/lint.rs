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
        // A `v`/`V` prefix is a deliberate signal that the author meant a
        // version, so only apply the IP/date carve-outs to bare tokens.
        let has_v_prefix = candidate.len() != token.len();
        if !has_v_prefix && (looks_like_ip_address(candidate) || looks_like_calendar_date(candidate))
        {
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
/// prose. Some non-version dotted numbers still slip through this gate -
/// IP addresses and calendar dates are filtered out separately below.
fn looks_like_version_attempt(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_digit()) && s.contains('.')
}

/// Recognizes dotted-quad IPv4 addresses: exactly four dot-separated
/// groups of one to three digits, each in 0..=255. These are common in
/// logs and config examples and would otherwise trip `TooManyCoreComponents`.
fn looks_like_ip_address(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == 4 && parts.iter().all(|p| is_octet(p))
}

fn is_octet(p: &str) -> bool {
    !p.is_empty()
        && p.len() <= 3
        && p.bytes().all(|b| b.is_ascii_digit())
        && p.parse::<u16>().is_ok_and(|n| n <= 255)
}

/// Recognizes `YYYY.MM.DD`, optionally followed by more numeric components
/// (e.g. a same-day build number). Calendar dates written this way are
/// usually zero-padded, which reads as a leading-zero semver violation even
/// though nobody meant it as a version.
fn looks_like_calendar_date(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() < 3 {
        return false;
    }
    let year_ok = parts[0].len() == 4
        && parts[0].bytes().all(|b| b.is_ascii_digit())
        && parts[0].parse::<u32>().is_ok_and(|y| (1900..=2099).contains(&y));
    year_ok && in_range(parts[1], 1, 12) && in_range(parts[2], 1, 31)
}

fn in_range(p: &str, min: u32, max: u32) -> bool {
    !p.is_empty()
        && p.len() <= 2
        && p.bytes().all(|b| b.is_ascii_digit())
        && p.parse::<u32>().is_ok_and(|n| n >= min && n <= max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_version_produces_no_finding() {
        assert!(lint_line(1, "release 1.2.3 is out").is_empty());
    }

    #[test]
    fn invalid_version_is_reported_with_location() {
        let findings = lint_line(7, "bumped to 1.02.0 today");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 7);
        assert_eq!(findings[0].column, 11);
        assert_eq!(findings[0].token, "1.02.0");
        assert!(findings[0].message.contains("leading zero"));
    }

    #[test]
    fn v_prefix_is_stripped_before_validating_but_kept_in_token() {
        let findings = lint_line(1, "see v1.2.3.4 for details");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].token, "v1.2.3.4");
        assert!(findings[0].message.contains("too many components"));
    }

    #[test]
    fn capital_v_prefix_is_also_stripped() {
        assert!(lint_line(1, "tag V1.2.3").is_empty());
    }

    #[test]
    fn words_without_a_dot_are_ignored() {
        assert!(lint_line(1, "build 42 succeeded").is_empty());
    }

    #[test]
    fn words_not_starting_with_a_digit_are_ignored() {
        // "beta.1" doesn't start with a digit, so it's skipped outright even
        // though it contains a dot.
        assert!(lint_line(1, "prefix beta.1 suffix").is_empty());
    }

    #[test]
    fn multiple_findings_on_one_line_report_distinct_columns() {
        let findings = lint_line(3, "1.02.0 then 2.0.0-beta_1");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].column, 1);
        assert_eq!(findings[1].column, 13);
    }

    #[test]
    fn ip_address_shaped_tokens_are_skipped() {
        assert!(lint_line(1, "connect to 192.168.1.1 first").is_empty());
    }

    #[test]
    fn ip_address_octet_out_of_range_is_still_checked_as_a_version() {
        // 999 can't be an IPv4 octet, so this falls through to normal
        // semver validation and gets flagged for the extra component.
        let findings = lint_line(1, "seen 999.168.1.1 in the wild");
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("too many components"));
    }

    #[test]
    fn v_prefixed_dotted_quad_is_still_checked_as_a_version() {
        // A `v` prefix means the author meant a version, not an address.
        let findings = lint_line(1, "see v192.168.1.1 for details");
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("too many components"));
    }

    #[test]
    fn calendar_date_shaped_tokens_are_skipped() {
        assert!(lint_line(1, "released on 2024.01.05 to everyone").is_empty());
    }

    #[test]
    fn calendar_date_with_trailing_build_number_is_skipped() {
        assert!(lint_line(1, "shipped as 2024.01.05.1 today").is_empty());
    }

    #[test]
    fn out_of_range_month_is_not_treated_as_a_date() {
        // Month 13 can't be a date, so this is checked as a version core
        // and rejected for the leading zero.
        let findings = lint_line(1, "bumped to 2024.13.05 today");
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("leading zero"));
    }

    #[test]
    fn trailing_punctuation_is_excluded_from_the_token() {
        let findings = lint_line(1, "see (1.02.0).");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].token, "1.02.0");
        assert_eq!(findings[0].column, 6);
    }
}

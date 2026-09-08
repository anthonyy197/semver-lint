//! A validating parser for the semver.org grammar.
//!
//! This is deliberately stricter than "does it look like a version" - it
//! exists to catch the mistakes real changelogs and manifests accumulate:
//! leading zeros, missing components, and stray characters that a loose
//! regex would wave through.

#[derive(Debug, PartialEq, Eq)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub pre_release: Option<String>,
    pub build: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SemverError {
    Empty,
    MissingMinor,
    MissingPatch,
    TooManyCoreComponents,
    EmptyNumericIdentifier,
    NonNumericCore(String),
    LeadingZero(String),
    NumberTooLarge(String),
    EmptyPrerelease,
    EmptyBuild,
    EmptyIdentifier,
    InvalidPrereleaseChar(String),
    InvalidBuildChar(String),
}

impl std::fmt::Display for SemverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SemverError::Empty => write!(f, "empty version string"),
            SemverError::MissingMinor => write!(f, "missing minor version"),
            SemverError::MissingPatch => write!(f, "missing patch version"),
            SemverError::TooManyCoreComponents => {
                write!(f, "too many components in version core")
            }
            SemverError::EmptyNumericIdentifier => write!(f, "empty numeric identifier"),
            SemverError::NonNumericCore(id) => {
                write!(f, "non-numeric identifier in version core '{id}'")
            }
            SemverError::LeadingZero(id) => write!(f, "leading zero in numeric identifier '{id}'"),
            SemverError::NumberTooLarge(id) => write!(f, "numeric identifier '{id}' overflows u64"),
            SemverError::EmptyPrerelease => write!(f, "empty pre-release component"),
            SemverError::EmptyBuild => write!(f, "empty build metadata component"),
            SemverError::EmptyIdentifier => write!(f, "empty identifier between dots"),
            SemverError::InvalidPrereleaseChar(id) => {
                write!(f, "invalid character in pre-release identifier '{id}'")
            }
            SemverError::InvalidBuildChar(id) => {
                write!(f, "invalid character in build identifier '{id}'")
            }
        }
    }
}

/// Parses a full semver string: `MAJOR.MINOR.PATCH[-PRERELEASE][+BUILD]`.
pub fn parse(input: &str) -> Result<Version, SemverError> {
    if input.is_empty() {
        return Err(SemverError::Empty);
    }

    // Build metadata is introduced by the first '+' and everything after it
    // is opaque, so split it off before we go looking for '-'.
    let (rest, build) = match input.find('+') {
        Some(idx) => (&input[..idx], Some(&input[idx + 1..])),
        None => (input, None),
    };
    let (core, pre_release) = match rest.find('-') {
        Some(idx) => (&rest[..idx], Some(&rest[idx + 1..])),
        None => (rest, None),
    };

    let mut parts = core.split('.');
    let major = parts.next().filter(|s| !s.is_empty()).unwrap_or("");
    let minor = parts.next().ok_or(SemverError::MissingMinor)?;
    let patch = parts.next().ok_or(SemverError::MissingPatch)?;
    if parts.next().is_some() {
        return Err(SemverError::TooManyCoreComponents);
    }

    let major = parse_numeric_identifier(major)?;
    let minor = parse_numeric_identifier(minor)?;
    let patch = parse_numeric_identifier(patch)?;

    if let Some(pre) = pre_release {
        validate_dotted_identifiers(pre, true)?;
    }
    if let Some(build) = build {
        validate_dotted_identifiers(build, false)?;
    }

    Ok(Version {
        major,
        minor,
        patch,
        pre_release: pre_release.map(String::from),
        build: build.map(String::from),
    })
}

fn parse_numeric_identifier(s: &str) -> Result<u64, SemverError> {
    if s.is_empty() {
        return Err(SemverError::EmptyNumericIdentifier);
    }
    if !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(SemverError::NonNumericCore(s.to_string()));
    }
    if s.len() > 1 && s.starts_with('0') {
        return Err(SemverError::LeadingZero(s.to_string()));
    }
    s.parse::<u64>()
        .map_err(|_| SemverError::NumberTooLarge(s.to_string()))
}

/// Validates a dot-separated run of identifiers (pre-release or build
/// metadata). Pre-release numeric identifiers additionally reject leading
/// zeros; build metadata identifiers do not carry that restriction.
fn validate_dotted_identifiers(s: &str, is_prerelease: bool) -> Result<(), SemverError> {
    if s.is_empty() {
        return Err(if is_prerelease {
            SemverError::EmptyPrerelease
        } else {
            SemverError::EmptyBuild
        });
    }
    for ident in s.split('.') {
        if ident.is_empty() {
            return Err(SemverError::EmptyIdentifier);
        }
        let chars_ok = ident.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
        if !chars_ok {
            return Err(if is_prerelease {
                SemverError::InvalidPrereleaseChar(ident.to_string())
            } else {
                SemverError::InvalidBuildChar(ident.to_string())
            });
        }
        if is_prerelease {
            let is_numeric = ident.bytes().all(|b| b.is_ascii_digit());
            if is_numeric && ident.len() > 1 && ident.starts_with('0') {
                return Err(SemverError::LeadingZero(ident.to_string()));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_core() {
        let v = parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert_eq!(v.pre_release, None);
        assert_eq!(v.build, None);
    }

    #[test]
    fn parses_all_zeros() {
        let v = parse("0.0.0").unwrap();
        assert_eq!((v.major, v.minor, v.patch), (0, 0, 0));
    }

    #[test]
    fn parses_prerelease_and_build() {
        let v = parse("1.2.3-alpha.1+build.5").unwrap();
        assert_eq!(v.pre_release.as_deref(), Some("alpha.1"));
        assert_eq!(v.build.as_deref(), Some("build.5"));
    }

    #[test]
    fn prerelease_alphanumeric_identifier_may_have_leading_zero() {
        // Only identifiers made entirely of digits are numeric for the
        // leading-zero rule; "0a" contains a letter so it's exempt.
        assert!(parse("1.2.3-0a").is_ok());
    }

    #[test]
    fn build_numeric_identifier_may_have_leading_zero() {
        assert!(parse("1.2.3+007").is_ok());
    }

    #[test]
    fn rejects_empty_input() {
        assert_eq!(parse(""), Err(SemverError::Empty));
    }

    #[test]
    fn rejects_missing_minor() {
        assert_eq!(parse("1"), Err(SemverError::MissingMinor));
    }

    #[test]
    fn rejects_missing_patch() {
        assert_eq!(parse("1.2"), Err(SemverError::MissingPatch));
    }

    #[test]
    fn rejects_too_many_core_components() {
        assert_eq!(parse("1.2.3.4"), Err(SemverError::TooManyCoreComponents));
    }

    #[test]
    fn rejects_empty_numeric_identifier() {
        assert_eq!(parse("1..3"), Err(SemverError::EmptyNumericIdentifier));
    }

    #[test]
    fn rejects_non_numeric_core() {
        assert_eq!(
            parse("1.x.0"),
            Err(SemverError::NonNumericCore("x".to_string()))
        );
    }

    #[test]
    fn rejects_leading_zero_in_core() {
        assert_eq!(
            parse("1.02.0"),
            Err(SemverError::LeadingZero("02".to_string()))
        );
    }

    #[test]
    fn accepts_lone_zero_component() {
        // A single "0" is not a leading zero.
        assert!(parse("1.0.3").is_ok());
    }

    #[test]
    fn rejects_number_too_large() {
        let huge = "18446744073709551616"; // u64::MAX + 1
        assert_eq!(
            parse(&format!("{huge}.0.0")),
            Err(SemverError::NumberTooLarge(huge.to_string()))
        );
    }

    #[test]
    fn rejects_empty_prerelease() {
        assert_eq!(parse("1.2.3-"), Err(SemverError::EmptyPrerelease));
    }

    #[test]
    fn rejects_empty_build() {
        assert_eq!(parse("1.2.3+"), Err(SemverError::EmptyBuild));
    }

    #[test]
    fn rejects_empty_identifier_between_dots_in_prerelease() {
        assert_eq!(
            parse("1.2.3-alpha..1"),
            Err(SemverError::EmptyIdentifier)
        );
    }

    #[test]
    fn rejects_empty_identifier_between_dots_in_build() {
        assert_eq!(parse("1.2.3+a..b"), Err(SemverError::EmptyIdentifier));
    }

    #[test]
    fn rejects_invalid_prerelease_char() {
        assert_eq!(
            parse("1.2.3-beta_1"),
            Err(SemverError::InvalidPrereleaseChar("beta_1".to_string()))
        );
    }

    #[test]
    fn rejects_invalid_build_char() {
        assert_eq!(
            parse("1.2.3+beta_1"),
            Err(SemverError::InvalidBuildChar("beta_1".to_string()))
        );
    }

    #[test]
    fn rejects_leading_zero_in_numeric_prerelease_identifier() {
        assert_eq!(
            parse("1.2.3-01"),
            Err(SemverError::LeadingZero("01".to_string()))
        );
    }
}

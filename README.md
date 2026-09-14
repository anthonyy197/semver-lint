# semver-lint

Changelogs, release notes, and version manifests are full of hand-typed
version strings, and hand-typed means occasionally wrong: a leading zero
that slipped in (`1.02.0`), a pre-release tag with an underscore instead of
a hyphen, a version missing its patch number. Most tooling that consumes
these strings assumes they already conform to
[semver.org](https://semver.org) and fails in confusing ways when they
don't. `semver-lint` scans plain text for anything that looks like an
attempted version number and reports the ones that aren't valid, with a
line number so you can go fix them.

## Usage

```
$ semver-lint CHANGELOG.md
CHANGELOG.md:12:11: leading zero in numeric identifier '02' (1.02.0)
CHANGELOG.md:30:8: too many components in version core (1.2.3.4)
CHANGELOG.md:41:1: invalid character in pre-release identifier 'beta_1' (2.0.0-beta_1)
```

Reading from stdin works the same way, using `-` or omitting the path:

```
$ cat versions.txt | semver-lint -
<stdin>:4:1: non-numeric identifier in version core 'x' (1.x.0)
```

Multiple files can be given in one invocation; each finding is prefixed
with the path it came from:

```
$ semver-lint CHANGELOG.md package.json
CHANGELOG.md:12:11: leading zero in numeric identifier '02' (1.02.0)
package.json:3:14: missing patch version (1.2)
```

Files are checked in order and a bad path doesn't stop the rest - it's
reported to stderr and the linter moves on to the next file.

Exit codes: `0` if no findings, `1` if findings were reported, `2` if any
file couldn't be opened or read. If both a finding and an I/O error occur
across the run, `2` takes priority.

## How it decides what to check

Each line is split into runs of characters a version could plausibly
contain (letters, digits, `.`, `-`, `+`). A run only gets checked if it
starts with a digit and contains a dot - that's enough to catch version
attempts without flagging every word in a paragraph. An optional leading
`v`/`V` is stripped before parsing, so `v1.2.3` is treated the same as
`1.2.3`. What's left is validated against the full semver grammar:
numeric core identifiers with no leading zeros, pre-release identifiers
restricted to alphanumerics and hyphens, and build metadata that can
contain leading zeros but nothing else non-alphanumeric.

That heuristic is intentionally loose, so two common dotted-number shapes
get an explicit carve-out before the semver check ever runs: dotted-quad
IPv4 addresses (`192.168.1.1`) and `YYYY.MM.DD` calendar dates (with or
without a trailing build number, like `2024.01.05.1`). Either carve-out is
skipped if the token has a `v`/`V` prefix, since that's a deliberate signal
that the author meant a version. Anything else that starts with a digit
and contains a dot still gets checked, so unusual but genuine calendar
versioning (`2024.1.5`, no zero-padding) is validated as a normal semver
core.

## Streaming

`semver-lint` reads its input one line at a time and reports findings as
it goes - it never buffers more than the current line, so it runs in
constant memory whether you point it at a 10-line file or a multi-gigabyte
log stream piped in over stdin.

## Building

Standard library only, no dependencies:

```
cargo build --release
```

## License

MIT, see [LICENSE](LICENSE).

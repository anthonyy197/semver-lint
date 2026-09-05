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

Exit codes: `0` if no findings, `1` if findings were reported, `2` on a
usage or I/O error (bad path, unreadable file).

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

That heuristic is intentionally loose, which means it will also flag
dotted numbers that aren't versions at all - IP addresses, dates written
as `2024.01.05`, and the like. Narrowing that down without missing real
versions is ongoing work; see the roadmap.

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

# Changelog

All notable changes are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/). Rule codes and names are part of
the public interface: a rule is never renamed or renumbered within a major
version.

## Unreleased

### Changed

- `format`: a glyph class that starts on a new line after `[` keeps the
  author's line breaks, so rows that group glyphs, or that line up with a
  parallel class, survive formatting. Only lines that are too long are
  wrapped. Classes written on one line are packed to the line width as before.
  Files formatted by 0.1 don't change.
- `format`: when a long rule needs a class split over lines, `by` stays on the
  rule's line if it fits there.

## 0.1.0 - 2026-10-13

First release.

- `format`: layout-only formatter; keeps tokens, comments, the file's indent and line endings
- `lint`: rules E001, G001, C001–C004, L001, L002, T001, X001, P001
- Projects: FEA `include()`, preprocessor `#include` / `#ifdef`, Glyphs-style manifests
- Incremental linting for editors
- `feanor.toml` settings, `# noqa` suppressions, safe and unsafe fixes
- `feanor` command: `fmt`, `lint`, `project`, `check`, `rule`

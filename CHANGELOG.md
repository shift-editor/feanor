# Changelog

All notable changes are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/). Rule codes and names are part of
the public interface: a rule is never renamed or renumbered within a major
version.

## 0.2.0 - 2026-10-13

- `symbols`: where glyph classes, mark classes, and lookups are defined and used, and
  where glyph names are written; `Symbols::at` finds the symbol under a cursor and
  `Symbols::rename` returns the edits that rename a class or lookup everywhere
- `Incremental::symbols`: the same for a document being edited

## 0.1.0 - 2026-10-13

First release.

- `format`: layout-only formatter; keeps tokens, comments, the file's indent and line endings
- `lint`: rules E001, G001, C001–C004, L001, L002, T001, X001, P001
- Projects: FEA `include()`, preprocessor `#include` / `#ifdef`, Glyphs-style manifests
- Incremental linting for editors
- `feanor.toml` settings, `# noqa` suppressions, safe and unsafe fixes
- `feanor` command: `fmt`, `lint`, `project`, `check`, `rule`

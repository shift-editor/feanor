# Changelog

All notable changes are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/). Rule codes and names are part of
the public interface: a rule is never renamed or renumbered within a major
version.

## Unreleased

First release.

- `format`: layout-only formatter; keeps tokens, comments, the file's indent and line endings
- `lint`: rules E001, G001, C001–C004, L001, L002, T001, X001, P001
- Projects: FEA `include()`, preprocessor `#include` / `#ifdef`, Glyphs-style manifests
- Incremental linting for editors
- `fea.toml` settings, `# noqa` suppressions, safe and unsafe fixes
- `fea` command: `fmt`, `lint`, `project`, `check`, `rule`

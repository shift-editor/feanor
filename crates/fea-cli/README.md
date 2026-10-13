# fea-cli

`fea`: format and lint OpenType feature files (`.fea`). Built on
[`fea-tools`](https://crates.io/crates/fea-tools) and
[fea-rs](https://docs.rs/fea-rs).

```console
$ cargo install fea-cli

$ fea fmt features.fea               # print formatted
$ fea fmt --write features.fea       # rewrite in place
$ fea fmt --check sources/*.fea      # exit 1 if anything would change

$ fea lint features.fea --glyphs MyFont-Regular.ufo
$ fea lint features.fea --fix        # apply safe fixes
$ fea lint features.fea --statistics

$ fea project sources/features.fea --all-builds   # follow includes, every #ifdef build
$ fea check sources/features.fea --format github  # CI: formatting + lint errors
$ fea rule G001                                    # explain a rule
```

Exit status: 0 when nothing was found, 1 when something was (lint errors, or
unformatted files with `--check`), 2 when a file couldn't be read.

## In GitHub Actions

```yaml
- run: cargo install fea-cli
- run: fea check sources/features.fea --format github
```

Diagnostics appear as annotations on the pull request.

## Settings

`fea` reads the nearest `fea.toml` at or above each file:

```toml
line-width = 100
indent = "auto"          # the file's own indent (default), a number of spaces, or "tab"

[lint]
ignore = ["C004"]
glyphs = "sources/MyFont-Regular.ufo"

[lint.per-file-ignores]
"classes.fea" = ["unused-class"]
```

Licensed under either of Apache-2.0 or MIT, at your option.

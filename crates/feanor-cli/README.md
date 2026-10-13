# feanor-cli

`fea`: format and lint OpenType feature files (`.fea`). Built on
[`feanor`](https://crates.io/crates/feanor) and
[fea-rs](https://docs.rs/fea-rs).

```console
$ cargo install --git https://github.com/shift-editor/feanor feanor-cli

$ feanor fmt features.fea               # print formatted
$ feanor fmt --write features.fea       # rewrite in place
$ feanor fmt --check sources/*.fea      # exit 1 if anything would change

$ feanor lint features.fea --glyphs MyFont-Regular.ufo
$ feanor lint features.fea --fix        # apply safe fixes
$ feanor lint features.fea --statistics

$ feanor project sources/features.fea --all-builds   # follow includes, every #ifdef build
$ feanor check sources/features.fea --format github  # CI: formatting + lint errors
$ feanor rule G001                                    # explain a rule
```

Exit status: 0 when nothing was found, 1 when something was (lint errors, or
unformatted files with `--check`), 2 when a file couldn't be read.

## In GitHub Actions

```yaml
- run: cargo install --git https://github.com/shift-editor/feanor feanor-cli
- run: feanor check sources/features.fea --format github
```

Diagnostics appear as annotations on the pull request.

## Settings

`feanor` reads the nearest `feanor.toml` at or above each file:

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

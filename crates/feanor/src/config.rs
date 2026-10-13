//! `feanor.toml`: settings for formatting and linting.
//!
//! ```toml
//! line-width = 100
//! indent = "auto"          # default: the file's own indent; or a number of spaces, or "tab"
//! keep-alignment = true    # re-align runs the author aligned (gofmt-style)
//!
//! [lint]
//! select = ["ALL"]         # codes (G001), families (C), or rule names
//! ignore = ["C004"]
//! glyphs = "sources/MyFont-Regular.ufo"   # a UFO, or a text file with one name per line
//! defines = ["ITALIC"]     # names for #ifdef in project linting
//!
//! [lint.per-file-ignores]
//! "classes.fea" = ["unused-class"]
//! "features/*.fea" = ["T001"]
//! ```
//!
//! Paths are relative to the directory holding `feanor.toml`.

use std::path::{Path, PathBuf};

use toml_edit::{DocumentMut, Item};

use crate::diagnostic::Selection;
use crate::fmt::{FormatOptions, Indent};

/// The settings file's name.
pub const FILE_NAME: &str = "feanor.toml";

/// Settings from a `feanor.toml`, or the defaults.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Config {
    /// Directory of the `feanor.toml`; relative paths in it resolve from here.
    pub root: PathBuf,
    /// `line-width`; see [`FormatOptions::line_width`].
    pub line_width: usize,
    /// `indent`; see [`Indent`].
    pub indent: Indent,
    /// `keep-alignment`; see [`FormatOptions::keep_alignment`].
    pub keep_alignment: bool,
    /// `lint.select`: selectors of the rules to report (all when empty).
    pub select: Vec<String>,
    /// `lint.ignore`: selectors of rules to skip everywhere.
    pub ignore: Vec<String>,
    /// `(glob, selectors)` pairs; see [`Config::selection_for`].
    pub per_file_ignores: Vec<(String, Vec<String>)>,
    /// The font's glyph list: a UFO directory or a text file of names.
    pub glyphs: Option<PathBuf>,
    /// `lint.defines`: names defined for `#ifdef` when linting a project.
    pub defines: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        let format = FormatOptions::default();
        Config {
            root: PathBuf::from("."),
            line_width: format.line_width,
            indent: format.indent,
            keep_alignment: format.keep_alignment,
            select: Vec::new(),
            ignore: Vec::new(),
            per_file_ignores: Vec::new(),
            glyphs: None,
            defines: Vec::new(),
        }
    }
}

/// A `feanor.toml` couldn't be used.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ConfigError {
    /// The file couldn't be read.
    #[error("couldn't read {}: {source}", path.display())]
    Read {
        /// The `feanor.toml`.
        path: PathBuf,
        /// Why reading failed.
        #[source]
        source: std::io::Error,
    },
    /// The file isn't valid TOML.
    #[error("{} isn't valid TOML: {source}", path.display())]
    Syntax {
        /// The `feanor.toml`.
        path: PathBuf,
        /// Where and why parsing failed.
        #[source]
        source: toml_edit::TomlError,
    },
    /// A setting has the wrong type or value.
    #[error("{}: {source}", path.display())]
    Setting {
        /// The `feanor.toml`.
        path: PathBuf,
        /// Which setting, and what it should be.
        #[source]
        source: SettingError,
    },
}

/// A setting has the wrong type or value.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("`{key}` must be {expected}")]
pub struct SettingError {
    /// The setting's dotted name, as written in `feanor.toml` (`lint.ignore`).
    pub key: &'static str,
    /// What the value should have been, in words.
    pub expected: &'static str,
}

fn invalid(key: &'static str, expected: &'static str) -> SettingError {
    SettingError { key, expected }
}

/// Why [`Config::parse`] failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ParseError {
    /// The text isn't valid TOML.
    #[error(transparent)]
    Syntax(#[from] toml_edit::TomlError),
    /// A setting has the wrong type or value.
    #[error(transparent)]
    Setting(#[from] SettingError),
}

impl Config {
    /// The nearest `feanor.toml` at or above `start` (a file or a directory), or
    /// the defaults when there is none.
    ///
    /// # Errors
    ///
    /// A [`ConfigError`] when a `feanor.toml` is found but can't be read or used.
    pub fn discover(start: &Path) -> Result<Config, ConfigError> {
        let start = start.canonicalize().unwrap_or_else(|_| start.to_path_buf());
        let mut dir = if start.is_dir() {
            Some(start.as_path())
        } else {
            start.parent()
        };
        while let Some(d) = dir {
            let candidate = d.join(FILE_NAME);
            if candidate.is_file() {
                return Config::load(&candidate);
            }
            dir = d.parent();
        }
        Ok(Config::default())
    }

    /// Read the `feanor.toml` at `path`.
    ///
    /// # Errors
    ///
    /// A [`ConfigError`] when the file can't be read, isn't TOML, or has a bad setting.
    pub fn load(path: &Path) -> Result<Config, ConfigError> {
        let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.into(),
            source,
        })?;
        let mut config = Config::parse(&text).map_err(|e| match e {
            ParseError::Syntax(source) => ConfigError::Syntax {
                path: path.into(),
                source,
            },
            ParseError::Setting(source) => ConfigError::Setting {
                path: path.into(),
                source,
            },
        })?;
        config.root = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        if let Some(g) = &config.glyphs {
            config.glyphs = Some(config.root.join(g));
        }
        Ok(config)
    }

    /// Parse `feanor.toml` text. Paths stay as written ([`Config::load`] resolves them).
    ///
    /// # Errors
    ///
    /// A [`ParseError`] for invalid TOML or a setting with the wrong type or value.
    pub fn parse(text: &str) -> Result<Config, ParseError> {
        let doc: DocumentMut = text.parse()?;
        let mut c = Config::default();
        if let Some(w) = doc.get("line-width") {
            let width = w
                .as_integer()
                .and_then(|w| usize::try_from(w).ok())
                .filter(|w| *w > 0);
            c.line_width = width.ok_or(invalid("line-width", "a positive integer"))?;
        }
        if let Some(i) = doc.get("indent") {
            let spaces = i
                .as_integer()
                .and_then(|n| usize::try_from(n).ok())
                .filter(|n| *n > 0);
            c.indent = match (spaces, i.as_str()) {
                (Some(n), _) => Indent::Spaces(n),
                (_, Some("tab")) => Indent::Tab,
                (_, Some("auto")) => Indent::Auto,
                _ => {
                    return Err(invalid("indent", "a number of spaces, \"tab\" or \"auto\"").into());
                }
            };
        }
        if let Some(a) = doc.get("keep-alignment") {
            c.keep_alignment = a
                .as_bool()
                .ok_or(invalid("keep-alignment", "true or false"))?;
        }
        let Some(lint) = doc.get("lint") else {
            return Ok(c);
        };
        c.select = strings(lint.get("select"), "lint.select")?;
        c.ignore = strings(lint.get("ignore"), "lint.ignore")?;
        c.defines = strings(lint.get("defines"), "lint.defines")?;
        if let Some(g) = lint.get("glyphs") {
            c.glyphs = Some(PathBuf::from(
                g.as_str().ok_or(invalid("lint.glyphs", "a path"))?,
            ));
        }
        if let Some(table) = lint.get("per-file-ignores").and_then(Item::as_table_like) {
            for (pattern, rules) in table.iter() {
                c.per_file_ignores.push((
                    pattern.to_string(),
                    strings(Some(rules), "lint.per-file-ignores")?,
                ));
            }
        }
        Ok(c)
    }

    /// The format settings.
    pub fn format_options(&self) -> FormatOptions {
        FormatOptions {
            line_width: self.line_width,
            indent: self.indent.clone(),
            keep_alignment: self.keep_alignment,
        }
    }

    /// The rules to report for the file at `path`: `select`, minus `ignore`
    /// and every `per-file-ignores` entry whose glob matches the path relative
    /// to [`Config::root`]. A glob without `/` matches the file name anywhere.
    pub fn selection_for(&self, path: &Path) -> Selection {
        let relative = path
            .strip_prefix(&self.root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let mut ignore = self.ignore.clone();
        for (pattern, rules) in &self.per_file_ignores {
            if glob_match(pattern, &relative) {
                ignore.extend(rules.iter().cloned());
            }
        }
        Selection {
            select: self.select.clone(),
            ignore,
        }
    }
}

fn strings(item: Option<&Item>, key: &'static str) -> Result<Vec<String>, SettingError> {
    let Some(item) = item else {
        return Ok(Vec::new());
    };
    let not_strings = || invalid(key, "a list of strings");
    let array = item.as_array().ok_or_else(not_strings)?;
    array
        .iter()
        .map(|v| v.as_str().map(String::from).ok_or_else(not_strings))
        .collect()
}

/// `*` matches within one path segment, `**` across segments; a pattern
/// without `/` matches the file name anywhere.
fn glob_match(pattern: &str, path: &str) -> bool {
    if !pattern.contains('/') {
        let name = path.rsplit('/').next().unwrap_or(path);
        return glob(pattern.as_bytes(), name.as_bytes());
    }
    glob(pattern.as_bytes(), path.as_bytes())
}

fn glob(p: &[u8], s: &[u8]) -> bool {
    match (p.first(), s.first()) {
        (None, None) => true,
        (Some(b'*'), _) if p.get(1) == Some(&b'*') => {
            let rest = p[2..].strip_prefix(b"/").unwrap_or(&p[2..]);
            (0..=s.len()).any(|i| glob(rest, &s[i..]))
        }
        (Some(b'*'), _) => (0..=s.len())
            .take_while(|&i| i == 0 || s[i - 1] != b'/')
            .any(|i| glob(&p[1..], &s[i..])),
        (Some(a), Some(b)) if a == b => glob(&p[1..], &s[1..]),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_setting() {
        let c = Config::parse(
            r#"
            line-width = 90
            indent = "tab"
            keep-alignment = false
            [lint]
            select = ["ALL"]
            ignore = ["C004"]
            glyphs = "Font.ufo"
            defines = ["ITALIC"]
            [lint.per-file-ignores]
            "classes.fea" = ["unused-class"]
            "#,
        )
        .unwrap();
        assert_eq!(c.line_width, 90);
        assert_eq!(c.indent, Indent::Tab);
        assert!(!c.keep_alignment);
        assert_eq!(c.ignore, ["C004"]);
        assert_eq!(c.glyphs.as_deref(), Some(Path::new("Font.ufo")));
        assert_eq!(c.defines, ["ITALIC"]);
        assert_eq!(
            c.per_file_ignores,
            [("classes.fea".to_string(), vec!["unused-class".to_string()])]
        );
    }

    #[test]
    fn defaults_to_the_files_own_indent() {
        assert_eq!(Config::parse("").unwrap().indent, Indent::Auto);
    }

    #[test]
    fn names_the_setting_that_is_wrong() {
        let err = Config::parse("indent = \"wide\"").unwrap_err();
        assert!(matches!(
            err,
            ParseError::Setting(SettingError { key: "indent", .. })
        ));
        let err = Config::parse("line-width = -3").unwrap_err();
        assert!(matches!(
            err,
            ParseError::Setting(SettingError {
                key: "line-width",
                ..
            })
        ));
        let err = Config::parse("[lint]\nignore = \"C004\"").unwrap_err();
        assert!(matches!(
            err,
            ParseError::Setting(SettingError {
                key: "lint.ignore",
                ..
            })
        ));
        assert!(matches!(
            Config::parse("line-width = ").unwrap_err(),
            ParseError::Syntax(_)
        ));
    }

    #[test]
    fn per_file_ignores_match_names_and_globs() {
        let mut c = Config::parse(
            r#"
            [lint]
            ignore = ["X001"]
            [lint.per-file-ignores]
            "classes.fea" = ["C004"]
            "features/*.fea" = ["T001"]
            "#,
        )
        .unwrap();
        c.root = PathBuf::from("/proj");
        assert_eq!(
            c.selection_for(Path::new("/proj/sources/classes.fea"))
                .ignore,
            ["X001", "C004"]
        );
        assert_eq!(
            c.selection_for(Path::new("/proj/features/calt.fea")).ignore,
            ["X001", "T001"]
        );
        assert_eq!(
            c.selection_for(Path::new("/proj/features/deep/calt.fea"))
                .ignore,
            ["X001"]
        );
    }

    #[test]
    fn double_star_crosses_directories() {
        assert!(glob_match("src/**/*.fea", "src/a/b/c.fea"));
        assert!(glob_match("src/**/*.fea", "src/c.fea"));
        assert!(!glob_match("src/*.fea", "src/a/c.fea"));
    }
}

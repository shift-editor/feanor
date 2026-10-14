//! Project assembly: many sources, one document, one source map.
//!
//! Real feature code is split three ways, and all three reduce to an ordered
//! list of text segments:
//! - FEA `include(path);` statements (Recursive): the statement is replaced by
//!   the included file;
//! - C-preprocessor lines, `#include "path"` with `#ifdef`/`#ifndef`/`#else`/
//!   `#endif` (Amiri): FEA sees them as comments, the build expands them;
//! - Glyphs-style sections (classes, prefixes, per-feature bodies): a manifest
//!   lists the chunks in order, and a body is wrapped in `feature tag { }`.
//!
//! [`Project::lint`] lints the assembled document once and maps every
//! diagnostic back to a file and a byte range in it. Generated text (a Glyphs
//! class's `@Name = [`, a feature wrapper) belongs to the chunk it wraps.

use std::collections::{BTreeSet, HashSet};
use std::fmt;
use std::ops::Range;
use std::path::{Path, PathBuf};

use crate::diagnostic::{Diagnostic, Rule, Severity};
use crate::lint::{LintOptions, Suggestions, lint_document};

/// A project couldn't be assembled.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ProjectError {
    /// A file exists but couldn't be read.
    #[error("couldn't read {}: {source}", path.display())]
    Read {
        /// The file.
        path: PathBuf,
        /// Why reading failed.
        #[source]
        source: std::io::Error,
    },
}

/// Read a project file; the one place a read failure becomes a [`ProjectError`].
fn read(path: &Path) -> Result<String, ProjectError> {
    std::fs::read_to_string(path).map_err(|source| ProjectError::Read {
        path: path.to_path_buf(),
        source,
    })
}

/// The file's canonical path, which identifies it when includes are followed.
/// Where the platform can't resolve paths (WASI in a browser), the path as
/// given, made absolute when possible, as long as the file exists.
fn canonical(path: &Path) -> Result<PathBuf, ProjectError> {
    let error = |source| ProjectError::Read {
        path: path.to_path_buf(),
        source,
    };
    match path.canonicalize() {
        Err(e) if e.kind() == std::io::ErrorKind::Unsupported => {
            std::fs::metadata(path).map_err(error)?;
            Ok(std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()))
        }
        result => result.map_err(error),
    }
}

/// Something wrong with how the project's files fit together. The project
/// still assembles; the offending include or directive is skipped.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProblemKind {
    /// No file at the included path.
    MissingInclude {
        /// The path as written.
        target: String,
    },
    /// The include would loop back to a file already being included, or nest
    /// more than 50 levels deep.
    IncludeCycle {
        /// The path as written.
        target: String,
    },
    /// `#else` with no `#ifdef` open.
    ElseWithoutIfdef,
    /// `#endif` with no `#ifdef` open.
    EndifWithoutIfdef,
    /// An `#ifdef` still open at the end of the file.
    UnclosedIfdef,
}

impl fmt::Display for ProblemKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProblemKind::MissingInclude { target } => {
                write!(f, "included file '{target}' not found")
            }
            ProblemKind::IncludeCycle { target } => {
                write!(f, "include of '{target}' is cyclic or too deep")
            }
            ProblemKind::ElseWithoutIfdef => f.write_str("#else without #ifdef"),
            ProblemKind::EndifWithoutIfdef => f.write_str("#endif without #ifdef"),
            ProblemKind::UnclosedIfdef => f.write_str("unclosed #ifdef"),
        }
    }
}

/// A [`ProblemKind`] at a place in one of the project's files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    /// Index into [`Project::files`].
    pub file: usize,
    /// Byte range in that file.
    pub range: Range<usize>,
    /// What's wrong.
    pub kind: ProblemKind,
}

/// A diagnostic in one of a project's files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileDiagnostic {
    /// Index into [`Project::files`].
    pub file: usize,
    /// Ranges (including fixes and related locations) are byte ranges in that file.
    pub diagnostic: Diagnostic,
}

#[derive(Debug)]
struct Segment {
    /// Index into `Project::files`, or `None` for synthetic text.
    pub file: Option<usize>,
    pub combined: Range<usize>,
    /// Byte offset in the file where this segment starts.
    pub local_start: usize,
    /// For synthetic wrapper text: the chunk it was generated for.
    pub owner: Option<usize>,
}

/// Many feature files assembled into one document, with a map back to each file.
///
/// ```no_run
/// use feanor::{LintOptions, Project};
///
/// let project = Project::from_root("sources/features.fea".as_ref(), &["ITALIC".to_string()])?;
/// for found in project.lint(&LintOptions::default()) {
///     println!("{}: {}", project.files()[found.file].display(), found.diagnostic.message);
/// }
/// # Ok::<(), feanor::ProjectError>(())
/// ```
#[derive(Debug, Default)]
pub struct Project {
    files: Vec<PathBuf>,
    texts: Vec<String>,
    combined: String,
    segments: Vec<Segment>,
    problems: Vec<Problem>,
    current_owner: Option<usize>,
    condition_names: BTreeSet<String>,
}

const MAX_DEPTH: usize = 50;

impl Project {
    /// Assemble from a root file, following FEA `include()` and preprocessor
    /// `#include`, with `defines` set for `#ifdef`. Includes resolve relative to
    /// the including file, then to the root file's directory.
    ///
    /// # Errors
    ///
    /// [`ProjectError::Read`] when the root or an included file exists but
    /// can't be read. A missing include is a [`Problem`], not an error.
    pub fn from_root(root: &Path, defines: &[String]) -> Result<Project, ProjectError> {
        let mut project = Project::default();
        let defines: HashSet<&str> = defines.iter().map(String::as_str).collect();
        let base = root.parent().unwrap_or(Path::new(".")).to_path_buf();
        project.expand(root, &base, &defines, &mut Vec::new())?;
        Ok(project)
    }

    /// Assemble from a manifest listing chunks in order, one per line, relative
    /// to the manifest: `path`, `path feature:tag` (a feature body, wrapped in
    /// `feature tag { }`), or `path class:Name` (a class's glyph list, wrapped
    /// in `@Name = [ ];`). This is how Glyphs sources store feature code.
    /// Lines starting with `#` are comments.
    ///
    /// # Errors
    ///
    /// [`ProjectError::Read`] when the manifest or a listed chunk can't be read.
    pub fn from_manifest(manifest: &Path) -> Result<Project, ProjectError> {
        let mut project = Project::default();
        let dir = manifest.parent().unwrap_or(Path::new(".")).to_path_buf();
        for line in read(manifest)?.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // `path feature:tag` wraps a feature body; `path class:Name` wraps a class's glyph list
            let (path, open, close) = if let Some((p, tag)) = line.split_once(" feature:") {
                let tag = tag.trim();
                (
                    p.trim(),
                    format!("feature {tag} {{\n"),
                    format!("\n}} {tag};\n"),
                )
            } else if let Some((p, name)) = line.split_once(" class:") {
                (
                    p.trim(),
                    format!("@{} = [", name.trim()),
                    "];\n".to_string(),
                )
            } else {
                (line, String::new(), String::new())
            };
            let text = read(&dir.join(path))?;
            let idx = project.add_file(dir.join(path), text);
            project.current_owner = Some(idx);
            if !open.is_empty() {
                project.push_synthetic(&open);
            }
            let len = project.texts[idx].len();
            project.push_file(idx, 0..len);
            if !close.is_empty() {
                project.push_synthetic(&close);
            }
            project.current_owner = None;
        }
        Ok(project)
    }

    fn add_file(&mut self, path: PathBuf, text: String) -> usize {
        self.files.push(path);
        self.texts.push(text);
        self.files.len() - 1
    }

    fn push_file(&mut self, file: usize, range: Range<usize>) {
        if range.is_empty() {
            return;
        }
        let start = self.combined.len();
        self.combined.push_str(&self.texts[file][range.clone()]);
        if !self.combined.ends_with('\n') && range.end == self.texts[file].len() {
            self.push_segment(Some(file), start, range.start);
            self.push_synthetic("\n");
            return;
        }
        self.push_segment(Some(file), start, range.start);
    }

    fn push_segment(&mut self, file: Option<usize>, start: usize, local_start: usize) {
        let end = self.combined.len();
        // extend the previous segment when the text continues it
        if let Some(last) = self.segments.last_mut() {
            let contiguous = last.file == file
                && last.owner == self.current_owner
                && last.combined.end == start
                && (file.is_none()
                    || last.local_start + (last.combined.end - last.combined.start) == local_start);
            if contiguous {
                last.combined.end = end;
                return;
            }
        }
        self.segments.push(Segment {
            file,
            combined: start..end,
            local_start,
            owner: self.current_owner,
        });
    }

    fn push_synthetic(&mut self, text: &str) {
        let start = self.combined.len();
        self.combined.push_str(text);
        self.push_segment(None, start, 0);
    }

    fn expand(
        &mut self,
        path: &Path,
        base: &Path,
        defines: &HashSet<&str>,
        stack: &mut Vec<PathBuf>,
    ) -> Result<(), ProjectError> {
        let canonical = canonical(path)?;
        let text = read(path)?;
        let file = self.add_file(path.to_path_buf(), text);
        stack.push(canonical);

        // cpp-style conditions: each entry is "this branch is active"
        let mut active: Vec<bool> = Vec::new();
        let is_active = |a: &Vec<bool>| a.iter().all(|x| *x);
        let mut at = 0;
        let mut emitted_to = 0;
        let lines: Vec<(usize, String)> = self.texts[file]
            .split_inclusive('\n')
            .map(|l| {
                let start = at;
                at += l.len();
                (start, l.to_string())
            })
            .collect();

        for (start, line) in lines {
            let end = start + line.len();
            let trimmed = line.trim();
            // a directive is `#keyword` with no space; `# else …` is an ordinary comment
            let directive = trimmed
                .strip_prefix('#')
                .filter(|d| d.starts_with(|c: char| c.is_ascii_lowercase()));

            match directive.and_then(|d| {
                d.split_once(char::is_whitespace)
                    .map(|(k, v)| (k, v.trim()))
                    .or(Some((d, "")))
            }) {
                Some((kw @ ("ifdef" | "ifndef"), name)) => {
                    self.condition_names.insert(name.to_string());
                    let defined = defines.contains(name);
                    active.push(if kw == "ifdef" { defined } else { !defined });
                    continue_line(self, file, &mut emitted_to, start, end, true);
                    continue;
                }
                Some(("else", _)) => {
                    match active.last_mut() {
                        Some(top) => *top = !*top,
                        None => self.problem(file, start..end, ProblemKind::ElseWithoutIfdef),
                    }
                    continue_line(self, file, &mut emitted_to, start, end, true);
                    continue;
                }
                Some(("endif", _)) => {
                    if active.pop().is_none() {
                        self.problem(file, start..end, ProblemKind::EndifWithoutIfdef);
                    }
                    continue_line(self, file, &mut emitted_to, start, end, true);
                    continue;
                }
                _ => {}
            }

            if !is_active(&active) {
                // skip inactive lines entirely: they map to nothing
                self.flush(file, &mut emitted_to, start);
                emitted_to = end;
                continue;
            }

            if let Some(target) = directive
                .and_then(|d| d.strip_prefix("include"))
                .map(str::trim)
                .and_then(quoted)
            {
                // keep the comment line, then splice the file in after it
                self.flush(file, &mut emitted_to, end);
                self.include(file, start..end, &target, path, base, defines, stack)?;
                continue;
            }

            if let Some((stmt, target)) = fea_include(&line) {
                let stmt = start + stmt.start..start + stmt.end;
                self.flush(file, &mut emitted_to, stmt.start);
                emitted_to = stmt.end;
                self.include(file, stmt, &target, path, base, defines, stack)?;
            }
        }
        if !active.is_empty() {
            let len = self.texts[file].len();
            self.problem(file, len..len, ProblemKind::UnclosedIfdef);
        }
        let len = self.texts[file].len();
        self.flush(file, &mut emitted_to, len);
        stack.pop();
        return Ok(());

        fn continue_line(
            project: &mut Project,
            file: usize,
            emitted_to: &mut usize,
            _start: usize,
            end: usize,
            _directive: bool,
        ) {
            project.flush(file, emitted_to, end);
        }
    }

    fn problem(&mut self, file: usize, range: Range<usize>, kind: ProblemKind) {
        self.problems.push(Problem { file, range, kind });
    }

    fn flush(&mut self, file: usize, emitted_to: &mut usize, upto: usize) {
        if upto > *emitted_to {
            self.push_file(file, *emitted_to..upto);
            *emitted_to = upto;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn include(
        &mut self,
        from: usize,
        stmt: Range<usize>,
        target: &str,
        including: &Path,
        base: &Path,
        defines: &HashSet<&str>,
        stack: &mut Vec<PathBuf>,
    ) -> Result<(), ProjectError> {
        let dir = including.parent().unwrap_or(Path::new("."));
        // the spec resolves relative to the including file; UFO builds use the project root
        let candidates = [dir.join(target), base.join(target)];
        let Some(found) = candidates.iter().find(|p| p.is_file()) else {
            self.problem(
                from,
                stmt,
                ProblemKind::MissingInclude {
                    target: target.to_string(),
                },
            );
            return Ok(());
        };
        if stack.len() >= MAX_DEPTH || stack.contains(&canonical(found)?) {
            self.problem(
                from,
                stmt,
                ProblemKind::IncludeCycle {
                    target: target.to_string(),
                },
            );
            return Ok(());
        }
        if !self.combined.ends_with('\n') {
            self.push_synthetic("\n");
        }
        self.expand(found, base, defines, stack)
    }

    /// The project's files, in the order they were first included.
    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }

    /// The text of file `file` (an index into [`Self::files`]).
    ///
    /// # Panics
    ///
    /// Panics when `file` is out of range.
    pub fn text(&self, file: usize) -> &str {
        &self.texts[file]
    }

    /// The assembled document that's linted.
    pub fn combined(&self) -> &str {
        &self.combined
    }

    /// Problems found while assembling.
    pub fn problems(&self) -> &[Problem] {
        &self.problems
    }

    /// Every name tested by `#ifdef` / `#ifndef` in the files this assembly read.
    pub fn condition_names(&self) -> &BTreeSet<String> {
        &self.condition_names
    }

    /// Lint the whole project as one document (so [`Rule::UndefinedClass`] and
    /// [`Rule::UndefinedLookup`] apply) and map each diagnostic to its file.
    /// Assembly [`Problem`]s are included as [`Rule::Project`] errors.
    /// A fix or related location that would cross into another file is dropped.
    pub fn lint(&self, options: &LintOptions) -> Vec<FileDiagnostic> {
        let options = LintOptions {
            project: true,
            ..*options
        };
        let mut out = Vec::new();
        for mut d in lint_document(&self.combined, &options, &mut Suggestions::new()) {
            let Some((file, range)) = self.attribute(&d.range) else {
                continue;
            };
            d.range = range;
            let fixes_stay_in_file = d
                .fixes
                .iter()
                .flat_map(|f| &f.edits)
                .all(|e| matches!(self.locate_range(&e.range), Some((f, _)) if f == file));
            if fixes_stay_in_file {
                for e in d.fixes.iter_mut().flat_map(|f| f.edits.iter_mut()) {
                    e.range = self
                        .locate_range(&e.range)
                        .map_or(e.range.clone(), |(_, r)| r);
                }
            } else {
                d.fixes.clear();
            }
            d.related = d
                .related
                .take()
                .and_then(|mut r| match self.locate_range(&r.range) {
                    Some((f, range)) if f == file => {
                        r.range = range;
                        Some(r)
                    }
                    _ => None,
                });
            out.push(FileDiagnostic {
                file,
                diagnostic: d,
            });
        }
        for p in &self.problems {
            let d = Diagnostic::new(
                Rule::Project,
                Severity::Error,
                p.range.clone(),
                p.kind.to_string(),
            );
            out.push(FileDiagnostic {
                file: p.file,
                diagnostic: d,
            });
        }
        out.sort_by_key(|f| (f.file, f.diagnostic.range.start));
        out
    }

    /// Map a byte offset in [`Self::combined`] to a file and an offset in it;
    /// `None` for generated text.
    pub fn locate(&self, at: usize) -> Option<(usize, usize)> {
        let i = self.segments.partition_point(|s| s.combined.end <= at);
        let seg = self.segments.get(i)?;
        if !seg.combined.contains(&at) && (seg.combined.end != at) {
            return None;
        }
        Some((seg.file?, seg.local_start + (at - seg.combined.start)))
    }

    /// Like [`Self::locate_range`], but generated wrapper text (a Glyphs
    /// class's `@Name = [`) maps to the start of the chunk it wraps.
    pub fn attribute(&self, r: &Range<usize>) -> Option<(usize, Range<usize>)> {
        if let Some(found) = self.locate_range(r) {
            return Some(found);
        }
        let i = self.segments.partition_point(|s| s.combined.end <= r.start);
        let owner = self.segments.get(i)?.owner?;
        Some((owner, 0..0))
    }

    /// Map a byte range in [`Self::combined`] to a file and a range in it;
    /// `None` when it touches generated text or spans two files.
    pub fn locate_range(&self, r: &Range<usize>) -> Option<(usize, Range<usize>)> {
        let (file, start) = self.locate(r.start)?;
        let end_probe = if r.end > r.start { r.end - 1 } else { r.end };
        let (file_end, end) = self.locate(end_probe)?;
        if file != file_end {
            return None;
        }
        let end = if r.end > r.start { end + 1 } else { end };
        Some((file, start..end))
    }
}

/// `"path"` or `<path>` → path.
fn quoted(s: &str) -> Option<String> {
    let s = s.trim();
    let inner = s
        .strip_prefix('"')
        .and_then(|x| x.strip_suffix('"'))
        .or_else(|| s.strip_prefix('<').and_then(|x| x.strip_suffix('>')))?;
    Some(inner.to_string())
}

/// An FEA `include(path);` statement on this line, outside a comment.
fn fea_include(line: &str) -> Option<(Range<usize>, String)> {
    let code_end = line.find('#').unwrap_or(line.len());
    let code = &line[..code_end];
    let start = code.find("include")?;
    let before_ok = start == 0
        || !code.as_bytes()[start - 1].is_ascii_alphanumeric()
            && code.as_bytes()[start - 1] != b'_';
    if !before_ok {
        return None;
    }
    let rest = &code[start + "include".len()..];
    let open = rest.find('(')?;
    if !rest[..open].trim().is_empty() {
        return None;
    }
    let close = rest.find(')')?;
    let target = rest[open + 1..close].trim().to_string();
    let mut end = start + "include".len() + close + 1;
    let tail = &code[end..];
    if let Some(semi) = tail.find(';').filter(|i| tail[..*i].trim().is_empty()) {
        end += semi + 1;
    }
    Some((start..end, target))
}

/// A build configuration: the names defined for `#ifdef`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BuildConfig {
    /// Names defined for `#ifdef`, in the order given.
    pub defines: Vec<String>,
}

impl fmt::Display for BuildConfig {
    /// `default build`, or the defines as `-D NAME` flags.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.defines.is_empty() {
            return f.write_str("default build");
        }
        let flags: Vec<String> = self.defines.iter().map(|d| format!("-D {d}")).collect();
        f.write_str(&flags.join(" "))
    }
}

/// Every build configuration of the project at `root` worth linting, each
/// including `base_defines`.
///
/// Names tested by `#ifdef` are discovered until no new ones appear (a file
/// included only under `#ifdef A` may test `B`). With up to four names, every
/// combination is returned; with more, the default, each name alone, and all
/// of them together.
///
/// # Errors
///
/// [`ProjectError::Read`] when a file can't be read.
pub fn configurations(
    root: &Path,
    base_defines: &[String],
) -> Result<Vec<BuildConfig>, ProjectError> {
    let mut names: BTreeSet<String> = BTreeSet::new();
    loop {
        let before = names.len();
        let probe: Vec<String> = names
            .iter()
            .cloned()
            .chain(base_defines.iter().cloned())
            .collect();
        for defines in [base_defines.to_vec(), probe] {
            names.extend(Project::from_root(root, &defines)?.condition_names);
        }
        if names.len() == before {
            break;
        }
    }
    let names: Vec<String> = names
        .into_iter()
        .filter(|n| !base_defines.contains(n))
        .collect();
    let mut configs = Vec::new();
    if names.len() <= 4 {
        for mask in 0..(1u32 << names.len()) {
            let set = names
                .iter()
                .enumerate()
                .filter(|(i, _)| mask & (1 << i) != 0)
                .map(|(_, n)| n.clone());
            configs.push(BuildConfig {
                defines: base_defines.iter().cloned().chain(set).collect(),
            });
        }
    } else {
        configs.push(BuildConfig {
            defines: base_defines.to_vec(),
        });
        for n in &names {
            configs.push(BuildConfig {
                defines: base_defines.iter().cloned().chain([n.clone()]).collect(),
            });
        }
        configs.push(BuildConfig {
            defines: base_defines
                .iter()
                .cloned()
                .chain(names.iter().cloned())
                .collect(),
        });
    }
    Ok(configs)
}

impl Project {
    /// Whether byte `local` of the file at `path` is part of the assembled
    /// text (and not, say, inside an inactive `#ifdef` branch).
    pub fn contains(&self, path: &Path, local: usize) -> bool {
        self.segments.iter().any(|s| {
            s.file.is_some_and(|f| self.files[f] == path)
                && (s.local_start..s.local_start + (s.combined.end - s.combined.start))
                    .contains(&local)
        })
    }
}

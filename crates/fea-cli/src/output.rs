//! Printing diagnostics: terminal, GitHub annotations, JSON; and diffs.

use std::collections::BTreeMap;
use std::io::IsTerminal;

use anstyle::{AnsiColor, Style};
use fea_tools::{Applicability, Diagnostic, LineIndex, Rule, Severity};
use similar::{ChangeTag, TextDiff};

use crate::OutputFormat;

fn painter() -> impl Fn(Style, &str) -> String {
    let color = std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    move |style: Style, s: &str| {
        if color {
            format!("{style}{s}{style:#}")
        } else {
            s.to_string()
        }
    }
}

pub(crate) fn print_diagnostic(format: OutputFormat, path: &str, text: &str, d: &Diagnostic) {
    match format {
        OutputFormat::Github => print_github(path, text, d),
        OutputFormat::Json => println!("{}", json(path, text, d)),
        OutputFormat::Human => print_human(path, text, d),
    }
}

/// rustc-style: header, location, the source line with a caret underline, fixes.
fn print_human(path: &str, text: &str, d: &Diagnostic) {
    let paint = painter();
    let (style, label) = match d.severity {
        Severity::Error => (
            Style::new().bold().fg_color(Some(AnsiColor::Red.into())),
            "error",
        ),
        Severity::Warning => (
            Style::new().bold().fg_color(Some(AnsiColor::Yellow.into())),
            "warning",
        ),
        _ => (
            Style::new().bold().fg_color(Some(AnsiColor::Cyan.into())),
            "hint",
        ),
    };
    let dim = Style::new().fg_color(Some(AnsiColor::Blue.into()));
    let index = LineIndex::new(text);
    let start = d.range.start.min(text.len());
    let line_no = index.position(start).line as usize;
    let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
    let line = text[line_start..].lines().next().unwrap_or("");
    let col = text[line_start..start].chars().count();
    let underline = text[start..d.range.end.min(line_start + line.len()).max(start)]
        .chars()
        .count()
        .max(1);
    let gutter = (line_no + 1).to_string().len();

    println!(
        "{}{}",
        paint(
            style,
            &format!("{label}[{} {}]", d.rule.code(), d.rule.name())
        ),
        paint(Style::new().bold(), &format!(": {}", d.message))
    );
    println!(
        "{:gutter$}{} {path}:{}:{}",
        "",
        paint(dim, "-->"),
        line_no + 1,
        col + 1
    );
    println!("{:gutter$} {}", "", paint(dim, "|"));
    let shown: String = line.chars().take(120).collect();
    println!(
        "{} {} {shown}",
        paint(dim, &(line_no + 1).to_string()),
        paint(dim, "|")
    );
    let width = underline.min(120usize.saturating_sub(col).max(1));
    println!(
        "{:gutter$} {} {:col$}{}",
        "",
        paint(dim, "|"),
        "",
        paint(style, &"^".repeat(width))
    );
    for f in &d.fixes {
        let kind = if f.applicability == Applicability::Safe {
            "fix"
        } else {
            "unsafe fix"
        };
        println!("{:gutter$} {} {kind}: {}", "", paint(dim, "="), f.title);
    }
    if let Some(related) = &d.related {
        let line = index.position(related.range.start).line + 1;
        println!(
            "{:gutter$} {} note: {} at line {line}",
            "",
            paint(dim, "="),
            related.label
        );
    }
    println!();
}

/// GitHub Actions workflow command; shows up as an annotation on the pull request.
fn print_github(path: &str, text: &str, d: &Diagnostic) {
    let level = match d.severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        _ => "notice",
    };
    let index = LineIndex::new(text);
    let start = index.position(d.range.start);
    let end = index.position(d.range.end);
    let escape = |s: &str| {
        s.replace('%', "%25")
            .replace('\r', "%0D")
            .replace('\n', "%0A")
    };
    println!(
        "::{level} file={path},line={},endLine={},col={},title={} {}::{}",
        start.line + 1,
        end.line + 1,
        start.character + 1,
        d.rule.code(),
        d.rule.name(),
        escape(&d.message)
    );
}

/// One diagnostic as JSON, with LSP-style zero-based UTF-16 positions.
pub(crate) fn json(path: &str, text: &str, d: &Diagnostic) -> serde_json::Value {
    let index = LineIndex::new(text);
    let range = |r: &std::ops::Range<usize>| serde_json::json!({ "start": index.position(r.start), "end": index.position(r.end) });
    serde_json::json!({
        "file": path,
        "rule": d.rule.name(),
        "code": d.rule.code(),
        "severity": d.severity,
        "range": range(&d.range),
        "message": d.message,
        "fixes": d.fixes.iter().map(|f| serde_json::json!({
            "title": f.title,
            "applicability": f.applicability,
            "edits": f.edits.iter().map(|e| serde_json::json!({ "range": range(&e.range), "newText": e.new_text })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "related": d.related.as_ref().map(|r| serde_json::json!({ "range": range(&r.range), "label": r.label })),
    })
}

/// Counts per rule, most frequent first, with how many have a safe fix.
pub(crate) fn print_statistics<'a>(diags: impl Iterator<Item = &'a Diagnostic>) {
    let mut counts: BTreeMap<Rule, (usize, usize)> = BTreeMap::new();
    for d in diags {
        let entry = counts.entry(d.rule).or_default();
        entry.0 += 1;
        entry.1 += usize::from(
            d.fixes
                .iter()
                .any(|f| f.applicability == Applicability::Safe),
        );
    }
    let mut rows: Vec<_> = counts.into_iter().collect();
    rows.sort_by(|a, b| b.1.0.cmp(&a.1.0));
    for (rule, (n, fixable)) in rows {
        let mark = if fixable > 0 {
            format!("  [{fixable} fixable]")
        } else {
            String::new()
        };
        println!("{n:>6}  {:<5} {}{mark}", rule.code(), rule.name());
    }
}

/// A unified diff, coloured on a terminal.
pub(crate) fn print_diff(name: &str, before: &str, after: &str) {
    let paint = painter();
    let red = Style::new().fg_color(Some(AnsiColor::Red.into()));
    let green = Style::new().fg_color(Some(AnsiColor::Green.into()));
    let cyan = Style::new().fg_color(Some(AnsiColor::Cyan.into()));
    let dim = Style::new().dimmed();
    let diff = TextDiff::from_lines(before, after);
    if diff.ratio() == 1.0 {
        return;
    }
    println!(
        "{}",
        paint(dim, &format!("--- {name}\n+++ {name} (formatted)"))
    );
    for group in diff.grouped_ops(2) {
        let (Some(first), Some(last)) = (group.first(), group.last()) else {
            continue;
        };
        let (o, n) = (first.old_range().start, first.new_range().start);
        let (ol, nl) = (last.old_range().end - o, last.new_range().end - n);
        println!(
            "{}",
            paint(cyan, &format!("@@ -{},{ol} +{},{nl} @@", o + 1, n + 1))
        );
        for op in group {
            for change in diff.iter_changes(&op) {
                let line = change.value().trim_end_matches('\n');
                match change.tag() {
                    ChangeTag::Delete => println!("{}", paint(red, &format!("-{line}"))),
                    ChangeTag::Insert => println!("{}", paint(green, &format!("+{line}"))),
                    ChangeTag::Equal => println!("{}", paint(dim, &format!(" {line}"))),
                }
            }
        }
    }
}

//! Aligned runs: keep column alignment the author chose, with columns recomputed.
//!
//! gofmt's rule, applied to two shapes that real feature code aligns by hand:
//! - consecutive substitutions aligned at `by`/`from` (Inter `ss05`):
//!   `sub A              by A.circled;`
//! - consecutive class definitions laid out as a table, where each column is
//!   one glyph across parallel classes (Recursive `common.fea`):
//!   `@numbers_dflt = [ zero          one ...`
//!
//! A run is aligned only when the author aligned it (two or more spaces before
//! a column on the same line). Columns are recomputed from the widest cell, so
//! renaming a glyph never leaves a ragged table. A run that needs no padding
//! is laid out as ordinary statements, since nothing would mark it as a table
//! on the next pass; that keeps formatting idempotent.

use std::collections::HashMap;

use fea_rs::Kind;

use super::{Elem, Placed, Settings, Stmt, Tok, sep};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Shape {
    /// cut before `by`/`from`
    Rule,
    /// cut before `=` and before every class member
    ClassRow { members: usize },
}

/// Flat-render a statement, splitting it into cells before the tokens `cut` selects.
fn cells(elems: &[Elem], mut cut: impl FnMut(usize, &Tok) -> bool) -> Option<Vec<String>> {
    let mut out = vec![String::new()];
    let mut prev: Option<&Tok> = None;
    for (i, e) in elems.iter().enumerate() {
        let Elem::Tok(t) = e else { return None };
        if prev.is_some() && cut(i, t) {
            out.push(String::new());
        } else if let Some(p) = prev {
            out.last_mut().unwrap().push_str(sep(p, t));
        }
        out.last_mut().unwrap().push_str(t.text);
        prev = Some(t);
    }
    Some(out)
}

fn shape(elems: &[Elem]) -> Option<(Shape, bool)> {
    let toks: Vec<&Tok> = elems
        .iter()
        .map(|e| match e {
            Elem::Tok(t) => Some(t),
            _ => None,
        })
        .collect::<Option<_>>()?;
    // a statement written across lines isn't a table row
    if toks.iter().skip(1).any(|t| t.nl_before > 0) {
        return None;
    }
    let aligned_before = |t: &Tok| t.nl_before == 0 && t.ws_len >= 2;

    let is_class_def = toks
        .iter()
        .any(|t| t.kind == Kind::Eq && t.parent == Kind::GlyphClassDefNode);
    if is_class_def {
        let classes = toks.iter().filter(|t| t.open.is_some()).count();
        if classes != 1 {
            return None;
        }
        let members = toks
            .iter()
            .filter_map(|t| t.class.map(|(_, u)| u))
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        let author_aligned = toks
            .iter()
            .any(|t| (t.class.is_some() || t.kind == Kind::Eq) && aligned_before(t));
        return Some((Shape::ClassRow { members }, author_aligned));
    }

    let by = toks
        .iter()
        .filter(|t| matches!(t.kind, Kind::ByKw | Kind::FromKw))
        .count();
    if by == 1 && toks.iter().all(|t| t.open.is_none() || t.class.is_none()) {
        let author_aligned = toks
            .iter()
            .any(|t| matches!(t.kind, Kind::ByKw | Kind::FromKw) && aligned_before(t));
        return Some((Shape::Rule, author_aligned));
    }
    None
}

fn row_cells(elems: &[Elem], shape: Shape) -> Option<Vec<String>> {
    match shape {
        Shape::Rule => cells(elems, |_, t| matches!(t.kind, Kind::ByKw | Kind::FromKw)),
        Shape::ClassRow { .. } => {
            // a new cell at `=` and at the first token of every class member
            let mut seen_units = std::collections::HashSet::new();
            cells(elems, |_, t| {
                if t.kind == Kind::Eq && t.parent == Kind::GlyphClassDefNode {
                    return true;
                }
                match t.class {
                    Some((_, u)) if t.open.is_none() && t.close.is_none() => {
                        seen_units.insert(u) && u > 0
                    }
                    _ => false,
                }
            })
        }
    }
}

/// Lines for statements in aligned runs, keyed by statement index.
pub(super) fn aligned_runs(stmts: &[Placed], opts: &Settings) -> HashMap<usize, String> {
    let mut out = HashMap::new();
    let mut i = 0;
    while i < stmts.len() {
        let Some((first_shape, _)) = code(&stmts[i]).and_then(shape) else {
            i += 1;
            continue;
        };
        // extend the run: same depth, same shape, no blank line or comment between
        let mut j = i + 1;
        let mut author_aligned = code(&stmts[i]).and_then(shape).is_some_and(|(_, a)| a);
        while j < stmts.len() && stmts[j].depth == stmts[i].depth && stmts[j].nl_before == 1 {
            match code(&stmts[j]).and_then(shape) {
                Some((s, a)) if s == first_shape => {
                    author_aligned |= a;
                    j += 1;
                }
                _ => break,
            }
        }
        if j - i >= 2
            && author_aligned
            && let Some(lines) = render_run(&stmts[i..j], first_shape, opts)
        {
            for (k, line) in lines.into_iter().enumerate() {
                out.insert(i + k, line);
            }
        }
        i = j;
    }
    out
}

fn code<'s, 'a>(p: &'s Placed<'a>) -> Option<&'s [Elem<'a>]> {
    match &p.stmt {
        Stmt::Code {
            elems,
            trailing: None,
        } => Some(elems),
        _ => None,
    }
}

fn render_run(run: &[Placed], shape: Shape, opts: &Settings) -> Option<Vec<String>> {
    let rows: Vec<Vec<String>> = run
        .iter()
        .map(|p| row_cells(code(p)?, shape))
        .collect::<Option<_>>()?;
    let columns = rows.iter().map(Vec::len).max()?;
    let mut widths = vec![0usize; columns];
    for row in &rows {
        for (c, cell) in row.iter().enumerate() {
            widths[c] = widths[c].max(cell.chars().count());
        }
    }
    // a run that needs no padding anywhere isn't a table, and the next pass
    // couldn't tell it was one: lay it out as ordinary statements (an overlong
    // class row would otherwise stay on one line now and wrap next time)
    let padded = rows.iter().any(|row| {
        row.iter()
            .enumerate()
            .any(|(c, cell)| c + 1 < row.len() && widths[c] > cell.chars().count())
    });
    if !padded {
        return None;
    }
    let indent = opts.indent.repeat(run[0].depth);
    let lines: Vec<String> = rows
        .iter()
        .map(|row| {
            let mut line = indent.clone();
            for (c, cell) in row.iter().enumerate() {
                line.push_str(cell);
                if c + 1 < row.len() {
                    line.push_str(&" ".repeat(widths[c] - cell.chars().count() + 1));
                }
            }
            line
        })
        .collect();
    // rules must still fit; a class table may run past the width, since wrapping
    // it would destroy the columns the author built it for
    if shape == Shape::Rule && lines.iter().any(|l| l.chars().count() > opts.width) {
        return None;
    }
    Some(lines)
}

//! Rules about what lookups do: rules that never run, duplicate substitutions
//! and kerning pairs, lookups called where they can't work, mark classes that
//! clash, empty blocks, and `languagesystem` statements after a feature.
//!
//! [`block_facts`] records each feature and lookup block as a list of
//! statements, with glyph operands left unexpanded (a class may be defined in
//! another piece). [`check`] runs during combine, once every class is known.
//!
//! Compilers put a feature's rules into lookups in order: a new lookup starts
//! at `lookupflag`, `script`, `language`, a `lookup` reference or block, and
//! whenever the kind of rule changes. A `lookup` block is always one lookup.
//! Within one lookup the first rule that matches wins.

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::rc::Rc;

use fea_rs::Kind;

use super::{ClassDefs, Facts, Item, Member, members, operand};
use crate::diagnostic::{Applicability, Diagnostic, Fix, Related, Rule, Severity, TextEdit};
use crate::glyphs::GlyphNames;

/// A glyph operand, unexpanded; `None` when it can't be expanded statically.
type Operand = Option<Vec<Member>>;

/// A `feature` or `lookup` block.
#[derive(Clone, Debug)]
pub(crate) struct Block {
    kind: BlockKind,
    /// The feature tag or lookup name.
    name: String,
    name_range: Range<usize>,
    /// Holds no rule, lookup, `aalt` feature reference or `size` parameters.
    empty: bool,
    statements: Vec<Statement>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlockKind {
    Feature,
    Lookup,
}

#[derive(Clone, Debug)]
enum Statement {
    /// `lookupflag`, `script`, `language`, or a `lookup` reference or block:
    /// a feature's next rules go into a new lookup.
    Boundary,
    Rule(Box<RuleFact>),
}

/// The OpenType lookup type a rule compiles to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LookupType {
    Single,
    Multiple,
    Alternate,
    Ligature,
    ChainSub,
    ReverseChainSub,
    SinglePos,
    PairPos,
    Cursive,
    MarkToBase,
    MarkToLigature,
    MarkToMark,
    ChainPos,
}

impl LookupType {
    fn of(kind: Kind) -> Option<LookupType> {
        Some(match kind {
            Kind::GsubType1 => LookupType::Single,
            Kind::GsubType2 => LookupType::Multiple,
            Kind::GsubType3 => LookupType::Alternate,
            Kind::GsubType4 => LookupType::Ligature,
            Kind::GsubType5 | Kind::GsubType6 | Kind::GsubIgnore => LookupType::ChainSub,
            Kind::GsubType8 => LookupType::ReverseChainSub,
            Kind::GposType1 => LookupType::SinglePos,
            Kind::GposType2 => LookupType::PairPos,
            Kind::GposType3 => LookupType::Cursive,
            Kind::GposType4 => LookupType::MarkToBase,
            Kind::GposType5 => LookupType::MarkToLigature,
            Kind::GposType6 => LookupType::MarkToMark,
            Kind::GposType7 | Kind::GposType8 | Kind::GposIgnore => LookupType::ChainPos,
            _ => return None,
        })
    }

    fn substitutes(self) -> bool {
        matches!(
            self,
            LookupType::Single
                | LookupType::Multiple
                | LookupType::Alternate
                | LookupType::Ligature
                | LookupType::ChainSub
                | LookupType::ReverseChainSub
        )
    }

    fn is_contextual(self) -> bool {
        matches!(
            self,
            LookupType::ChainSub | LookupType::ReverseChainSub | LookupType::ChainPos
        )
    }

    fn attaches_marks(self) -> bool {
        matches!(
            self,
            LookupType::MarkToBase | LookupType::MarkToLigature | LookupType::MarkToMark
        )
    }
}

/// One rule. For a contextual rule `input` is the marked sequence; otherwise
/// it's what the rule applies to (the target of `sub`, the glyphs of `pos`).
#[derive(Clone, Debug)]
struct RuleFact {
    ty: LookupType,
    range: Range<usize>,
    /// The rule with its line, when it's alone on the line; else just the rule.
    removal: Range<usize>,
    /// Glyphs before the input, nearest first.
    backtrack: Vec<Operand>,
    input: Vec<Operand>,
    lookahead: Vec<Operand>,
    /// Lookups called from the input: (input position, name, range of the name).
    calls: Vec<(usize, String, Range<usize>)>,
    /// `enum pos`: class operands become glyph pairs.
    enumerated: bool,
    /// Every operand of a pair is one glyph name (`pos a v -10;`).
    single_glyph_pair: bool,
    /// Mark classes attached by a mark rule, with their ranges.
    mark_classes: Vec<(String, Range<usize>)>,
    /// What a substitution produces: the operands after `by` or `from`.
    replacement: Vec<Operand>,
    /// A positioning rule's value records, without spaces (`-10`, `<-41 0 0 0>`).
    value: String,
    /// What a contextual rule does: `ignore`, or the lookups it calls by
    /// position and its inline replacement. Equal actions mean the same effect.
    action: String,
    /// A contextual rule with `by` after its marked glyphs (`sub x a' by b;`).
    inline: bool,
}

/// A top-level feature block: where it starts, and where its line starts.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FeatureStart {
    start: usize,
    line_start: usize,
}

/// A top-level `languagesystem` statement.
#[derive(Clone, Debug)]
pub(crate) struct LanguageSystem {
    range: Range<usize>,
    /// The statement with its line, to move it.
    removal: Range<usize>,
    text: String,
    /// Just past the newline that ends its line.
    line_end: usize,
}

const OPERANDS: [Kind; 7] = [
    Kind::GlyphName,
    Kind::GlyphNameOrRange,
    Kind::GlyphClass,
    Kind::NamedGlyphClass,
    Kind::GlyphRange,
    Kind::Cid,
    Kind::GlyphsPredicateNode,
];

/// Direct children of the node at `at`, with their indices, trivia skipped.
fn children<'b>(items: &'b [Item<'b>], at: usize) -> impl Iterator<Item = (usize, &'b Item<'b>)> {
    let end = items[at].range.end;
    let depth = items[at].depth;
    items
        .iter()
        .enumerate()
        .skip(at + 1)
        .take_while(move |(_, x)| x.range.start < end || (x.range.is_empty() && x.depth > depth))
        .filter(move |(_, x)| x.parent == at && !matches!(x.kind, Kind::Whitespace | Kind::Comment))
}

fn operand_at(items: &[Item], at: usize, font: Option<&GlyphNames>) -> Operand {
    members(&operand(items, at).collect::<Vec<_>>(), font)
}

/// Where the line holding `at` starts (after `\n`, `\r` or `\r\n`).
fn line_start(source: &str, at: usize) -> usize {
    source[..at].rfind(['\n', '\r']).map_or(0, |i| i + 1)
}

/// Just past the line break that ends the line holding `at`, or the end of the text.
fn line_end(source: &str, at: usize) -> usize {
    let Some(i) = source[at..].find(['\n', '\r']) else {
        return source.len();
    };
    let crlf = source[at + i..].starts_with("\r\n");
    at + i + if crlf { 2 } else { 1 }
}

/// `range` widened to its whole line when nothing else is on that line.
fn line_of(source: &str, range: &Range<usize>) -> Range<usize> {
    let line_start = line_start(source, range.start);
    let line_end = line_end(source, range.end);
    let alone = source[line_start..range.start].trim().is_empty()
        && source[range.end..line_end].trim().is_empty();
    if alone {
        line_start..line_end
    } else {
        range.clone()
    }
}

/// Record blocks, mark classes and top-level `languagesystem` statements.
pub(crate) fn block_facts(
    items: &[Item],
    source: &str,
    font: Option<&GlyphNames>,
    facts: &mut Facts,
) {
    for (i, item) in items.iter().enumerate() {
        match item.kind {
            Kind::FeatureNode | Kind::LookupBlockNode => {
                if let Some(block) = block(items, i, source, font) {
                    facts.blocks.push(block);
                }
                if item.kind == Kind::FeatureNode && item.parent == usize::MAX {
                    facts.feature_starts.push(FeatureStart {
                        start: item.range.start,
                        line_start: line_start(source, item.range.start),
                    });
                }
            }
            Kind::MarkClassNode => {
                let mut glyphs = None;
                let mut name = None;
                for (j, child) in children(items, i) {
                    match child.kind {
                        Kind::NamedGlyphClass if glyphs.is_some() => {
                            name = child.text.map(String::from)
                        }
                        k if OPERANDS.contains(&k) && glyphs.is_none() => {
                            glyphs = Some(operand_at(items, j, font))
                        }
                        _ => {}
                    }
                }
                if let (Some(name), Some(glyphs)) = (name, glyphs) {
                    facts.mark_class_members.push((name, glyphs));
                }
            }
            Kind::LanguageSystemNode if item.parent == usize::MAX => {
                let removal = line_of(source, &item.range);
                facts.languagesystems.push(LanguageSystem {
                    range: item.range.clone(),
                    text: source[item.range.clone()].to_string(),
                    line_end: removal.end,
                    removal,
                });
            }
            _ => {}
        }
    }
}

fn block(items: &[Item], at: usize, source: &str, font: Option<&GlyphNames>) -> Option<Block> {
    let (kind, label) = match items[at].kind {
        Kind::FeatureNode => (BlockKind::Feature, Kind::Tag),
        _ => (BlockKind::Lookup, Kind::Label),
    };
    let (_, name) = children(items, at).find(|(_, x)| x.kind == label)?;
    let mut statements = Vec::new();
    // ufo2ft inserts generated rules at a `# Automatic Code` comment, and Glyphs
    // marks generated features `# automatic`: such a block isn't empty when built
    let end = items[at].range.end;
    let generated = items[at + 1..]
        .iter()
        .take_while(|x| x.range.start < end)
        .any(|x| {
            x.kind == Kind::Comment
                && x.text
                    .is_some_and(|t| t.to_ascii_lowercase().contains("automatic"))
        });
    let mut empty = !generated;
    for (j, child) in children(items, at) {
        match child.kind {
            Kind::LookupFlagNode | Kind::ScriptNode | Kind::LanguageNode => {
                statements.push(Statement::Boundary)
            }
            Kind::LookupRefNode | Kind::LookupBlockNode => {
                statements.push(Statement::Boundary);
                empty = false;
            }
            Kind::GsubIgnore | Kind::GposIgnore => {
                empty = false;
                // `ignore sub x a', y a';` is one pattern per statement node
                for (k, pattern) in children(items, j) {
                    if pattern.kind == Kind::IgnoreRuleStatementNode {
                        let ty = LookupType::of(child.kind)?;
                        let mut fact = contextual(items, k, ty, source, font);
                        fact.range = child.range.clone();
                        fact.removal = line_of(source, &child.range);
                        fact.action = "ignore".to_string();
                        statements.push(Statement::Rule(Box::new(fact)));
                    }
                }
            }
            k => {
                // other statements (rules, `aalt` references, `size` parameters) are content;
                // so are tokens the parser couldn't place, which keeps broken blocks out of L003
                let structural = matches!(
                    k,
                    Kind::SubtableNode
                        | Kind::FeatureNamesKw
                        | Kind::CvParametersKw
                        | Kind::FeatureKw
                        | Kind::LookupKw
                        | Kind::Tag
                        | Kind::Label
                        | Kind::LBrace
                        | Kind::RBrace
                        | Kind::Semi
                        | Kind::UseExtensionKw
                );
                if !structural {
                    empty = false;
                }
                if let Some(ty) = LookupType::of(k) {
                    let fact = rule(items, j, ty, source, font);
                    statements.push(Statement::Rule(Box::new(fact)));
                }
            }
        }
    }
    Some(Block {
        kind,
        name: name.text.unwrap_or_default().to_string(),
        name_range: name.range.clone(),
        empty,
        statements,
    })
}

fn rule(
    items: &[Item],
    at: usize,
    ty: LookupType,
    source: &str,
    font: Option<&GlyphNames>,
) -> RuleFact {
    let mut fact = if ty.is_contextual() {
        contextual(items, at, ty, source, font)
    } else {
        plain(items, at, ty, source, font)
    };
    fact.range = items[at].range.clone();
    fact.removal = line_of(source, &items[at].range);
    fact
}

fn empty_fact(ty: LookupType) -> RuleFact {
    RuleFact {
        ty,
        range: 0..0,
        removal: 0..0,
        backtrack: Vec::new(),
        input: Vec::new(),
        lookahead: Vec::new(),
        calls: Vec::new(),
        enumerated: false,
        single_glyph_pair: false,
        mark_classes: Vec::new(),
        replacement: Vec::new(),
        value: String::new(),
        action: String::new(),
        inline: false,
    }
}

/// A rule without context: its operands up to `by` / `from` (substitutions),
/// or its first one or two glyph operands (positioning).
fn plain(
    items: &[Item],
    at: usize,
    ty: LookupType,
    source: &str,
    font: Option<&GlyphNames>,
) -> RuleFact {
    let mut fact = empty_fact(ty);
    let wanted = match ty {
        LookupType::PairPos => 2,
        t if t.substitutes() => usize::MAX,
        _ => 1,
    };
    let mut single_glyphs = true;
    let mut after_by = false;
    let mut values: Vec<String> = Vec::new();
    for (j, child) in children(items, at) {
        match child.kind {
            Kind::ByKw | Kind::FromKw => after_by = true,
            Kind::EnumKw => fact.enumerated = true,
            Kind::ValueRecordNode | Kind::Number => {
                values.push(compact(&source[child.range.clone()]));
            }
            k if OPERANDS.contains(&k) && after_by => {
                fact.replacement.push(operand_at(items, j, font));
            }
            k if OPERANDS.contains(&k) && fact.input.len() < wanted => {
                single_glyphs &= matches!(k, Kind::GlyphName | Kind::GlyphNameOrRange);
                fact.input.push(operand_at(items, j, font));
            }
            _ => {}
        }
    }
    fact.value = values.join("|");
    fact.single_glyph_pair = ty == LookupType::PairPos && single_glyphs && fact.input.len() == 2;
    if ty.attaches_marks() {
        let end = items[at].range.end;
        for x in items[at + 1..].iter().take_while(|x| x.range.start < end) {
            if x.kind == Kind::NamedGlyphClass && x.parent_kind == Kind::AnchorMarkNode {
                fact.mark_classes
                    .push((x.text.unwrap_or_default().to_string(), x.range.clone()));
            }
        }
    }
    fact
}

/// Text without whitespace, to compare values and actions however they're spaced.
fn compact(text: &str) -> String {
    text.split_whitespace().collect()
}

/// A rule with marked glyphs: backtrack, input (with the lookups it calls) and lookahead.
fn contextual(
    items: &[Item],
    at: usize,
    ty: LookupType,
    source: &str,
    font: Option<&GlyphNames>,
) -> RuleFact {
    let mut fact = empty_fact(ty);
    let mut inline = String::new();
    for (j, part) in children(items, at) {
        match part.kind {
            Kind::InlineSubNode => {
                inline = compact(&source[part.range.clone()]);
                fact.inline = true;
            }
            Kind::BacktrackSequence => {
                for (k, x) in children(items, j) {
                    if OPERANDS.contains(&x.kind) {
                        fact.backtrack.push(operand_at(items, k, font));
                    }
                }
                fact.backtrack.reverse();
            }
            Kind::LookaheadSequence => {
                for (k, x) in children(items, j) {
                    if OPERANDS.contains(&x.kind) {
                        fact.lookahead.push(operand_at(items, k, font));
                    }
                }
            }
            Kind::ContextSequence => {
                for (k, glyph) in children(items, j) {
                    if glyph.kind != Kind::ContextGlyphNode {
                        continue;
                    }
                    let position = fact.input.len();
                    let mut value = None;
                    for (m, x) in children(items, k) {
                        if OPERANDS.contains(&x.kind) && value.is_none() {
                            value = Some(operand_at(items, m, font));
                        }
                        if x.kind == Kind::LookupRefNode
                            && let Some((_, name)) =
                                children(items, m).find(|(_, n)| n.kind == Kind::Ident)
                        {
                            let label = name.text.unwrap_or_default().to_string();
                            fact.calls.push((position, label, name.range.clone()));
                        }
                    }
                    fact.input.push(value.flatten());
                }
            }
            _ => {}
        }
    }
    let calls: Vec<String> = fact
        .calls
        .iter()
        .map(|(p, name, _)| format!("{p}:{name}"))
        .collect();
    fact.action = format!("{} {inline}", calls.join(" "));
    fact
}

// ---------------------------------------------------------------- checks

/// Glyph sets as sorted, deduplicated ids, with class expansions cached.
struct Glyphs<'f> {
    classes: ClassDefs<'f>,
    /// Where the operands being expanded are written, which decides what a class name means.
    at: usize,
    ids: HashMap<&'f str, u32>,
    names: Vec<&'f str>,
    /// Class expansions by where the definition is.
    expanded: HashMap<usize, Option<Rc<[u32]>>>,
    /// Class expansions in written order, for substitutions that pair glyphs by index.
    ordered: HashMap<usize, Option<Rc<[u32]>>>,
}

impl<'f> Glyphs<'f> {
    fn id(&mut self, name: &'f str) -> u32 {
        if let Some(&id) = self.ids.get(name) {
            return id;
        }
        let id = self.names.len() as u32;
        self.names.push(name);
        self.ids.insert(name, id);
        id
    }

    /// The class `name` as defined where the current operands are written.
    fn class(&mut self, name: &str, depth: usize) -> Option<Rc<[u32]>> {
        let (defined_at, members) = self.classes.resolve(name, self.at)?;
        if let Some(cached) = self.expanded.get(&defined_at) {
            return cached.clone();
        }
        let used_at = std::mem::replace(&mut self.at, defined_at);
        let value = members.as_ref().and_then(|m| self.set_of(m, depth + 1));
        self.at = used_at;
        self.expanded.insert(defined_at, value.clone());
        value
    }

    fn set_of(
        &mut self,
        members: impl IntoIterator<Item = &'f Member>,
        depth: usize,
    ) -> Option<Rc<[u32]>> {
        if depth > 32 {
            return None;
        }
        let mut out: Vec<u32> = Vec::new();
        for m in members {
            match m {
                Member::Glyph(g) => out.push(self.id(g)),
                Member::Class(c) => out.extend(self.class(c, depth)?.iter().copied()),
            }
        }
        out.sort_unstable();
        out.dedup();
        Some(out.into())
    }

    /// An operand's glyphs in written order, repeats kept.
    fn list(&mut self, operand: &'f Operand) -> Option<Vec<u32>> {
        self.list_of(operand.as_ref()?, 0)
    }

    fn list_of(&mut self, members: &'f [Member], depth: usize) -> Option<Vec<u32>> {
        if depth > 32 {
            return None;
        }
        let mut out = Vec::new();
        for m in members {
            match m {
                Member::Glyph(g) => out.push(self.id(g)),
                Member::Class(c) => {
                    let (defined_at, members) = self.classes.resolve(c, self.at)?;
                    if let Some(cached) = self.ordered.get(&defined_at) {
                        out.extend(cached.as_ref()?.iter().copied());
                        continue;
                    }
                    let used_at = std::mem::replace(&mut self.at, defined_at);
                    let value: Option<Rc<[u32]>> = members
                        .as_ref()
                        .and_then(|m| self.list_of(m, depth + 1))
                        .map(Into::into);
                    self.at = used_at;
                    self.ordered.insert(defined_at, value.clone());
                    out.extend(value?.iter().copied());
                }
            }
        }
        Some(out)
    }

    fn set(&mut self, operand: &'f Operand) -> Option<Rc<[u32]>> {
        self.set_of(operand.as_ref()?, 0)
    }

    /// Every operand of a sequence, or `None` if one can't be expanded.
    fn sequence(&mut self, operands: &'f [Operand]) -> Option<Vec<Rc<[u32]>>> {
        operands.iter().map(|o| self.set(o)).collect()
    }

    fn names(&self, ids: &[u32], at_most: usize) -> String {
        let mut shown: Vec<&str> = ids
            .iter()
            .take(at_most)
            .map(|&i| self.names[i as usize])
            .collect();
        if ids.len() > at_most {
            shown.push("…");
        }
        shown.join(" ")
    }
}

fn superset(a: &[u32], b: &[u32]) -> bool {
    b.iter().all(|g| a.binary_search(g).is_ok())
}

fn intersection(a: &[u32], b: &[u32]) -> Vec<u32> {
    a.iter()
        .copied()
        .filter(|g| b.binary_search(g).is_ok())
        .collect()
}

/// The lookups a block compiles to: runs of rules of one type.
fn lookups_in(block: &Block) -> Vec<Vec<&RuleFact>> {
    let mut out: Vec<Vec<&RuleFact>> = Vec::new();
    let mut current: Vec<&RuleFact> = Vec::new();
    for statement in &block.statements {
        match statement {
            // `script` and `language` split a `lookup` block too
            Statement::Boundary => out.push(std::mem::take(&mut current)),
            Statement::Rule(r) => {
                if current.first().is_some_and(|first| first.ty != r.ty) {
                    out.push(std::mem::take(&mut current));
                }
                current.push(r);
            }
        }
    }
    out.push(current);
    out.retain(|l| !l.is_empty());
    out
}

/// What a standalone lookup does, for the lookups that call it.
struct Called {
    ty: LookupType,
    /// Every glyph its rules start at; `None` when that isn't known.
    coverage: Option<Vec<u32>>,
}

fn shifted(r: &Range<usize>, at: usize) -> Range<usize> {
    r.start + at..r.end + at
}

/// Run every check over the facts of all pieces.
pub(crate) fn check<'f>(
    pieces: impl Iterator<Item = (usize, &'f Facts)> + Clone,
    out: &mut Vec<Diagnostic>,
) {
    let classes = ClassDefs::new(pieces.clone());
    let mut mark_class_members: HashMap<&str, Vec<&Member>> = HashMap::new();
    let mut unknown_mark_classes: HashSet<&str> = HashSet::new();
    for (_, f) in pieces.clone() {
        for (name, glyphs) in &f.mark_class_members {
            match glyphs {
                Some(g) => mark_class_members.entry(name).or_default().extend(g.iter()),
                None => {
                    unknown_mark_classes.insert(name);
                }
            }
        }
    }
    let mut glyphs = Glyphs {
        classes,
        at: 0,
        ids: HashMap::new(),
        names: Vec::new(),
        expanded: HashMap::new(),
        ordered: HashMap::new(),
    };

    let called = called_lookups(pieces.clone(), &mut glyphs);

    for (at, f) in pieces.clone() {
        for block in &f.blocks {
            if block.empty {
                empty_block(block, at, out);
            }
            // `aalt` gathers alternates from other features: repeats are how it works
            if block.kind == BlockKind::Feature && block.name == "aalt" {
                continue;
            }
            for lookup in lookups_in(block) {
                match lookup[0].ty {
                    LookupType::Single | LookupType::Multiple | LookupType::Alternate => {
                        duplicate_substitutions(&lookup, at, &mut glyphs, out)
                    }
                    LookupType::Ligature => duplicate_ligatures(&lookup, at, &mut glyphs, out),
                    LookupType::PairPos => duplicate_pairs(&lookup, at, &mut glyphs, out),
                    t if t.is_contextual() => unreachable_rules(&lookup, at, &mut glyphs, out),
                    t if t.attaches_marks() => mark_class_conflicts(
                        &lookup,
                        at,
                        &mark_class_members,
                        &unknown_mark_classes,
                        &mut glyphs,
                        out,
                    ),
                    _ => {}
                }
                for rule in &lookup {
                    lookup_calls(rule, at, &called, &mut glyphs, out);
                }
            }
        }
    }

    languagesystem_order(pieces, out);
}

fn empty_block(block: &Block, at: usize, out: &mut Vec<Diagnostic>) {
    let what = match block.kind {
        BlockKind::Feature => format!(
            "feature {} has no rules, so it adds nothing to the font",
            block.name
        ),
        BlockKind::Lookup => format!("lookup {} has no rules, so it does nothing", block.name),
    };
    out.push(Diagnostic::new(
        Rule::EmptyBlock,
        Severity::Warning,
        shifted(&block.name_range, at),
        what,
    ));
}

fn related(range: &Range<usize>, at: usize, label: &str) -> Option<Related> {
    Some(Related {
        range: shifted(range, at),
        label: label.to_string(),
    })
}

fn remove_fix(rule: &RuleFact, at: usize, title: &str) -> Fix {
    Fix::new(
        title,
        vec![TextEdit {
            range: shifted(&rule.removal, at),
            new_text: String::new(),
        }],
        Applicability::Safe,
    )
}

/// R006: a rule that repeats what an earlier one already does.
fn redundant(rule: &RuleFact, earlier: &RuleFact, at: usize, message: &str) -> Diagnostic {
    let mut d = Diagnostic::new(
        Rule::RedundantRule,
        Severity::Hint,
        shifted(&rule.range, at),
        message,
    );
    d.related = related(&earlier.range, at, "earlier rule");
    d.fixes
        .push(remove_fix(rule, at, "Remove the redundant rule"));
    d
}

/// What a substitution maps each of its targets to, or `None` when that can't be known.
fn substitution_results<'f>(
    rule: &'f RuleFact,
    glyphs: &mut Glyphs<'f>,
) -> Option<Vec<(u32, Vec<u32>)>> {
    let targets = glyphs.list(rule.input.first()?)?;
    match rule.ty {
        LookupType::Single => {
            let replacements = glyphs.list(rule.replacement.first()?)?;
            match replacements.len() {
                1 => Some(targets.iter().map(|&t| (t, replacements.clone())).collect()),
                n if n == targets.len() => Some(
                    targets
                        .iter()
                        .zip(&replacements)
                        .map(|(&t, &r)| (t, vec![r]))
                        .collect(),
                ),
                _ => None,
            }
        }
        LookupType::Multiple => {
            let sequence: Vec<u32> = rule
                .replacement
                .iter()
                .map(|o| match glyphs.list(o)?.as_slice() {
                    [g] => Some(*g),
                    _ => None,
                })
                .collect::<Option<_>>()?;
            Some(targets.iter().map(|&t| (t, sequence.clone())).collect())
        }
        LookupType::Alternate => {
            let mut alternates = glyphs.list(rule.replacement.first()?)?;
            alternates.sort_unstable();
            Some(targets.iter().map(|&t| (t, alternates.clone())).collect())
        }
        _ => None,
    }
}

/// R002 for single, multiple and alternate substitutions: one glyph substituted
/// twice with different results. The same result twice is R006.
fn duplicate_substitutions<'f>(
    lookup: &[&'f RuleFact],
    at: usize,
    glyphs: &mut Glyphs<'f>,
    out: &mut Vec<Diagnostic>,
) {
    let mut first: HashMap<u32, (&RuleFact, Vec<u32>)> = HashMap::new();
    for rule in lookup {
        glyphs.at = rule.range.start + at;
        let Some(results) = substitution_results(rule, glyphs) else {
            continue;
        };
        let mut conflicts: Vec<u32> = Vec::new();
        let mut conflicting_rule = None;
        let mut repeating_rule = None;
        let mut repeats = 0;
        for (target, result) in &results {
            match first.get(target) {
                Some((earlier, r)) if r == result => {
                    repeats += 1;
                    repeating_rule.get_or_insert(*earlier);
                }
                Some((earlier, _)) => {
                    conflicts.push(*target);
                    conflicting_rule.get_or_insert(*earlier);
                }
                None => {}
            }
        }
        if let Some(earlier) = conflicting_rule {
            let mut d = Diagnostic::new(
                Rule::DuplicateSubstitution,
                Severity::Error,
                shifted(&rule.range, at),
                format!(
                    "{} is already substituted differently earlier in this lookup; fea-rs uses \
                     this rule and fontTools rejects the file",
                    glyphs.names(&conflicts, 4)
                ),
            );
            d.related = related(&earlier.range, at, "earlier substitution");
            out.push(d);
        } else if let Some(earlier) = repeating_rule
            && repeats == results.len()
        {
            out.push(redundant(
                rule,
                earlier,
                at,
                "repeats a substitution earlier in this lookup, so it changes nothing",
            ));
        }
        for (target, result) in results {
            first.entry(target).or_insert((rule, result));
        }
    }
}

/// R002 for ligatures: one sequence of glyphs with two different ligatures.
/// The same ligature twice is R006.
fn duplicate_ligatures<'f>(
    lookup: &[&'f RuleFact],
    at: usize,
    glyphs: &mut Glyphs<'f>,
    out: &mut Vec<Diagnostic>,
) {
    const MAX_SEQUENCES: usize = 4096;
    let mut first: HashMap<Vec<u32>, (&RuleFact, Vec<u32>)> = HashMap::new();
    for rule in lookup {
        glyphs.at = rule.range.start + at;
        let Some(sets) = glyphs.sequence(&rule.input) else {
            continue;
        };
        let Some(ligature) = rule.replacement.first().and_then(|o| glyphs.list(o)) else {
            continue;
        };
        let count = sets.iter().try_fold(1usize, |n, s| n.checked_mul(s.len()));
        if count.is_none_or(|n| n > MAX_SEQUENCES || n == 0) {
            continue;
        }
        let mut sequences: Vec<Vec<u32>> = vec![Vec::new()];
        for set in &sets {
            sequences = sequences
                .into_iter()
                .flat_map(|s| set.iter().map(move |g| [s.as_slice(), &[*g]].concat()))
                .collect();
        }
        let conflict = sequences
            .iter()
            .find(|s| first.get(*s).is_some_and(|(_, l)| *l != ligature));
        let repeated = sequences.iter().all(|s| first.contains_key(s));
        if let Some(sequence) = conflict {
            let mut d = Diagnostic::new(
                Rule::DuplicateSubstitution,
                Severity::Error,
                shifted(&rule.range, at),
                format!(
                    "{} is already a different ligature earlier in this lookup; both fea-rs and \
                     fontTools reject the file",
                    glyphs.names(sequence, 8)
                ),
            );
            d.related = related(&first[sequence].0.range, at, "earlier ligature");
            out.push(d);
        } else if repeated && let Some(earlier) = sequences.first().map(|s| first[s].0) {
            out.push(redundant(
                rule,
                earlier,
                at,
                "repeats a ligature earlier in this lookup, so it changes nothing",
            ));
        }
        for s in sequences {
            first.entry(s).or_insert((rule, ligature.clone()));
        }
    }
}

/// R003: a glyph pair positioned twice in one lookup with different values.
/// Compilers keep the first, so the later value is ignored and removing it
/// changes nothing. The same value twice is R006.
fn duplicate_pairs<'f>(
    lookup: &[&'f RuleFact],
    at: usize,
    glyphs: &mut Glyphs<'f>,
    out: &mut Vec<Diagnostic>,
) {
    let mut first: HashMap<(u32, u32), &RuleFact> = HashMap::new();
    for rule in lookup {
        // class pairs (`pos @A @B`, `pos [a e] v` without `enum`) go to their own subtable
        if !(rule.single_glyph_pair || rule.enumerated) {
            continue;
        }
        glyphs.at = rule.range.start + at;
        let Some(sets) = glyphs.sequence(&rule.input) else {
            continue;
        };
        let [left, right] = &sets[..] else {
            continue;
        };
        let pairs: Vec<(u32, u32)> = left
            .iter()
            .flat_map(|a| right.iter().map(move |b| (*a, *b)))
            .collect();
        let repeated: Vec<&(u32, u32)> = pairs.iter().filter(|p| first.contains_key(*p)).collect();
        let all = repeated.len() == pairs.len();
        let different = repeated.iter().find(|p| first[**p].value != rule.value);
        if let Some(&&(a, b)) = different {
            let shown = format!("{} {}", glyphs.names(&[a], 1), glyphs.names(&[b], 1));
            let earlier = first[&(a, b)];
            let message = if all && pairs.len() == 1 {
                format!(
                    "the pair {shown} is already positioned earlier in this lookup ({}), so this \
                     value is ignored",
                    earlier.value.replace('|', " ")
                )
            } else {
                format!(
                    "{} of these pairs, such as {shown}, are already positioned earlier in this \
                     lookup with other values, so those are ignored",
                    repeated.len()
                )
            };
            let mut d = Diagnostic::new(
                Rule::DuplicatePair,
                Severity::Warning,
                shifted(&rule.range, at),
                message,
            );
            d.related = related(&earlier.range, at, "earlier rule, which wins");
            if all {
                d.fixes
                    .push(remove_fix(rule, at, "Remove the ignored rule"));
            }
            out.push(d);
        } else if all && let Some(p) = repeated.first() {
            out.push(redundant(
                rule,
                first[*p],
                at,
                "repeats a kerning pair earlier in this lookup with the same value, so it \
                 changes nothing",
            ));
        }
        for p in pairs {
            first.entry(p).or_insert(rule);
        }
    }
}

/// R001: a contextual rule that an earlier rule in the same lookup always
/// matches first. The earlier rule matches wherever its glyph sets contain the
/// later rule's, position by position around the first input glyph. When both
/// do the same thing, the later one is R006.
fn unreachable_rules<'f>(
    lookup: &[&'f RuleFact],
    at: usize,
    glyphs: &mut Glyphs<'f>,
    out: &mut Vec<Diagnostic>,
) {
    // pairwise comparison; very large lookups would make every keystroke slow
    const MAX_RULES: usize = 2000;
    if lookup.len() > MAX_RULES {
        return;
    }
    struct Pattern {
        back: Vec<Rc<[u32]>>,
        ahead: Vec<Rc<[u32]>>,
    }
    let patterns: Vec<Option<Pattern>> = lookup
        .iter()
        .map(|r| {
            glyphs.at = r.range.start + at;
            let back = glyphs.sequence(&r.backtrack)?;
            let mut ahead = glyphs.sequence(&r.input)?;
            ahead.extend(glyphs.sequence(&r.lookahead)?);
            // a rule with no input or an empty class matches nothing
            if ahead.is_empty() || back.iter().chain(&ahead).any(|s| s.is_empty()) {
                return None;
            }
            Some(Pattern { back, ahead })
        })
        .collect();
    let covers = |a: &Pattern, b: &Pattern| {
        a.back.len() <= b.back.len()
            && a.ahead.len() <= b.ahead.len()
            && a.ahead.iter().zip(&b.ahead).all(|(x, y)| superset(x, y))
            && a.back.iter().zip(&b.back).all(|(x, y)| superset(x, y))
    };
    for (j, later) in patterns.iter().enumerate() {
        let Some(later) = later else {
            continue;
        };
        let earlier = (0..j).find(|&i| patterns[i].as_ref().is_some_and(|p| covers(p, later)));
        let Some(i) = earlier else {
            continue;
        };
        // compilers gather the inline ligatures of a lookup's rules into one
        // ligature lookup that forms the longest match, so whichever rule
        // matches, a later rule's longer ligature still forms
        let shared_ligatures = lookup[i].inline && lookup[j].inline && lookup[j].input.len() > 1;
        if shared_ligatures {
            continue;
        }
        if lookup[i].action == lookup[j].action {
            out.push(redundant(
                lookup[j],
                lookup[i],
                at,
                "never runs, but an earlier rule in this lookup matches everything it matches \
                 and does the same, so it changes nothing",
            ));
            continue;
        }
        let mut d = Diagnostic::new(
            Rule::UnreachableRule,
            Severity::Warning,
            shifted(&lookup[j].range, at),
            "this rule never runs: an earlier rule in the same lookup matches everything it \
             matches and does something else",
        );
        d.related = related(
            &lookup[i].range,
            at,
            "earlier rule that always matches first",
        );
        out.push(d);
    }
}

/// C005: two mark classes that share a glyph, attached in one lookup.
fn mark_class_conflicts<'f>(
    lookup: &[&'f RuleFact],
    at: usize,
    members: &HashMap<&str, Vec<&'f Member>>,
    unknown: &HashSet<&str>,
    glyphs: &mut Glyphs<'f>,
    out: &mut Vec<Diagnostic>,
) {
    /// A mark class already attached in this lookup.
    struct Attached<'r> {
        name: &'r str,
        glyphs: Rc<[u32]>,
        range: &'r Range<usize>,
    }
    let mut seen: Vec<Attached> = Vec::new();
    for rule in lookup {
        glyphs.at = rule.range.start + at;
        for (name, range) in &rule.mark_classes {
            if seen.iter().any(|a| a.name == name) || unknown.contains(name.as_str()) {
                continue;
            }
            let Some(set) = members
                .get(name.as_str())
                .and_then(|m| glyphs.set_of(m.iter().copied(), 0))
            else {
                continue;
            };
            let clash = seen.iter().find_map(|a| {
                let shared = intersection(&set, &a.glyphs);
                (!shared.is_empty()).then_some((a.name, shared, a.range))
            });
            if let Some((other, shared, other_range)) = clash {
                let mut d = Diagnostic::new(
                    Rule::MarkClassConflict,
                    Severity::Error,
                    shifted(range, at),
                    format!(
                        "{} is in both {other} and {name}, which are attached in the same lookup; \
                         compilers reject this, so put them in separate lookups",
                        glyphs.names(&shared, 3)
                    ),
                );
                d.related = related(other_range, at, "the other mark class");
                out.push(d);
            }
            seen.push(Attached {
                name,
                glyphs: set,
                range,
            });
        }
    }
}

/// Every standalone or nested lookup by name (the first definition wins).
fn called_lookups<'f>(
    pieces: impl Iterator<Item = (usize, &'f Facts)>,
    glyphs: &mut Glyphs<'f>,
) -> HashMap<&'f str, Called> {
    let mut out: HashMap<&'f str, Called> = HashMap::new();
    for (at, f) in pieces {
        for block in f.blocks.iter().filter(|b| b.kind == BlockKind::Lookup) {
            if out.contains_key(block.name.as_str()) {
                continue;
            }
            let rules: Vec<&RuleFact> = block
                .statements
                .iter()
                .filter_map(|s| match s {
                    Statement::Rule(r) => Some(r.as_ref()),
                    Statement::Boundary => None,
                })
                .collect();
            let Some(first) = rules.first() else { continue };
            // a mark lookup applies at the mark, which its rules don't name directly
            let coverage = if first.ty.attaches_marks() || first.ty == LookupType::Cursive {
                None
            } else {
                rules
                    .iter()
                    .map(|r| {
                        glyphs.at = r.range.start + at;
                        r.input.first().and_then(|o| glyphs.set(o))
                    })
                    .collect::<Option<Vec<_>>>()
                    .map(|sets| {
                        let mut all: Vec<u32> =
                            sets.iter().flat_map(|s| s.iter().copied()).collect();
                        all.sort_unstable();
                        all.dedup();
                        all
                    })
            };
            out.insert(
                &block.name,
                Called {
                    ty: first.ty,
                    coverage,
                },
            );
        }
    }
    out
}

/// R004 and R005: lookups called from a contextual rule.
fn lookup_calls<'f>(
    rule: &'f RuleFact,
    at: usize,
    called: &HashMap<&str, Called>,
    glyphs: &mut Glyphs<'f>,
    out: &mut Vec<Diagnostic>,
) {
    for (_, name, range) in &rule.calls {
        let Some(lookup) = called.get(name.as_str()) else {
            continue;
        };
        if lookup.ty.substitutes() != rule.ty.substitutes() {
            let (does, from) = if lookup.ty.substitutes() {
                ("substitutes glyphs", "a positioning rule")
            } else {
                ("positions glyphs", "a substitution rule")
            };
            out.push(Diagnostic::new(
                Rule::WrongLookupType,
                Severity::Error,
                shifted(range, at),
                format!("lookup {name} {does}, but it's called from {from}; compilers reject this"),
            ));
        }
    }

    // R005 judges a call only when the glyph it sees is the one the rule names:
    // the first call at that position, with no earlier call that could have
    // replaced glyphs or changed how many there are
    let mut seen_at: HashSet<usize> = HashSet::new();
    let mut sequence_may_change = false;
    for (position, name, range) in &rule.calls {
        let first_here = seen_at.insert(*position);
        let lookup = called.get(name.as_str());
        let changes_sequence = lookup.is_none_or(|l| {
            matches!(
                l.ty,
                LookupType::Multiple
                    | LookupType::Ligature
                    | LookupType::ChainSub
                    | LookupType::ReverseChainSub
            )
        });
        let judged = first_here && !sequence_may_change;
        sequence_may_change |= changes_sequence;
        if !judged {
            continue;
        }
        let Some(lookup) = lookup else {
            continue;
        };
        if lookup.ty.substitutes() != rule.ty.substitutes() {
            continue;
        }
        let Some(coverage) = &lookup.coverage else {
            continue;
        };
        glyphs.at = rule.range.start + at;
        let Some(marked) = rule.input.get(*position).and_then(|o| glyphs.set(o)) else {
            continue;
        };
        if intersection(&marked, coverage).is_empty() {
            out.push(Diagnostic::new(
                Rule::LookupHasNoEffect,
                Severity::Warning,
                shifted(range, at),
                format!(
                    "lookup {name} has no rule for {}, so it does nothing here",
                    glyphs.names(&marked, 4)
                ),
            ));
        }
    }
}

/// E003: `languagesystem` after the first feature applies only to the features after it.
fn languagesystem_order<'f>(
    pieces: impl Iterator<Item = (usize, &'f Facts)> + Clone,
    out: &mut Vec<Diagnostic>,
) {
    let Some((first_feature, first_feature_line)) = pieces
        .clone()
        .flat_map(|(at, f)| {
            f.feature_starts
                .iter()
                .map(move |s| (s.start + at, s.line_start + at))
        })
        .min()
    else {
        return;
    };
    let systems: Vec<(usize, &LanguageSystem)> = pieces
        .flat_map(|(at, f)| f.languagesystems.iter().map(move |l| (at, l)))
        .collect();
    // move late ones after the last one that's in place, or above the first feature
    let target = systems
        .iter()
        .filter(|(at, l)| l.range.start + at < first_feature)
        .map(|(at, l)| l.line_end + at)
        .max()
        .unwrap_or(first_feature_line);
    let late = systems
        .iter()
        .filter(|(at, l)| l.range.start + at > first_feature);
    for (at, system) in late {
        let mut d = Diagnostic::new(
            Rule::LanguagesystemAfterFeature,
            Severity::Warning,
            shifted(&system.range, *at),
            "languagesystem after a feature applies only to the features after it; \
             the specification requires it before every feature",
        );
        d.fixes.push(Fix::new(
            "Move it above the first feature",
            vec![
                TextEdit {
                    range: target..target,
                    new_text: format!("{}\n", system.text),
                },
                TextEdit {
                    range: shifted(&system.removal, *at),
                    new_text: String::new(),
                },
            ],
            Applicability::Unsafe,
        ));
        out.push(d);
    }
}

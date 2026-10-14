//! Hidden `feanor dev …` commands used by the test harnesses in `tools/` and for
//! benchmarking. Not part of the supported interface.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use clap::Subcommand;
use fea_rs::compile::{Compiler, NopFeatureProvider, NopVariationInfo};
use fea_rs::{GlyphIdent, GlyphMap, Node, NodeOrToken};
use feanor::internal::{first_token_mismatch, glyph_map_with_file_names, parse, parse_glyph_map};
use feanor::{Diagnostic, FormatOptions, Incremental, Indent, LintOptions, format};

use crate::inputs::{load_glyphs, read_input};

#[derive(Subcommand)]
pub(crate) enum DevCommand {
    /// Every token with its kind, byte range and enclosing node kinds, as JSON lines.
    Tokens { file: PathBuf },
    /// Format twice; check idempotence, the token stream, and that fea-rs
    /// compiles both versions to the same bytes.
    Verify {
        file: PathBuf,
        #[arg(long)]
        glyphs: Option<PathBuf>,
    },
    /// Format the file cut at 200 points (half-typed code); every cut must stay safe.
    Truncate { file: PathBuf },
    /// Apply many edits through the incremental linter and compare with full lints.
    Incremental {
        file: PathBuf,
        #[arg(long)]
        glyphs: Option<PathBuf>,
    },
    /// Compile with fea-rs and print its errors and warnings in full.
    Compile {
        file: PathBuf,
        #[arg(long)]
        glyphs: Option<PathBuf>,
        /// Write the compiled bytes here.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Use exactly the glyphs in `--glyphs` (a text file), in its order, so
        /// glyph ids match a font with that glyph order.
        #[arg(long, requires = "glyphs")]
        exact_order: bool,
    },
    /// Time lint and format in-process (median of several runs), as JSON.
    Bench {
        file: PathBuf,
        #[arg(long)]
        glyphs: Option<PathBuf>,
        /// How many times to run each.
        #[arg(long, default_value_t = 9)]
        runs: usize,
    },
}

pub(crate) fn run(command: DevCommand) -> Result<ExitCode, String> {
    match command {
        DevCommand::Tokens { file } => tokens(&file),
        DevCommand::Verify { file, glyphs } => verify(&file, glyphs.as_deref()),
        DevCommand::Truncate { file } => truncate(&file),
        DevCommand::Incremental { file, glyphs } => incremental(&file, glyphs.as_deref()),
        DevCommand::Compile {
            file,
            glyphs,
            out,
            exact_order,
        } => compile_report(&file, glyphs.as_deref(), out.as_deref(), exact_order),
        DevCommand::Bench { file, glyphs, runs } => bench(&file, glyphs.as_deref(), runs),
    }
}

fn text_of(path: &Path) -> Result<String, String> {
    read_input(path)
        .map(|i| i.text)
        .ok_or_else(|| format!("{} can't be used", path.display()))
}

fn four_spaces() -> FormatOptions {
    let mut options = FormatOptions::default();
    options.indent = Indent::Spaces(4);
    options
}

fn tokens(path: &Path) -> Result<ExitCode, String> {
    fn walk(n: &Node, pos: &mut usize, parents: &mut Vec<String>) {
        for c in n.iter_children() {
            match c {
                NodeOrToken::Node(child) => {
                    parents.push(format!("{:?}", child.kind()));
                    walk(child, pos, parents);
                    parents.pop();
                }
                NodeOrToken::Token(t) => {
                    let start = *pos;
                    *pos += t.text.len();
                    let item = serde_json::json!({
                        "kind": format!("{:?}", t.kind),
                        "text": t.text.as_str(),
                        "start": start,
                        "end": *pos,
                        "parent": parents.last(),
                        "grandparent": parents.iter().rev().nth(1),
                        "depth": parents.len(),
                    });
                    println!("{item}");
                }
            }
        }
    }
    let src = text_of(path)?;
    let tree = parse(&src, Some(&parse_glyph_map(&src)));
    walk(tree.root(), &mut 0, &mut Vec::new());
    Ok(ExitCode::SUCCESS)
}

fn fea_rs_glyphs(names: impl Iterator<Item = String>) -> Result<GlyphMap, String> {
    GlyphMap::new(names.map(|n| GlyphIdent::Name(n.as_str().into())))
        .map_err(|e| format!("glyph list: {e:?}"))
}

fn compile(src: &str, glyphs: &GlyphMap, name: &str) -> Result<Vec<u8>, String> {
    let path = std::env::temp_dir().join(format!("fea-dev-{}-{name}", std::process::id()));
    std::fs::write(&path, src).map_err(|e| e.to_string())?;
    let result = Compiler::<NopFeatureProvider, NopVariationInfo>::new(&path, glyphs)
        .print_warnings(false)
        .compile_binary()
        .map_err(|e| e.to_string());
    std::fs::remove_file(&path).ok();
    result
}

fn verify(path: &Path, glyphs: Option<&Path>) -> Result<ExitCode, String> {
    let src = text_of(path)?;
    let t = Instant::now();
    let once = format(&src, &four_spaces());
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    let twice = format(&once.text, &four_spaces());
    let idempotent = once.text == twice.text;
    let tokens = first_token_mismatch(&src, &once.text);
    println!(
        "{}: {} -> {} lines, {ms:.1} ms, {} untouched region(s)",
        path.display(),
        src.lines().count(),
        once.text.lines().count(),
        once.untouched.len()
    );
    println!(
        "  idempotent:      {}",
        if idempotent { "yes" } else { "NO" }
    );
    println!(
        "  tokens+comments: {}",
        tokens
            .as_deref()
            .map_or("identical".to_string(), |m| format!("MISMATCH {m}"))
    );
    let mut ok = idempotent && tokens.is_none();

    // the font's glyphs (when given) plus every name in the file, so stale or
    // undeclared names don't stop the compile
    let font = glyphs
        .map(load_glyphs)
        .transpose()?
        .map(|g| fea_rs_glyphs(g.iter().map(String::from)))
        .transpose()?;
    let map = glyph_map_with_file_names(&src, font.as_ref(), false);
    let wrap = |s: &str| {
        if once.feature_body {
            format!("feature zzzz {{\n{s}\n}} zzzz;\n")
        } else {
            s.to_string()
        }
    };
    match (
        compile(&wrap(&src), &map, "before.fea"),
        compile(&wrap(&once.text), &map, "after.fea"),
    ) {
        (Ok(a), Ok(b)) => {
            println!(
                "  compiled bytes:  {} ({} bytes)",
                if a == b { "identical" } else { "DIFFER" },
                a.len()
            );
            ok &= a == b;
        }
        (Err(e), _) => println!(
            "  compiled bytes:  source doesn't compile: {}",
            e.lines().next().unwrap_or("")
        ),
        (_, Err(e)) => {
            println!("  compiled bytes:  FORMATTED DOESN'T COMPILE: {e}");
            ok = false;
        }
    }
    Ok(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

fn truncate(path: &Path) -> Result<ExitCode, String> {
    let src = text_of(path)?;
    let mut cuts: Vec<usize> = (1..=200).map(|i| src.len() * i / 201).collect();
    cuts.retain(|&c| src.is_char_boundary(c));
    let (mut bad_idem, mut bad_tok, mut panics) = (0, 0, 0);
    for &cut in &cuts {
        let prefix = &src[..cut];
        let result = std::panic::catch_unwind(|| {
            let once = format(prefix, &four_spaces()).text;
            let twice = format(&once, &four_spaces()).text;
            (once == twice, first_token_mismatch(prefix, &once).is_none())
        });
        match result {
            Ok((idem, tok)) => {
                bad_idem += usize::from(!idem);
                bad_tok += usize::from(!tok);
            }
            Err(_) => panics += 1,
        }
    }
    println!(
        "{}: {} cuts, non-idempotent {bad_idem}, token mismatch {bad_tok}, panics {panics}",
        path.display(),
        cuts.len()
    );
    Ok(if bad_idem + bad_tok + panics == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

fn incremental(path: &Path, glyphs: Option<&Path>) -> Result<ExitCode, String> {
    let src = text_of(path)?;
    let font = glyphs.map(load_glyphs).transpose()?;
    let options = match &font {
        Some(g) => LintOptions::default().with_glyphs(g),
        None => LintOptions::default(),
    };
    let key = |ds: &[Diagnostic]| -> Vec<String> {
        ds.iter()
            .map(|d| {
                format!(
                    "{} {:?} {:?} {} {:?} {:?}",
                    d.rule, d.severity, d.range, d.message, d.fixes, d.related
                )
            })
            .collect()
    };
    let t = Instant::now();
    let mut doc = Incremental::new(src.clone(), options);
    let build_ms = t.elapsed().as_secs_f64() * 1000.0;

    let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
    let mut rand = |n: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        usize::try_from(seed % u64::try_from(n.max(1)).unwrap_or(1)).unwrap_or(0)
    };
    // typing a rule keystroke by keystroke, then deleting it; then random characters
    let mut steps: Vec<(usize, usize, String)> = Vec::new();
    let line_starts: Vec<usize> = src.match_indices("\n    ").map(|(i, _)| i + 1).collect();
    for _ in 0..12 {
        if line_starts.is_empty() {
            break;
        }
        let at = line_starts[rand(line_starts.len())];
        let typed = "    sub a by b; # typed\n";
        steps.extend(
            typed
                .char_indices()
                .map(|(k, ch)| (at + k, at + k, ch.to_string())),
        );
        steps.extend(
            (0..typed.len())
                .rev()
                .map(|k| (at + k, at + k + 1, String::new())),
        );
    }
    let alphabet = ['a', ' ', ';', '{', '}', '[', ']', '#', '\n', '@', '\''];
    let mut len = src.len();
    for _ in 0..150 {
        let at = rand(len);
        if rand(2) == 0 {
            let ch = alphabet[rand(alphabet.len())];
            steps.push((at, at, ch.to_string()));
            len += ch.len_utf8();
        } else if len > 1 {
            steps.push((at, at + 1, String::new()));
            len -= 1;
        }
    }

    let (mut inc_ms, mut full_ms) = (Vec::new(), Vec::new());
    let (mut mismatches, mut rebuilds) = (0, 0);
    let check_every = if src.len() > 300_000 { 5 } else { 1 };
    for (n, (s0, s1, text)) in steps.iter().enumerate() {
        let doc_len = doc.text().len();
        let (mut s0, mut s1) = ((*s0).min(doc_len), (*s1).min(doc_len));
        while !doc.text().is_char_boundary(s0) {
            s0 -= 1;
        }
        while !doc.text().is_char_boundary(s1) {
            s1 += 1;
        }
        let t = Instant::now();
        let stats = doc.edit(s0..s1, text);
        let got = doc.diagnostics();
        inc_ms.push(t.elapsed().as_secs_f64() * 1000.0);
        rebuilds += usize::from(stats.full_rebuild);
        if (n + 1) % check_every != 0 {
            continue;
        }
        let t = Instant::now();
        let want = feanor::lint(doc.text(), &options);
        full_ms.push(t.elapsed().as_secs_f64() * 1000.0);
        if key(&got) != key(&want) {
            mismatches += 1;
        }
    }
    let pct = |v: &mut Vec<f64>, p: f64| {
        v.sort_by(f64::total_cmp);
        v.get(((v.len().saturating_sub(1)) as f64 * p) as usize)
            .copied()
            .unwrap_or(0.0)
    };
    println!(
        "{}: {} edits, build {build_ms:.1} ms; incremental median {:.2} ms, p95 {:.2} ms; full lint median {:.2} ms; {rebuilds} rebuild(s); {mismatches} mismatch(es) in {} checked",
        path.display(),
        steps.len(),
        pct(&mut inc_ms.clone(), 0.5),
        pct(&mut inc_ms, 0.95),
        pct(&mut full_ms.clone(), 0.5),
        full_ms.len()
    );
    Ok(if mismatches == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

/// Median wall time of `runs` calls, in milliseconds.
fn median_ms(runs: usize, mut f: impl FnMut()) -> f64 {
    let mut times: Vec<f64> = (0..runs.max(1))
        .map(|_| {
            let t = Instant::now();
            f();
            t.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    times.sort_by(f64::total_cmp);
    times[times.len() / 2]
}

fn bench(path: &Path, glyphs: Option<&Path>, runs: usize) -> Result<ExitCode, String> {
    let src = text_of(path)?;
    let font = glyphs.map(load_glyphs).transpose()?;
    let options = match &font {
        Some(g) => LintOptions::default().with_glyphs(g),
        None => LintOptions::default(),
    };
    let lint_ms = median_ms(runs, || {
        std::hint::black_box(feanor::lint(&src, &options));
    });
    let format_ms = median_ms(runs, || {
        std::hint::black_box(format(&src, &FormatOptions::default()));
    });
    let report = serde_json::json!({
        "file": path.display().to_string(),
        "lines": src.lines().count(),
        "bytes": src.len(),
        "lint_ms": (lint_ms * 100.0).round() / 100.0,
        "format_ms": (format_ms * 100.0).round() / 100.0,
        "runs": runs,
    });
    println!("{report}");
    Ok(ExitCode::SUCCESS)
}

fn compile_report(
    path: &Path,
    glyphs: Option<&Path>,
    out: Option<&Path>,
    exact_order: bool,
) -> Result<ExitCode, String> {
    let src = text_of(path)?;
    let map = match glyphs {
        Some(list) if exact_order => {
            let names = text_of(list)?;
            let names = names.lines().map(str::trim).filter(|n| !n.is_empty());
            fea_rs_glyphs(names.map(String::from))?
        }
        _ => {
            let font = glyphs
                .map(load_glyphs)
                .transpose()?
                .map(|g| fea_rs_glyphs(g.iter().map(String::from)))
                .transpose()?;
            glyph_map_with_file_names(&src, font.as_ref(), false)
        }
    };
    let result = Compiler::<NopFeatureProvider, NopVariationInfo>::new(path, &map)
        .print_warnings(true)
        .compile_binary();
    match result {
        Ok(bytes) => {
            if let Some(out) = out {
                std::fs::write(out, &bytes).map_err(|e| format!("{}: {e}", out.display()))?;
            }
            // a fingerprint, to tell whether two sources compile to the same tables
            let mut hasher = std::hash::DefaultHasher::new();
            std::hash::Hash::hash(&bytes, &mut hasher);
            let fingerprint = std::hash::Hasher::finish(&hasher);
            println!(
                "{}: compiled ({} bytes, {fingerprint:016x})",
                path.display(),
                bytes.len()
            );
            Ok(ExitCode::SUCCESS)
        }
        Err(e) => {
            println!("{}: failed\n{}", path.display(), e.display_verbose());
            Ok(ExitCode::FAILURE)
        }
    }
}

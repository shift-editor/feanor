//! `fea`: format and lint OpenType feature files.

mod dev;
mod inputs;
mod output;

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use clap::{Args, Parser, Subcommand, ValueEnum};
use fea_tools::{
    Applicability, BuildConfig, Config, Diagnostic, Rule, Severity, apply_fixes, apply_noqa,
    configurations, format, lint,
};

use crate::inputs::{Input, load_glyphs, read_input};
use crate::output::{print_diagnostic, print_diff, print_statistics};

/// Format and lint OpenType feature files (.fea).
///
/// Settings come from the nearest fea.toml at or above each file.
#[derive(Parser)]
#[command(name = "fea", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Format files. Prints the result, or rewrites the files with --write.
    Fmt {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// Rewrite the files in place.
        #[arg(short, long, conflicts_with_all = ["check", "diff"])]
        write: bool,
        /// Exit with status 1 if any file isn't formatted; print nothing else.
        #[arg(long)]
        check: bool,
        /// Show what formatting would change, as a unified diff.
        #[arg(long)]
        diff: bool,
        #[command(flatten)]
        style: StyleArgs,
    },
    /// Lint files, each on its own.
    Lint {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        #[command(flatten)]
        lint: LintArgs,
        /// Apply safe fixes and rewrite the files.
        #[arg(long)]
        fix: bool,
        /// With --fix, also apply unsafe fixes (renames to suggestions).
        #[arg(long, requires = "fix")]
        unsafe_fixes: bool,
    },
    /// Lint a whole project: a root file with its includes, or a .txt manifest of chunks.
    Project {
        root: PathBuf,
        #[command(flatten)]
        lint: LintArgs,
        /// Lint every #ifdef build configuration and merge the results.
        #[arg(long)]
        all_builds: bool,
    },
    /// For CI: fail when any project file isn't formatted or lint finds an error.
    Check {
        root: PathBuf,
        #[command(flatten)]
        lint: LintArgs,
    },
    /// List the rules, or explain one (`fea rule G001`).
    Rule { code: Option<String> },
    /// Development and test-harness commands.
    #[command(hide = true, subcommand)]
    Dev(dev::DevCommand),
}

#[derive(Args, Clone)]
struct StyleArgs {
    /// Line width (overrides fea.toml).
    #[arg(long)]
    line_width: Option<usize>,
    /// Indent: a number of spaces, `tab`, or `auto` (overrides fea.toml).
    #[arg(long)]
    indent: Option<String>,
    /// Ignore fea.toml.
    #[arg(long)]
    no_config: bool,
}

#[derive(Args, Clone)]
struct LintArgs {
    /// The font's glyphs: a UFO, or a text file with one name per line.
    #[arg(long)]
    glyphs: Option<PathBuf>,
    /// Define a name for #ifdef (repeatable).
    #[arg(short = 'D', long = "define")]
    defines: Vec<String>,
    /// Rules to report: codes, families (C) or names, comma-separated.
    #[arg(long, value_delimiter = ',')]
    select: Vec<String>,
    /// Rules to skip, added to fea.toml's.
    #[arg(long, value_delimiter = ',')]
    ignore: Vec<String>,
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    format: OutputFormat,
    /// Print counts per rule instead of each diagnostic.
    #[arg(long)]
    statistics: bool,
    /// Ignore fea.toml.
    #[arg(long)]
    no_config: bool,
}

#[derive(ValueEnum, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutputFormat {
    Human,
    /// GitHub Actions annotations.
    Github,
    Json,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Fmt {
            files,
            write,
            check,
            diff,
            style,
        } => run_fmt(&files, write, check, diff, &style),
        Command::Lint {
            files,
            lint,
            fix,
            unsafe_fixes,
        } => run_lint(&files, &lint, fix, unsafe_fixes),
        Command::Project {
            root,
            lint,
            all_builds,
        } => run_project(&root, &lint, all_builds),
        Command::Check { root, lint } => run_check(&root, &lint),
        Command::Rule { code } => run_rule(code.as_deref()),
        Command::Dev(command) => dev::run(command),
    };
    match result {
        Ok(status) => status,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::from(2)
        }
    }
}

/// Exit status: 0 when nothing was found, 1 when something was.
fn found(problems: bool) -> ExitCode {
    if problems {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn config_for(path: &std::path::Path, no_config: bool) -> Result<Config, String> {
    if no_config {
        return Ok(Config::default());
    }
    Config::discover(path).map_err(|e| e.to_string())
}

fn style_options(config: &Config, style: &StyleArgs) -> Result<fea_tools::FormatOptions, String> {
    let mut options = config.format_options();
    if let Some(w) = style.line_width {
        options.line_width = w;
    }
    if let Some(i) = &style.indent {
        options.indent =
            match i.as_str() {
                "tab" => fea_tools::Indent::Tab,
                "auto" => fea_tools::Indent::Auto,
                n => fea_tools::Indent::Spaces(n.parse().map_err(|_| {
                    format!("--indent: expected a number, `tab` or `auto`, got `{n}`")
                })?),
            };
    }
    Ok(options)
}

fn run_fmt(
    files: &[PathBuf],
    write: bool,
    check: bool,
    diff: bool,
    style: &StyleArgs,
) -> Result<ExitCode, String> {
    let mut changed = 0;
    let mut unreadable = 0;
    for path in files {
        let Some(Input { text, .. }) = read_input(path) else {
            unreadable += 1;
            continue;
        };
        let config = config_for(path, style.no_config)?;
        let out = format(&text, &style_options(&config, style)?);
        let differs = out.text != text;
        changed += usize::from(differs);
        match (write, check, diff) {
            (true, _, _) if differs => {
                std::fs::write(path, &out.text).map_err(|e| format!("{}: {e}", path.display()))?
            }
            (true, _, _) => {}
            (_, true, _) if differs => println!("would reformat {}", path.display()),
            (_, true, _) => {}
            (_, _, true) => print_diff(&path.display().to_string(), &text, &out.text),
            _ => print!("{}", out.text),
        }
    }
    if check || diff {
        eprintln!("{changed} of {} file(s) would be reformatted", files.len());
    }
    if unreadable > 0 {
        return Ok(ExitCode::from(2));
    }
    Ok(found((check || diff) && changed > 0))
}

fn run_lint(
    files: &[PathBuf],
    args: &LintArgs,
    fix: bool,
    unsafe_fixes: bool,
) -> Result<ExitCode, String> {
    let mut all: Vec<(String, String, Vec<Diagnostic>)> = Vec::new();
    let mut unreadable = 0;
    let t = Instant::now();
    for path in files {
        let Some(Input { text, .. }) = read_input(path) else {
            unreadable += 1;
            continue;
        };
        let config = config_for(path, args.no_config)?;
        let glyph_path = args.glyphs.clone().or(config.glyphs.clone());
        let glyphs = glyph_path.as_deref().map(load_glyphs).transpose()?;
        let options = match &glyphs {
            Some(g) => fea_tools::LintOptions::default().with_glyphs(g),
            None => fea_tools::LintOptions::default(),
        };
        let mut diags = lint(&text, &options);
        let mut selection = config.selection_for(path);
        if !args.select.is_empty() {
            selection.select = args.select.clone();
        }
        selection.ignore.extend(args.ignore.iter().cloned());
        selection.retain(&mut diags);
        apply_noqa(&text, &mut diags);
        if fix {
            let allowed = if unsafe_fixes {
                Applicability::Unsafe
            } else {
                Applicability::Safe
            };
            let fixed = apply_fixes(&text, &diags, allowed);
            if fixed != text {
                std::fs::write(path, &fixed).map_err(|e| format!("{}: {e}", path.display()))?;
                // report what's left
                diags = lint(&fixed, &options);
                selection.retain(&mut diags);
                apply_noqa(&fixed, &mut diags);
                all.push((path.display().to_string(), fixed, diags));
                continue;
            }
        }
        all.push((path.display().to_string(), text, diags));
    }
    let errors = report(&all, args.format, args.statistics);
    if args.format == OutputFormat::Human && !args.statistics {
        let total: usize = all.iter().map(|f| f.2.len()).sum();
        eprintln!(
            "{total} diagnostic(s) in {} file(s), {:.1} ms",
            all.len(),
            t.elapsed().as_secs_f64() * 1000.0
        );
    }
    if unreadable > 0 {
        return Ok(ExitCode::from(2));
    }
    Ok(found(errors > 0))
}

/// Print diagnostics for several files; returns how many are errors.
fn report(
    files: &[(String, String, Vec<Diagnostic>)],
    format: OutputFormat,
    statistics: bool,
) -> usize {
    let errors = files
        .iter()
        .flat_map(|f| &f.2)
        .filter(|d| d.severity == Severity::Error)
        .count();
    if statistics {
        print_statistics(files.iter().flat_map(|f| &f.2));
        return errors;
    }
    if format == OutputFormat::Json {
        let items: Vec<serde_json::Value> = files
            .iter()
            .flat_map(|(path, text, diags)| diags.iter().map(move |d| output::json(path, text, d)))
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&items).unwrap_or_default()
        );
        return errors;
    }
    for (path, text, diags) in files {
        for d in diags {
            print_diagnostic(format, path, text, d);
        }
    }
    errors
}

fn project_inputs(
    root: &std::path::Path,
    args: &LintArgs,
) -> Result<(Config, Option<fea_tools::GlyphNames>), String> {
    let config = config_for(root, args.no_config)?;
    let glyph_path = args.glyphs.clone().or(config.glyphs.clone());
    let glyphs = glyph_path.as_deref().map(load_glyphs).transpose()?;
    Ok((config, glyphs))
}

fn load_project(root: &std::path::Path, defines: &[String]) -> Result<fea_tools::Project, String> {
    let manifest = root.extension().is_some_and(|e| e == "txt");
    let loaded = if manifest {
        fea_tools::Project::from_manifest(root)
    } else {
        fea_tools::Project::from_root(root, defines)
    };
    loaded.map_err(|e| e.to_string())
}

/// Lint a project once per build configuration and merge: an unused class or
/// lookup counts only when it's unused in every build that contains it, and
/// anything found in some builds only says which.
fn lint_builds(
    root: &std::path::Path,
    args: &LintArgs,
    config: &Config,
    glyphs: Option<&fea_tools::GlyphNames>,
    all_builds: bool,
) -> Result<Vec<(String, String, Vec<Diagnostic>)>, String> {
    let mut defines = config.defines.clone();
    defines.extend(args.defines.iter().cloned());
    let builds = if all_builds {
        configurations(root, &defines).map_err(|e| e.to_string())?
    } else {
        vec![BuildConfig { defines }]
    };
    let options = match glyphs {
        Some(g) => fea_tools::LintOptions::default().with_glyphs(g),
        None => fea_tools::LintOptions::default(),
    };

    struct Found {
        path: PathBuf,
        d: Diagnostic,
        builds: Vec<usize>,
    }
    let mut found: Vec<Found> = Vec::new();
    let mut projects = Vec::new();
    for (b, build) in builds.iter().enumerate() {
        let project = load_project(root, &build.defines)?;
        for f in project.lint(&options) {
            let path = project.files()[f.file].clone();
            match found
                .iter_mut()
                .find(|x| x.path == path && x.d == f.diagnostic)
            {
                Some(x) => x.builds.push(b),
                None => found.push(Found {
                    path,
                    d: f.diagnostic,
                    builds: vec![b],
                }),
            }
        }
        projects.push(project);
    }

    let base = root.parent().unwrap_or(std::path::Path::new("."));
    let mut by_file: Vec<(String, String, Vec<Diagnostic>)> = Vec::new();
    for mut f in found {
        let present: Vec<usize> = (0..builds.len())
            .filter(|&b| projects[b].contains(&f.path, f.d.range.start))
            .collect();
        let everywhere = present.iter().all(|b| f.builds.contains(b));
        let unused = matches!(f.d.rule, Rule::UnusedClass | Rule::UnusedLookup);
        if unused && !everywhere {
            continue;
        }
        if builds.len() > 1 && (!everywhere || present.len() < builds.len()) {
            let labels: Vec<String> = f.builds.iter().map(|&b| builds[b].to_string()).collect();
            f.d.message
                .push_str(&format!(" (only in: {})", labels.join(", ")));
        }
        let shown = f
            .path
            .strip_prefix(base)
            .unwrap_or(&f.path)
            .display()
            .to_string();
        let text = std::fs::read_to_string(&f.path).unwrap_or_default();
        let mut selection = config.selection_for(&f.path);
        if !args.select.is_empty() {
            selection.select = args.select.clone();
        }
        selection.ignore.extend(args.ignore.iter().cloned());
        if !selection.allows(f.d.rule) {
            continue;
        }
        let mut one = vec![f.d];
        apply_noqa(&text, &mut one);
        let Some(d) = one.pop() else { continue };
        match by_file.iter_mut().find(|x| x.0 == shown) {
            Some(entry) => entry.2.push(d),
            None => by_file.push((shown, text, vec![d])),
        }
    }
    by_file.sort_by(|a, b| a.0.cmp(&b.0));
    for entry in &mut by_file {
        entry.2.sort_by_key(|d| d.range.start);
    }
    Ok(by_file)
}

fn run_project(
    root: &std::path::Path,
    args: &LintArgs,
    all_builds: bool,
) -> Result<ExitCode, String> {
    let t = Instant::now();
    let (config, glyphs) = project_inputs(root, args)?;
    let files = lint_builds(root, args, &config, glyphs.as_ref(), all_builds)?;
    let errors = report(&files, args.format, args.statistics);
    if args.format == OutputFormat::Human && !args.statistics {
        let total: usize = files.iter().map(|f| f.2.len()).sum();
        eprintln!(
            "{total} diagnostic(s), {:.1} ms",
            t.elapsed().as_secs_f64() * 1000.0
        );
    }
    Ok(found(errors > 0))
}

fn run_check(root: &std::path::Path, args: &LintArgs) -> Result<ExitCode, String> {
    let (config, glyphs) = project_inputs(root, args)?;
    let project = load_project(root, &config.defines)?;
    let base = root.parent().unwrap_or(std::path::Path::new("."));
    let mut unformatted = 0;
    for (i, path) in project.files().iter().enumerate() {
        let text = project.text(i);
        let options = config_for(path, args.no_config)?.format_options();
        if format(text, &options).text != text {
            unformatted += 1;
            let shown = path.strip_prefix(base).unwrap_or(path).display();
            match args.format {
                OutputFormat::Github => println!(
                    "::warning file={shown},title=fea fmt::file is not formatted (run `fea fmt --write`)"
                ),
                _ => println!("not formatted: {shown}"),
            }
        }
    }
    let files = lint_builds(root, args, &config, glyphs.as_ref(), true)?;
    let errors = report(&files, args.format, args.statistics);
    eprintln!(
        "{unformatted} of {} file(s) need formatting; {errors} lint error(s)",
        project.files().len()
    );
    Ok(found(errors > 0 || unformatted > 0))
}

fn run_rule(code: Option<&str>) -> Result<ExitCode, String> {
    let Some(code) = code else {
        for rule in Rule::ALL {
            println!("{}  {:<26} {}", rule.code(), rule.name(), rule.summary());
        }
        return Ok(ExitCode::SUCCESS);
    };
    let rule = Rule::from_code_or_name(code)
        .ok_or_else(|| format!("no rule `{code}` (see `fea rule`)"))?;
    print!("{}", rule.explanation());
    Ok(ExitCode::SUCCESS)
}

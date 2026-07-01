//! Conformance checker for `oxc-css-parser`.
//!
//! Clones a set of upstream CSS / preprocessor test corpora, each pinned to a
//! fixed commit SHA, then runs every CSS-family file (`.css`/`.scss`/`.sass`/
//! `.less` — all four syntaxes the parser supports) through the parser and
//! writes committed snapshot files under `tasks/conformance/snapshots/`.
//! (sass-spec packs its tests in `.hrx` archives, which are unpacked into their
//! `.scss`/`.sass`/`.css` entries.) The snapshots are:
//!
//! - `summary.snap` — success/failed counts per suite + total + per syntax.
//! - `<suite>.snap` — the sorted list of files that failed in that suite.
//!
//! `success` is a clean parse (zero errors); `failed` is `recovered +
//! hard_error + panic`. Regenerate the snapshots by re-running, and review
//! changes via `git diff` — that is how regressions/improvements surface.
//!
//! Pinned SHAs keep runs reproducible; bump them deliberately to ingest
//! upstream changes. Cloned repos live under `tasks/conformance/repos/` (git
//! ignored) and are fetched shallow + blobless + sparse to stay small.
//!
//! Tracking issue: <https://github.com/oxc-project/oxc-css-parser/issues/19>.
//!
//! ```text
//! cargo run -p conformance                 # clone + parse all suites, write snapshots
//! cargo run -p conformance -- sass-spec     # only the named suite(s) (summary not rewritten)
//! cargo run -p conformance -- --clone       # clone/update only, do not parse
//! cargo run -p conformance -- --clean       # remove all cloned repos
//! ```

use std::{
    fmt::Write as _,
    fs,
    io::{self, Write},
    panic,
    path::{Path, PathBuf},
    process::Command,
};

use oxc_css_parser::{Allocator, Parser, Syntax, ast::Stylesheet};

/// An upstream test corpus, pinned to a fixed commit.
struct Suite {
    /// Directory name under `tasks/conformance/repos/`, and the CLI selector.
    name: &'static str,
    /// Git remote to clone from.
    url: &'static str,
    /// Pinned commit SHA. Bump deliberately to ingest upstream changes.
    sha: &'static str,
    /// Cone-mode sparse-checkout directories; empty means a full checkout.
    sparse: &'static [&'static str],
    /// Sub-path (relative to the repo root) scanned for parseable files.
    walk: &'static str,
    /// Note shown in the report — e.g. which phase wires up its real harness.
    note: &'static str,
}

/// The conformance corpora, pinned. See issue #19 for the rationale behind each.
const SUITES: &[Suite] = &[
    Suite {
        name: "css-parsing-tests",
        url: "https://github.com/SimonSapin/css-parsing-tests.git",
        sha: "203ce36bffd617db7f118c551e32794561fb273d",
        sparse: &[],
        walk: "",
        note: "CSS Syntax L3, JSON input->tree — needs a dedicated adapter",
    },
    Suite {
        name: "wpt",
        url: "https://github.com/web-platform-tests/wpt.git",
        sha: "1722fb6566acac7b0fc7bfc9aae55a47594b9d03",
        sparse: &["css/css-syntax"],
        walk: "css/css-syntax",
        note: "Phase 3 — testharness assertions need an HTML/JS harness",
    },
    Suite {
        name: "csswg-drafts",
        url: "https://github.com/w3c/csswg-drafts.git",
        sha: "cca93bb94ae073c964ffe076bbe75d6baef90dd6",
        sparse: &[
            "css-syntax-3",
            "selectors-4",
            "css-color-4",
            "css-values-4",
            "mediaqueries-5",
            "css-conditional-5",
            "css-ui-4",
            "scroll-animations-1",
            "css-cascade-5",
        ],
        walk: "",
        note: "Phase 2 — extract examples from Bikeshed (.bs) sources",
    },
    Suite {
        name: "webref",
        url: "https://github.com/w3c/webref.git",
        sha: "9cce6ee56b9b281df9a81baa4cfc4a931e103333",
        sparse: &["ed/css"],
        walk: "ed/css",
        note: "Phase 4 — spec-surface coverage data (JSON), not parsed as CSS",
    },
    Suite {
        name: "postcss-parser-tests",
        url: "https://github.com/postcss/postcss-parser-tests.git",
        sha: "de1bc546de3678dd1c85e57cb2e9eece0098ddb9",
        sparse: &[],
        walk: "cases",
        note: "real-world CSS edge cases",
    },
    Suite {
        name: "sass-spec",
        url: "https://github.com/sass/sass-spec.git",
        sha: "a2ead9225786d49e91f5cc36755b7713596a2338",
        sparse: &["spec"],
        walk: "spec",
        note: "canonical Sass/SCSS suite; tests packed in .hrx archives (unpacked)",
    },
    Suite {
        name: "less.js",
        url: "https://github.com/less/less.js.git",
        sha: "8ae2cc3bfa79f0718ad6fe5f263a1d6819fe9d5c",
        sparse: &["packages/test-data"],
        walk: "packages/test-data",
        note: "Less reference suite (tests compilation; we parse only)",
    },
];

fn repos_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("repos")
}

fn snapshots_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("snapshots")
}

fn git(dir: &Path, args: &[&str]) -> io::Result<std::process::Output> {
    Command::new("git").arg("-C").arg(dir).args(args).output()
}

fn git_ok(dir: &Path, args: &[&str]) -> io::Result<()> {
    let output = git(dir, args)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "`git {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

/// Clone or update `suite` to its pinned SHA. Returns `Ok(true)` if a network
/// fetch happened, `Ok(false)` if the checkout was already at the pinned SHA.
fn ensure_repo(suite: &Suite) -> io::Result<bool> {
    let dir = repos_dir().join(suite.name);

    if !dir.join(".git").is_dir() {
        fs::create_dir_all(&dir)?;
        git_ok(&dir, &["init", "-q"])?;
    }

    let has_origin = git(&dir, &["remote", "get-url", "origin"]).is_ok_and(|o| o.status.success());
    if !has_origin {
        git_ok(&dir, &["remote", "add", "origin", suite.url])?;
    }

    // Already checked out at the pinned SHA — nothing to do.
    if let Ok(out) = git(&dir, &["rev-parse", "HEAD"])
        && out.status.success()
        && String::from_utf8_lossy(&out.stdout).trim() == suite.sha
    {
        return Ok(false);
    }

    let sparse = !suite.sparse.is_empty();
    if sparse {
        git_ok(&dir, &["sparse-checkout", "init", "--cone"])?;
        let mut args = vec!["sparse-checkout", "set"];
        args.extend_from_slice(suite.sparse);
        git_ok(&dir, &args)?;
    }

    // GitHub allows fetching an arbitrary commit by SHA. `--depth 1` skips
    // history; for sparse checkouts we also add `--filter=blob:none` so only the
    // in-cone blobs are pulled (keeps huge repos like wpt/csswg-drafts small).
    // For full checkouts we skip the filter — it would just force a second,
    // flakier round-trip to lazily fetch every blob at checkout time.
    let mut fetch = vec!["fetch", "-q", "--depth", "1"];
    if sparse {
        fetch.push("--filter=blob:none");
    }
    fetch.extend_from_slice(&["origin", suite.sha]);
    git_ok(&dir, &fetch)?;
    git_ok(&dir, &["checkout", "-q", "FETCH_HEAD"])?;
    Ok(true)
}

/// The outcome of parsing one file. Non-clean variants carry a span-free error
/// label (the first error's `ErrorKind`) for the failures snapshot.
enum Outcome {
    Clean,
    Recovered(String),
    HardError(String),
    Panic,
}

fn parse_outcome(source: &str, syntax: Syntax) -> Outcome {
    let caught = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        let allocator = Allocator::default();
        let mut parser = Parser::new(&allocator, source, syntax);
        match parser.parse::<Stylesheet>() {
            Ok(_) => match parser.recoverable_errors().first() {
                None => Outcome::Clean,
                Some(error) => Outcome::Recovered(error.kind.to_string()),
            },
            Err(error) => Outcome::HardError(error.kind.to_string()),
        }
    }));
    caught.unwrap_or(Outcome::Panic)
}

fn syntax_for(path: &Path) -> Option<Syntax> {
    syntax_for_ext(path.extension()?.to_str()?)
}

/// Map a bare file extension (no dot) to a syntax.
fn syntax_for_ext(ext: &str) -> Option<Syntax> {
    match ext {
        "css" => Some(Syntax::Css),
        "scss" => Some(Syntax::Scss),
        "sass" => Some(Syntax::Sass),
        "less" => Some(Syntax::Less),
        _ => None,
    }
}

/// Map an HRX entry path (e.g. `scss/input.scss`) to a syntax.
fn syntax_for_entry(name: &str) -> Option<Syntax> {
    let base = name.rsplit('/').next().unwrap_or(name);
    syntax_for_ext(base.rsplit_once('.')?.1)
}

fn is_hrx(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "hrx")
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let is_git = path.file_name().is_some_and(|name| name == ".git");
            if !is_git {
                collect_files(&path, out);
            }
        } else if syntax_for(&path).is_some() || is_hrx(&path) {
            out.push(path);
        }
    }
}

#[derive(Default, Clone)]
struct Tally {
    files: u32,
    clean: u32,
    recovered: u32,
    hard_error: u32,
    panic: u32,
}

impl Tally {
    fn record(&mut self, outcome: &Outcome) {
        self.files += 1;
        match outcome {
            Outcome::Clean => self.clean += 1,
            Outcome::Recovered(_) => self.recovered += 1,
            Outcome::HardError(_) => self.hard_error += 1,
            Outcome::Panic => self.panic += 1,
        }
    }

    fn add(&mut self, other: &Tally) {
        self.files += other.files;
        self.clean += other.clean;
        self.recovered += other.recovered;
        self.hard_error += other.hard_error;
        self.panic += other.panic;
    }

    /// A clean parse with zero errors.
    fn success(&self) -> u32 {
        self.clean
    }

    /// Anything that is not a clean parse.
    fn failed(&self) -> u32 {
        self.recovered + self.hard_error + self.panic
    }
}

/// Per-syntax tallies, so the summary can report coverage across all four
/// syntaxes the parser supports.
#[derive(Default)]
struct BySyntax {
    css: Tally,
    scss: Tally,
    sass: Tally,
    less: Tally,
}

impl BySyntax {
    fn get_mut(&mut self, syntax: Syntax) -> &mut Tally {
        match syntax {
            Syntax::Css => &mut self.css,
            Syntax::Scss => &mut self.scss,
            Syntax::Sass => &mut self.sass,
            Syntax::Less => &mut self.less,
        }
    }

    fn add(&mut self, other: &BySyntax) {
        self.css.add(&other.css);
        self.scss.add(&other.scss);
        self.sass.add(&other.sass);
        self.less.add(&other.less);
    }
}

/// One failing file: `tag` is `RECOVER`/`ERROR`/`PANIC`, `rel_path` is relative
/// to the suite repo root, `label` is the error kind (empty for panics).
struct Failure {
    tag: &'static str,
    rel_path: String,
    label: String,
}

#[derive(Default)]
struct SuiteReport {
    tally: Tally,
    by_syntax: BySyntax,
    failures: Vec<Failure>,
}

/// Render a path relative to `base` using forward slashes (stable across
/// platforms, avoids `str::replace`).
fn rel_path(path: &Path, base: &Path) -> String {
    let rel = path.strip_prefix(base).unwrap_or(path);
    rel.components().filter_map(|c| c.as_os_str().to_str()).collect::<Vec<_>>().join("/")
}

fn run_suite(suite: &Suite) -> SuiteReport {
    let suite_root = repos_dir().join(suite.name);
    let mut files = Vec::new();
    collect_files(&suite_root.join(suite.walk), &mut files);

    let mut report = SuiteReport::default();
    for path in files {
        if let Some(syntax) = syntax_for(&path) {
            if let Ok(source) = fs::read_to_string(&path) {
                process_unit(rel_path(&path, &suite_root), &source, syntax, &mut report);
            }
        } else if let Ok(archive) = fs::read_to_string(&path) {
            // sass-spec packs each test in an `.hrx` archive; parse its entries.
            let base = rel_path(&path, &suite_root);
            for (entry, source) in parse_hrx(&archive) {
                if let Some(syntax) = syntax_for_entry(&entry) {
                    process_unit(format!("{base}::{entry}"), &source, syntax, &mut report);
                }
            }
        }
    }
    report.failures.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    report
}

/// Parse one CSS unit and fold its outcome into `report`.
fn process_unit(rel_path: String, source: &str, syntax: Syntax, report: &mut SuiteReport) {
    let outcome = parse_outcome(source, syntax);
    report.tally.record(&outcome);
    report.by_syntax.get_mut(syntax).record(&outcome);
    let failure = match outcome {
        Outcome::Clean => return,
        Outcome::Recovered(label) => Failure { tag: "RECOVER", rel_path, label },
        Outcome::HardError(label) => Failure { tag: "ERROR", rel_path, label },
        Outcome::Panic => Failure { tag: "PANIC", rel_path, label: String::new() },
    };
    report.failures.push(failure);
}

/// A line that delimits sections in an HRX archive.
enum Boundary {
    /// `<===> path` — starts a file entry.
    File(String),
    /// `<===>` — starts a comment section (ignored).
    Comment,
}

fn hrx_boundary(raw: &str) -> Option<Boundary> {
    let line = raw.trim_end_matches(['\n', '\r']);
    let after_lt = line.strip_prefix('<')?;
    let eqs = after_lt.len() - after_lt.trim_start_matches('=').len();
    if eqs == 0 {
        return None; // a boundary needs at least one '='
    }
    let after_gt = after_lt[eqs..].strip_prefix('>')?;
    match after_gt.strip_prefix(' ') {
        Some(path) => Some(Boundary::File(path.to_string())),
        None if after_gt.is_empty() => Some(Boundary::Comment),
        None => None, // `<==>x` without a space is not a boundary
    }
}

/// Unpack an HRX archive (<https://github.com/google/hrx>) into `(path, content)`
/// entries. Lenient: any run of `=` between `<` and `>` is treated as a boundary.
fn parse_hrx(text: &str) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    let mut path: Option<String> = None;
    let mut content = String::new();
    for line in text.split_inclusive('\n') {
        match hrx_boundary(line) {
            Some(boundary) => {
                if let Some(p) = path.take() {
                    entries.push((p, std::mem::take(&mut content)));
                } else {
                    content.clear();
                }
                path = match boundary {
                    Boundary::File(p) => Some(p),
                    Boundary::Comment => None,
                };
            }
            None => content.push_str(line),
        }
    }
    if let Some(p) = path {
        entries.push((p, content));
    }
    entries
}

fn header_row(first: &str) -> String {
    format!(
        "{first:<22} {:>6} {:>8} {:>7} {:>7} {:>6} {:>8} {:>6}",
        "files", "success", "failed", "clean", "recov", "harderr", "panic"
    )
}

fn row(label: &str, t: &Tally) -> String {
    format!(
        "{label:<22} {:>6} {:>8} {:>7} {:>7} {:>6} {:>8} {:>6}",
        t.files,
        t.success(),
        t.failed(),
        t.clean,
        t.recovered,
        t.hard_error,
        t.panic
    )
}

fn write_suite_snapshot(suite: &Suite, report: &SuiteReport) -> io::Result<()> {
    let t = &report.tally;
    let mut out = String::new();
    let _ = writeln!(out, "suite: {}", suite.name);
    let _ = writeln!(out, "sha: {}", suite.sha);
    let _ = writeln!(out, "files: {}   success: {}   failed: {}", t.files, t.success(), t.failed());
    let _ = writeln!(
        out,
        "clean: {}  recovered: {}  hard_error: {}  panic: {}",
        t.clean, t.recovered, t.hard_error, t.panic
    );
    let _ = writeln!(out, "\nfailures:");
    if report.failures.is_empty() {
        let _ = writeln!(out, "none");
    }
    for failure in &report.failures {
        if failure.label.is_empty() {
            let _ = writeln!(out, "{:<8} {}", failure.tag, failure.rel_path);
        } else {
            let _ = writeln!(out, "{:<8} {}    {}", failure.tag, failure.rel_path, failure.label);
        }
    }
    fs::write(snapshots_dir().join(format!("{}.snap", suite.name)), out)
}

fn write_summary_snapshot(
    reports: &[(&Suite, SuiteReport)],
    total: &Tally,
    by_syntax: &BySyntax,
) -> io::Result<()> {
    let mut out = String::new();
    let _ = writeln!(out, "# oxc-css-parser conformance — `cargo run -p conformance`");
    let _ = writeln!(out, "# success = clean parse; failed = recovered + hard_error + panic");
    let _ = writeln!(out);
    let _ = writeln!(out, "{}", header_row("suite"));
    for (suite, report) in reports {
        let _ = writeln!(out, "{}", row(suite.name, &report.tally));
    }
    let _ = writeln!(out, "{}", "-".repeat(72));
    let _ = writeln!(out, "{}", row("total", total));
    let _ = writeln!(out, "\n# by syntax");
    let _ = writeln!(out, "{}", header_row("syntax"));
    let _ = writeln!(out, "{}", row("css", &by_syntax.css));
    let _ = writeln!(out, "{}", row("scss", &by_syntax.scss));
    let _ = writeln!(out, "{}", row("sass", &by_syntax.sass));
    let _ = writeln!(out, "{}", row("less", &by_syntax.less));
    fs::write(snapshots_dir().join("summary.snap"), out)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let clean = args.iter().any(|a| a == "--clean");
    let clone_only = args.iter().any(|a| a == "--clone");
    let filters: Vec<&str> =
        args.iter().filter(|a| !a.starts_with('-')).map(String::as_str).collect();

    if clean {
        let dir = repos_dir();
        match fs::remove_dir_all(&dir) {
            Ok(()) => println!("removed {}", dir.display()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => println!("nothing to remove"),
            Err(e) => eprintln!("failed to remove {}: {e}", dir.display()),
        }
        return;
    }

    let selected: Vec<&Suite> =
        SUITES.iter().filter(|s| filters.is_empty() || filters.contains(&s.name)).collect();
    if selected.is_empty() {
        let names = SUITES.iter().map(|s| s.name).collect::<Vec<_>>().join(", ");
        eprintln!("no matching suite; available: {names}");
        return;
    }

    // Silence per-file panic output; `catch_unwind` records it instead.
    panic::set_hook(Box::new(|_| {}));

    println!("cloning into {}", repos_dir().display());
    let mut clone_failed = false;
    for suite in &selected {
        print!("  {:<22} {}  ", suite.name, &suite.sha[..12]);
        io::stdout().flush().ok();
        match ensure_repo(suite) {
            Ok(true) => println!("fetched"),
            Ok(false) => println!("up-to-date"),
            Err(e) => {
                println!("ERROR: {e}");
                clone_failed = true;
            }
        }
    }
    // Fail loudly rather than produce partial snapshots from a half-cloned corpus;
    // clones flake on network hiccups, so callers (and CI) should retry.
    if clone_failed {
        eprintln!("\none or more clones failed (network?); re-run to retry.");
        std::process::exit(1);
    }

    if clone_only {
        return;
    }

    // Parsing is recursive, so run it on a thread with a large stack: some
    // deeply-nested inputs (e.g. sass-spec) overflow the default 8 MiB main
    // stack, and a stack overflow aborts the process — it is not a catchable
    // panic. 1 GiB is reserved virtual address space, committed lazily.
    let full_run = filters.is_empty();
    let worker = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || run_and_snapshot(&selected, full_run))
        .expect("failed to spawn worker thread");
    worker.join().expect("worker thread panicked");
}

/// Parse every selected suite and write the snapshot files.
fn run_and_snapshot(selected: &[&Suite], full_run: bool) {
    let mut total = Tally::default();
    let mut total_by_syntax = BySyntax::default();
    let mut reports: Vec<(&Suite, SuiteReport)> = Vec::new();
    for suite in selected {
        let report = run_suite(suite);
        total.add(&report.tally);
        total_by_syntax.add(&report.by_syntax);
        reports.push((suite, report));
    }

    // Report to stdout.
    println!("\n{}", header_row("suite"));
    for (suite, report) in &reports {
        println!("{}", row(suite.name, &report.tally));
    }
    println!("{}", "-".repeat(72));
    println!("{}", row("total", &total));
    println!("\nnotes:");
    for (suite, _) in &reports {
        println!("  {:<22} {}", suite.name, suite.note);
    }

    // Write snapshots.
    if let Err(e) = fs::create_dir_all(snapshots_dir()) {
        eprintln!("failed to create {}: {e}", snapshots_dir().display());
        return;
    }
    for (suite, report) in &reports {
        // Suites whose CSS is embedded in HTML/.bs/JSON ship no plain files; skip
        // writing an empty failures snapshot for them (they still appear in the summary).
        if report.tally.files == 0 {
            continue;
        }
        if let Err(e) = write_suite_snapshot(suite, report) {
            eprintln!("failed to write {}.snap: {e}", suite.name);
        }
    }

    // The summary aggregates every suite, so only rewrite it on a full run.
    if full_run {
        if let Err(e) = write_summary_snapshot(&reports, &total, &total_by_syntax) {
            eprintln!("failed to write summary.snap: {e}");
        }
    } else {
        println!("\n(partial run — summary.snap left unchanged)");
    }

    println!("\nsnapshots written to {}", snapshots_dir().display());
}

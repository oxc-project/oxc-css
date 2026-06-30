//! Conformance runner for `oxc-css-parser`.
//!
//! Clones a set of upstream CSS / preprocessor test corpora, each pinned to a
//! fixed commit SHA, then runs every CSS-family file through the parser and
//! reports per-suite outcomes (clean / recovered / hard-error / panic).
//!
//! Pinned SHAs keep runs reproducible; bump them deliberately to ingest
//! upstream changes. Cloned repos live under `tasks/conformance/repos/` (git
//! ignored) and are fetched shallow + blobless + sparse to stay small.
//!
//! Tracking issue: <https://github.com/oxc-project/oxc-css-parser/issues/19>.
//!
//! ```text
//! cargo run -p conformance                 # clone (if needed) + parse all suites
//! cargo run -p conformance -- sass-spec     # only the named suite(s)
//! cargo run -p conformance -- --clone       # clone/update only, do not parse
//! cargo run -p conformance -- --clean       # remove all cloned repos
//! ```

use std::{
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
        note: "canonical Sass/SCSS suite (tests compilation; we parse only)",
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

#[derive(Clone, Copy)]
enum Outcome {
    Clean,
    Recovered,
    HardError,
    Panic,
}

fn parse_outcome(source: &str, syntax: Syntax) -> Outcome {
    let caught = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        let allocator = Allocator::default();
        let mut parser = Parser::new(&allocator, source, syntax);
        match parser.parse::<Stylesheet>() {
            Ok(_) if parser.recoverable_errors().is_empty() => Outcome::Clean,
            Ok(_) => Outcome::Recovered,
            Err(_) => Outcome::HardError,
        }
    }));
    caught.unwrap_or(Outcome::Panic)
}

fn syntax_for(path: &Path) -> Option<Syntax> {
    match path.extension()?.to_str()? {
        "css" => Some(Syntax::Css),
        "scss" => Some(Syntax::Scss),
        "sass" => Some(Syntax::Sass),
        "less" => Some(Syntax::Less),
        _ => None,
    }
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
        } else if syntax_for(&path).is_some() {
            out.push(path);
        }
    }
}

#[derive(Default)]
struct Tally {
    files: u32,
    clean: u32,
    recovered: u32,
    hard_error: u32,
    panic: u32,
}

impl Tally {
    fn record(&mut self, outcome: Outcome) {
        self.files += 1;
        match outcome {
            Outcome::Clean => self.clean += 1,
            Outcome::Recovered => self.recovered += 1,
            Outcome::HardError => self.hard_error += 1,
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
}

fn run_suite(suite: &Suite) -> (Tally, Vec<PathBuf>) {
    let root = repos_dir().join(suite.name).join(suite.walk);
    let mut files = Vec::new();
    collect_files(&root, &mut files);
    files.sort();

    let mut tally = Tally::default();
    let mut panics = Vec::new();
    for path in files {
        let Ok(source) = fs::read_to_string(&path) else { continue };
        let syntax = syntax_for(&path).unwrap_or(Syntax::Css);
        let outcome = parse_outcome(&source, syntax);
        tally.record(outcome);
        if matches!(outcome, Outcome::Panic) {
            panics.push(path);
        }
    }
    (tally, panics)
}

fn print_row(label: &str, t: &Tally) {
    println!(
        "{:<22} {:>7} {:>7} {:>10} {:>9} {:>6}",
        label, t.files, t.clean, t.recovered, t.hard_error, t.panic
    );
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

    // Silence per-file panic output; `catch_unwind` records the count instead.
    panic::set_hook(Box::new(|_| {}));

    println!("cloning into {}", repos_dir().display());
    for suite in &selected {
        print!("  {:<22} {}  ", suite.name, &suite.sha[..12]);
        io::stdout().flush().ok();
        match ensure_repo(suite) {
            Ok(true) => println!("fetched"),
            Ok(false) => println!("up-to-date"),
            Err(e) => println!("ERROR: {e}"),
        }
    }

    if clone_only {
        return;
    }

    println!();
    println!(
        "{:<22} {:>7} {:>7} {:>10} {:>9} {:>6}",
        "suite", "files", "clean", "recov", "harderr", "panic"
    );

    let mut total = Tally::default();
    let mut all_panics: Vec<PathBuf> = Vec::new();
    for suite in &selected {
        let (tally, mut panics) = run_suite(suite);
        print_row(suite.name, &tally);
        total.add(&tally);
        all_panics.append(&mut panics);
    }
    print_row("total", &total);

    println!("\nnotes:");
    for suite in &selected {
        println!("  {:<22} {}", suite.name, suite.note);
    }

    if all_panics.is_empty() {
        println!("\nno panics — robustness invariant holds.");
    } else {
        println!("\n{} panic(s):", all_panics.len());
        for path in &all_panics {
            println!("  {}", path.display());
        }
        std::process::exit(1);
    }
}

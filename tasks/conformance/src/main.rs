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
//! hard_error + panic` on files that are expected to parse. Files that the
//! reference compiler itself rejects — sass-spec HRX tests with a sibling
//! `error` file, and less.js `tests-error/parse/**` — are *expected* to fail:
//! a parse error there is correct behavior, so they are counted in a separate
//! `expected` column and listed in their own snapshot section. Regenerate the
//! snapshots by re-running, and review changes via `git diff` — that is how
//! regressions/improvements surface.
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

use oxc_css_parser::{Allocator, ParserBuilder, ParserOptions, Syntax, ast::Stylesheet};

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
    /// Path prefixes (relative to the repo root) whose files may legitimately
    /// fail to parse — the reference compiler rejects them too, or they are
    /// encoding-test fixtures with no single valid text decoding.
    expect_error_under: &'static [&'static str],
    /// Enable [`ParserOptions::allow_postcss_simple_vars`] — for corpora
    /// written against the postcss-simple-vars plugin.
    postcss_simple_vars: bool,
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
        expect_error_under: &[],
        postcss_simple_vars: false,
        note: "CSS Syntax L3, JSON input->tree — needs a dedicated adapter",
    },
    Suite {
        name: "wpt",
        url: "https://github.com/web-platform-tests/wpt.git",
        sha: "1722fb6566acac7b0fc7bfc9aae55a47594b9d03",
        sparse: &["css/css-syntax"],
        walk: "css/css-syntax",
        // Mixed-encoding fixtures (e.g. a UTF-16 `@charset` prelude followed by
        // windows-1250 bytes) exist to drive encoding-detection HTML tests; no
        // single text decoding of them is valid CSS.
        expect_error_under: &["css/css-syntax/charset/support"],
        postcss_simple_vars: false,
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
        // examples the spec itself presents as invalid (`h2..foo` — "entirely
        // invalid and the entire style rule is dropped"); line-keyed, so
        // revisit when bumping the pinned SHA
        expect_error_under: &["selectors-4/Overview.bs::L1439", "selectors-4/Overview.bs::L1447"],
        postcss_simple_vars: false,
        note: "spec examples extracted from Bikeshed (.bs) sources",
    },
    Suite {
        name: "webref",
        url: "https://github.com/w3c/webref.git",
        sha: "9cce6ee56b9b281df9a81baa4cfc4a931e103333",
        sparse: &["ed/css"],
        walk: "ed/css",
        expect_error_under: &[],
        postcss_simple_vars: false,
        note: "Phase 4 — spec-surface coverage data (JSON), not parsed as CSS",
    },
    Suite {
        name: "postcss-parser-tests",
        url: "https://github.com/postcss/postcss-parser-tests.git",
        sha: "de1bc546de3678dd1c85e57cb2e9eece0098ddb9",
        sparse: &[],
        walk: "cases",
        expect_error_under: &[],
        postcss_simple_vars: true,
        note: "real-world CSS edge cases",
    },
    Suite {
        name: "sass-spec",
        url: "https://github.com/sass/sass-spec.git",
        sha: "a2ead9225786d49e91f5cc36755b7713596a2338",
        sparse: &["spec"],
        walk: "spec",
        expect_error_under: &[],
        postcss_simple_vars: false,
        note: "canonical Sass/SCSS suite; tests packed in .hrx archives (unpacked)",
    },
    Suite {
        name: "less.js",
        url: "https://github.com/less/less.js.git",
        sha: "8ae2cc3bfa79f0718ad6fe5f263a1d6819fe9d5c",
        sparse: &["packages/test-data"],
        walk: "packages/test-data",
        expect_error_under: &[
            "packages/test-data/tests-error/parse",
            // under eval/, but less.js's own parser throws these
            // ("Guards are only currently allowed on a single selector.")
            "packages/test-data/tests-error/eval/multiple-guards-on-css-selectors",
        ],
        postcss_simple_vars: false,
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

fn parse_outcome(source: &str, syntax: Syntax, options: ParserOptions) -> Outcome {
    let caught = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        let allocator = Allocator::default();
        let mut parser =
            ParserBuilder::new(&allocator, source).syntax(syntax).options(options).build();
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

/// Decode raw file bytes to text. Real-world corpora include UTF-16 files
/// (wpt's charset tests, some with no BOM); detect those — by BOM, or by NUL
/// bytes near the start (no valid CSS text contains NUL) — and decode them,
/// falling back to UTF-8. Returns `None` for undecodable (binary) content.
fn decode_text(bytes: &[u8]) -> Option<String> {
    fn utf16(bytes: &[u8], le: bool) -> Option<String> {
        let units = bytes.chunks_exact(2).map(|c| {
            if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) }
        });
        char::decode_utf16(units).collect::<Result<String, _>>().ok()
    }
    match bytes {
        [0xFF, 0xFE, rest @ ..] => utf16(rest, true),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, false),
        _ => {
            let head = &bytes[..bytes.len().min(64)];
            if head.contains(&0) {
                // Guess byte order from where the NULs sit: ASCII code points
                // put the NUL in the high byte — first for BE, second for LE.
                let le = head.iter().step_by(2).filter(|&&b| b == 0).count()
                    < head.iter().skip(1).step_by(2).filter(|&&b| b == 0).count();
                utf16(bytes, le)
            } else {
                String::from_utf8(bytes.to_vec()).ok()
            }
        }
    }
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
        } else if syntax_for(&path).is_some() || is_hrx(&path) || is_bikeshed(&path) {
            out.push(path);
        }
    }
}

#[derive(Default, Clone)]
struct Tally {
    files: u32,
    clean: u32,
    /// Parse errors on expected-error files (see [`Suite::expect_error_under`]
    /// and HRX sibling `error` entries) — correct behavior, not failures.
    expected: u32,
    recovered: u32,
    hard_error: u32,
    panic: u32,
}

impl Tally {
    fn record(&mut self, outcome: &Outcome, expect_error: bool) {
        self.files += 1;
        match outcome {
            Outcome::Clean => self.clean += 1,
            // A graceful parse error on an expected-error file is correct
            // behavior; count it apart so `failed` only tracks files that
            // should parse. A panic is a crash — never expected.
            Outcome::Recovered(_) | Outcome::HardError(_) if expect_error => self.expected += 1,
            Outcome::Recovered(_) => self.recovered += 1,
            Outcome::HardError(_) => self.hard_error += 1,
            Outcome::Panic => self.panic += 1,
        }
    }

    fn add(&mut self, other: &Tally) {
        self.files += other.files;
        self.clean += other.clean;
        self.expected += other.expected;
        self.recovered += other.recovered;
        self.hard_error += other.hard_error;
        self.panic += other.panic;
    }

    /// A clean parse with zero errors.
    fn success(&self) -> u32 {
        self.clean
    }

    /// A non-clean parse of a file that is expected to parse.
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
    /// Parse errors on expected-error files, snapshotted in their own section:
    /// they should stay failing, so one flipping to clean shows up in review.
    expected_failures: Vec<Failure>,
}

/// Render a path relative to `base` using forward slashes (stable across
/// platforms, avoids `str::replace`).
fn rel_path(path: &Path, base: &Path) -> String {
    let rel = path.strip_prefix(base).unwrap_or(path);
    rel.components().filter_map(|c| c.as_os_str().to_str()).collect::<Vec<_>>().join("/")
}

/// Whether an HRX entry belongs to a test that expects a compile error: the
/// reference compiler rejects it, so a parse error is correct behavior. Error
/// tests carry a sibling `error` (or `error-<impl>`) entry in the same
/// directory. Tests whose `options.yml` lists dart-sass under `:todo:` are
/// known-unsupported by the reference compiler (fuzzer archives etc.), so
/// failures there are expected as well.
fn hrx_expects_error(entries: &[(String, String)], entry: &str) -> bool {
    let dir = entry.rsplit_once('/').map_or("", |(dir, _)| dir);
    entries.iter().any(|(name, content)| {
        let (entry_dir, base) = name.rsplit_once('/').map_or(("", name.as_str()), |(d, b)| (d, b));
        if entry_dir == dir && (base == "error" || base.starts_with("error-")) {
            return true;
        }
        // options.yml applies to its own directory and everything below it
        base == "options.yml"
            && (entry_dir == dir
                || entry_dir.is_empty()
                || dir.starts_with(&format!("{entry_dir}/")))
            && content.contains(":todo:")
            && content.contains("dart-sass")
    })
}

fn is_bikeshed(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "bs")
}

/// Parse one extracted spec example. Examples are frequently fragments — a
/// run of declarations, a selector, or a value — so a failed stylesheet parse
/// is retried in each of those contexts before it counts as a failure (the
/// same methodology as the issue #19 prototype).
fn process_example(
    rel_path: String,
    css: &str,
    options: ParserOptions,
    expect_error: bool,
    report: &mut SuiteReport,
) {
    let outcome = parse_outcome(css, Syntax::Css, options);
    let outcome = if matches!(outcome, Outcome::Clean) {
        outcome
    } else {
        let contexts =
            [format!("a {{\n{css}\n}}"), format!("{css} {{}}"), format!("a {{ p: {css} }}")];
        if contexts
            .iter()
            .any(|wrapped| matches!(parse_outcome(wrapped, Syntax::Css, options), Outcome::Clean))
        {
            Outcome::Clean
        } else {
            outcome
        }
    };
    report.tally.record(&outcome, expect_error);
    report.by_syntax.get_mut(Syntax::Css).record(&outcome, expect_error);
    let (failure, is_panic) = match outcome {
        Outcome::Clean => return,
        Outcome::Recovered(label) => (Failure { tag: "RECOVER", rel_path, label }, false),
        Outcome::HardError(label) => (Failure { tag: "ERROR", rel_path, label }, false),
        Outcome::Panic => (Failure { tag: "PANIC", rel_path, label: String::new() }, true),
    };
    if expect_error && !is_panic {
        report.expected_failures.push(failure);
    } else {
        report.failures.push(failure);
    }
}

/// Extract CSS example blocks from a Bikeshed spec source: the contents of
/// `<pre>`/`<xmp>` blocks that are marked as CSS examples (their own
/// class/highlight says so, or they sit inside a `<div class=example>`),
/// skipping IDL/grammar/propdef/metadata blocks and examples that are not
/// actually CSS (embedded HTML demos, `…` elisions).
fn extract_bikeshed_examples(source: &str) -> Vec<(usize, String)> {
    const EXCLUDE: &[&str] = &[
        "idl",
        "propdef",
        "descdef",
        "prod",
        "railroad",
        "metadata",
        "biblio",
        "anchors",
        "link-defaults",
        "argumentdef",
        "elementdef",
        "simpledef",
        "lang-html",
        "lang-js",
        "lang-markup",
        "highlight=html",
        "highlight=\"html\"",
        "highlight=js",
        "highlight=\"js\"",
        "highlight=xml",
    ];

    let mut out = Vec::new();
    let mut div_stack: Vec<bool> = Vec::new();
    let mut i = 0;
    while let Some(offset) = source[i..].find('<') {
        let at = i + offset;
        let rest = &source[at..];
        if let Some(tag) = tag_at(rest, "<div") {
            div_stack.push(tag.to_ascii_lowercase().contains("example"));
            i = at + tag.len();
        } else if rest.starts_with("</div") {
            div_stack.pop();
            i = at + 5;
        } else if let Some((attrs, content, after)) =
            block_at(rest, "pre").or_else(|| block_at(rest, "xmp"))
        {
            let attrs = attrs.to_ascii_lowercase();
            let marked_css = attrs.contains("example")
                || attrs.contains("lang-css")
                || attrs.contains("language-css")
                || attrs.contains("highlight=css")
                || attrs.contains("highlight=\"css\"")
                || attrs.contains("highlight='css'");
            let in_example = div_stack.iter().any(|&is_example| is_example);
            let excluded = EXCLUDE.iter().any(|marker| attrs.contains(marker));
            if (marked_css || in_example) && !excluded {
                let css = clean_example(content);
                if !css.is_empty() {
                    let line = 1 + source[..at].matches('\n').count();
                    out.push((line, css));
                }
            }
            i = at + after;
        } else {
            i = at + 1;
        }
    }
    out
}

/// The full `<name ...>` opening tag at the start of `rest`, if present.
fn tag_at<'s>(rest: &'s str, open: &str) -> Option<&'s str> {
    let tail = rest.strip_prefix(open)?;
    if !tail.starts_with([' ', '\t', '\n', '>']) {
        return None;
    }
    rest.find('>').map(|end| &rest[..=end])
}

/// A whole `<name ...>content</name>` block at the start of `rest`; returns
/// (opening-tag attrs, raw content, offset just past the closing tag).
fn block_at<'s>(rest: &'s str, name: &str) -> Option<(&'s str, &'s str, usize)> {
    let tail = rest.strip_prefix('<')?.strip_prefix(name)?;
    if !tail.starts_with([' ', '\t', '\n', '>']) {
        return None;
    }
    let open_end = rest.find('>')?;
    let close = format!("</{name}");
    let close_at = open_end + rest[open_end..].find(&close)?;
    let after = close_at + rest[close_at..].find('>').map_or(close.len(), |e| e + 1);
    Some((&rest[1 + name.len()..open_end], &rest[open_end + 1..close_at], after))
}

/// Unescape, dedent, and vet an extracted example; empty string means "skip".
fn clean_example(raw: &str) -> String {
    // Bikeshed sources use empty HTML comments to control whitespace
    // (`<pre><!--\n-->2n+0 ...`); drop those before anything else, keeping
    // real content comments (a CDO/CDC example is never whitespace-only).
    let text = strip_whitespace_html_comments(raw);
    let text = html_unescape(&text);
    // not CSS: embedded HTML demos, elision placeholders, pseudocode math
    if looks_like_markup(&text) || text.contains('…') {
        return String::new();
    }
    if text.lines().any(|l| l.trim() == "...") {
        return String::new();
    }
    if text.lines().any(|line| {
        let mut parts = line.trim().splitn(2, char::is_whitespace);
        let head = parts.next().unwrap_or("");
        !head.is_empty()
            && head.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && parts.next().is_some_and(|tail| tail.trim_start().starts_with("= "))
    }) {
        return String::new();
    }
    // `{ ... }` / `(...)` elision placeholders stand for omitted contents;
    // empty the block so the surrounding syntax still gets parsed.
    let text = text.replace("{ ... }", "{}").replace("{...}", "{}");
    dedent(&text)
}

fn strip_whitespace_html_comments(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(at) = rest.find("<!--") {
        match rest[at + 4..].find("-->") {
            Some(len) if rest[at + 4..at + 4 + len].trim().is_empty() => {
                out.push_str(&rest[..at]);
                rest = &rest[at + 4 + len + 3..];
            }
            _ => {
                out.push_str(&rest[..at + 4]);
                rest = &rest[at + 4..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn html_unescape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let mut matched = false;
        for (entity, ch) in [
            ("&lt;", '<'),
            ("&gt;", '>'),
            ("&quot;", '"'),
            ("&#39;", '\''),
            ("&#x27;", '\''),
            ("&apos;", '\''),
            ("&#xa;", '\n'),
            ("&#xA;", '\n'),
            ("&nbsp;", ' '),
            ("&amp;", '&'),
        ] {
            if rest.starts_with(entity) {
                out.push(ch);
                rest = &rest[entity.len()..];
                matched = true;
                break;
            }
        }
        if !matched {
            out.push('&');
            rest = &rest[1..];
        }
    }
    out.push_str(rest);
    out
}

/// Whether the text still contains HTML-tag-looking sequences (`</...`,
/// `<span ...>`). Range media queries (`(400px < width)`) don't trip this:
/// their `<` is never followed directly by a letter run. Value-grammar
/// references (`<color>`) do, which is desirable — grammar snippets are not
/// parseable CSS either.
fn looks_like_markup(text: &str) -> bool {
    let bytes = text.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b != b'<' {
            continue;
        }
        match bytes.get(i + 1) {
            Some(b'/') => return true,
            Some(c) if c.is_ascii_alphabetic() => {
                let mut j = i + 1;
                while bytes.get(j).is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'-') {
                    j += 1;
                }
                if matches!(bytes.get(j), Some(b'>' | b' ' | b'\t' | b'\n')) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

/// Strip the longest common leading whitespace and surrounding blank lines.
fn dedent(text: &str) -> String {
    let mut prefix: Option<&str> = None;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let lead = &line[..line.len() - line.trim_start().len()];
        prefix = Some(match prefix {
            None => lead,
            Some(prev) => {
                let common =
                    prev.as_bytes().iter().zip(lead.as_bytes()).take_while(|(a, b)| a == b).count();
                &prev[..common]
            }
        });
    }
    let prefix = prefix.unwrap_or("");
    let mut lines: Vec<&str> =
        text.lines().map(|l| l.strip_prefix(prefix).unwrap_or(l.trim_start())).collect();
    while lines.first().is_some_and(|l| l.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

fn run_suite(suite: &Suite) -> SuiteReport {
    let suite_root = repos_dir().join(suite.name);
    let mut files = Vec::new();
    collect_files(&suite_root.join(suite.walk), &mut files);

    let options = ParserOptions {
        allow_postcss_simple_vars: suite.postcss_simple_vars,
        ..ParserOptions::default()
    };

    let mut report = SuiteReport::default();
    for path in files {
        let Ok(bytes) = fs::read(&path) else { continue };
        let Some(source) = decode_text(&bytes) else { continue };
        let rel = rel_path(&path, &suite_root);
        if let Some(syntax) = syntax_for(&path) {
            let expect_error = suite.expect_error_under.iter().any(|p| rel.starts_with(p));
            process_unit(rel, &source, syntax, options, expect_error, &mut report);
        } else if is_bikeshed(&path) {
            for (line, css) in extract_bikeshed_examples(&source) {
                let unit = format!("{rel}::L{line}");
                let expect_error = suite.expect_error_under.iter().any(|p| unit.starts_with(p));
                process_example(unit, &css, options, expect_error, &mut report);
            }
        } else {
            // sass-spec packs each test in an `.hrx` archive; parse its entries.
            let entries = parse_hrx(&source);
            for (entry, entry_source) in &entries {
                if let Some(syntax) = syntax_for_entry(entry) {
                    let expect_error = hrx_expects_error(&entries, entry);
                    process_unit(
                        format!("{rel}::{entry}"),
                        entry_source,
                        syntax,
                        options,
                        expect_error,
                        &mut report,
                    );
                }
            }
        }
    }
    report.failures.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    report.expected_failures.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    report
}

/// Parse one CSS unit and fold its outcome into `report`.
fn process_unit(
    rel_path: String,
    source: &str,
    syntax: Syntax,
    options: ParserOptions,
    expect_error: bool,
    report: &mut SuiteReport,
) {
    let outcome = parse_outcome(source, syntax, options);
    report.tally.record(&outcome, expect_error);
    report.by_syntax.get_mut(syntax).record(&outcome, expect_error);
    let (failure, is_panic) = match outcome {
        Outcome::Clean => return,
        Outcome::Recovered(label) => (Failure { tag: "RECOVER", rel_path, label }, false),
        Outcome::HardError(label) => (Failure { tag: "ERROR", rel_path, label }, false),
        Outcome::Panic => (Failure { tag: "PANIC", rel_path, label: String::new() }, true),
    };
    if expect_error && !is_panic {
        report.expected_failures.push(failure);
    } else {
        report.failures.push(failure);
    }
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
        "{first:<22} {:>6} {:>8} {:>7} {:>9} {:>7} {:>6} {:>8} {:>6}",
        "files", "success", "failed", "expected", "clean", "recov", "harderr", "panic"
    )
}

fn row(label: &str, t: &Tally) -> String {
    format!(
        "{label:<22} {:>6} {:>8} {:>7} {:>9} {:>7} {:>6} {:>8} {:>6}",
        t.files,
        t.success(),
        t.failed(),
        t.expected,
        t.clean,
        t.recovered,
        t.hard_error,
        t.panic
    )
}

fn write_failure_lines(out: &mut String, failures: &[Failure]) {
    if failures.is_empty() {
        let _ = writeln!(out, "none");
    }
    for failure in failures {
        if failure.label.is_empty() {
            let _ = writeln!(out, "{:<8} {}", failure.tag, failure.rel_path);
        } else {
            let _ = writeln!(out, "{:<8} {}    {}", failure.tag, failure.rel_path, failure.label);
        }
    }
}

fn write_suite_snapshot(suite: &Suite, report: &SuiteReport) -> io::Result<()> {
    let t = &report.tally;
    let mut out = String::new();
    let _ = writeln!(out, "suite: {}", suite.name);
    let _ = writeln!(out, "sha: {}", suite.sha);
    let _ = writeln!(
        out,
        "files: {}   success: {}   failed: {}   expected: {}",
        t.files,
        t.success(),
        t.failed(),
        t.expected
    );
    let _ = writeln!(
        out,
        "clean: {}  recovered: {}  hard_error: {}  panic: {}",
        t.clean, t.recovered, t.hard_error, t.panic
    );
    let _ = writeln!(out, "\nfailures:");
    write_failure_lines(&mut out, &report.failures);
    // Expected-error files are meant to keep failing; snapshot them so one
    // flipping to clean (parser accepting what the reference rejects) shows
    // up as a diff in this section.
    if t.expected > 0 {
        let _ = writeln!(out, "\nexpected failures (error tests, correct behavior):");
        write_failure_lines(&mut out, &report.expected_failures);
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
    let _ = writeln!(out, "# success = clean parse; failed = recovered + hard_error + panic;");
    let _ =
        writeln!(out, "# expected = parse errors on error-tests (correct behavior, not failures)");
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

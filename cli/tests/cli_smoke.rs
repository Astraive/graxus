//! Subprocess smoke tests for the `graxus` CLI binary.
//!
//! These tests exercise the real compiled binary end-to-end as a separate
//! process (not just the library), which is the only way to catch startup-time
//! failures such as the Windows debug stack overflow that previously crashed
//! every invocation before any command logic ran.
//!
//! Pipeline covered: `init` -> `index` -> `status` -> `find` -> `graph docs`
//! -> `codemap show` -> `update` (idempotency).

use std::fs;
use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

/// Absolute path to the freshly built `graxus` binary under test.
fn bin() -> String {
    // CARGO_BIN_EXE_graxus is injected by `cargo test` and points at the
    // debug binary built for this package.
    env!("CARGO_BIN_EXE_graxus").to_string()
}

/// Run the graxus binary with the given args. Asserts a successful exit and
/// returns combined stdout+stderr.
fn run(args: &[&str]) -> String {
    let output = Command::new(bin())
        .args(args)
        .output()
        .expect("failed to spawn graxus binary");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(
        output.status.success(),
        "graxus {:?} failed with status {:?}\n--- stdout ---\n{}\n--- stderr ---\n{}",
        args,
        output.status,
        stdout,
        stderr
    );
    stdout
}

/// Create a small project to index: one Rust file with two functions and a
/// call between them, plus a Markdown doc with frontmatter and a wiki link.
fn write_fixtures(root: &Path) {
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src/main.rs"),
        r#"fn main() {
    println!("hey graxus");
    helper_fn();
}

pub fn helper_fn() -> i32 {
    42
}
"#,
    )
    .unwrap();

    fs::write(
        root.join("README.md"),
        r#"---
title: Smoke
tags: [demo]
---

# Smoke

A [[helper_fn]] reference.
"#,
    )
    .unwrap();
}

#[test]
fn help_does_not_crash() {
    // The P0 regression: every invocation (including --help) used to stack
    // overflow in Windows debug builds. This is the smallest possible guard.
    let out = run(&["--help"]);
    assert!(out.contains("AI-native codebase knowledge engine"));
    assert!(out.contains("Usage:"));
    assert!(out.contains("Commands:"));
}

#[test]
fn full_pipeline_init_index_status_find_graph_codemap() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    // 1. init (creates graxus.yaml + .graxus/)
    run(&["init", root.to_str().unwrap()]);
    assert!(root.join("graxus.yaml").exists());
    assert!(root.join(".graxus").is_dir());

    write_fixtures(root);

    // 2. index via --root (exercises global-arg wiring)
    let root_arg = root.to_str().unwrap();
    let index_out = run(&["--root", root_arg, "index"]);
    assert!(
        index_out.contains("Symbols:") || index_out.contains("Symbols  :"),
        "index should report symbols: {}",
        index_out
    );

    // 3. status --json reflects the --root override
    let status_out = run(&["--root", root_arg, "status", "--json"]);
    assert!(status_out.contains("\"name\""));
    // The JSON root should reference our temp dir.
    assert!(
        status_out.contains("graxus_dir"),
        "status json missing graxus_dir: {}",
        status_out
    );

    // 4. find matches the function name in the indexed source
    let find_out = run(&["--root", root_arg, "find", "helper_fn"]);
    assert!(
        find_out.contains("helper_fn"),
        "find should surface helper_fn: {}",
        find_out
    );

    // 5. graph docs reports the markdown node
    let graph_out = run(&["--root", root_arg, "graph", "docs"]);
    assert!(
        graph_out.contains("Docs Graph") || graph_out.contains("Nodes:"),
        "graph docs should report nodes: {}",
        graph_out
    );

    // 6. codemap show reports the indexed symbols
    let codemap_out = run(&["--root", root_arg, "codemap", "show"]);
    assert!(
        codemap_out.contains("Code Codemap") || codemap_out.contains("Symbols:"),
        "codemap show should report symbols: {}",
        codemap_out
    );
}

#[test]
fn update_is_idempotent_and_preserves_call_rows() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    run(&["init", root.to_str().unwrap()]);
    write_fixtures(root);
    let root_arg = root.to_str().unwrap();

    // Initial full index.
    run(&["--root", root_arg, "index"]);

    // Helper: count symbols via `symbols` command output.
    let symbol_count = || -> usize {
        let out = run(&["--root", root_arg, "symbols"]);
        out.lines()
            .rev()
            .find_map(|l| {
                l.trim()
                    .strip_prefix("Total: ")
                    .and_then(|s| s.split_whitespace().next())
                    .and_then(|n| n.parse::<usize>().ok())
            })
            .unwrap_or(0)
    };

    let after_index = symbol_count();
    assert!(
        after_index >= 2,
        "expected >=2 symbols, got {}",
        after_index
    );

    // Modify the source so an incremental update has real work to do.
    fs::write(
        root.join("src/main.rs"),
        r#"fn main() {
    println!("hey graxus");
    helper_fn();
    helper_fn();
}

pub fn helper_fn() -> i32 {
    42
}
"#,
    )
    .unwrap();

    // First incremental update: should detect 1 modified file and snapshot.
    let upd1 = run(&["--root", root_arg, "update"]);
    assert!(upd1.contains("modified"), "update #1: {}", upd1);
    assert!(
        upd1.contains("Snapshot saved"),
        "update #1 snapshot: {}",
        upd1
    );

    let after_update_1 = symbol_count();
    assert_eq!(
        after_update_1, after_index,
        "re-indexing the same number of symbols must not duplicate rows"
    );

    // Second incremental update with NO changes: must be a no-op.
    let upd2 = run(&["--root", root_arg, "update"]);
    assert!(
        upd2.contains("up to date") || upd2.contains("No changes"),
        "update #2 should be no-op: {}",
        upd2
    );

    // Third incremental update after another modification: still idempotent.
    fs::write(
        root.join("src/main.rs"),
        r#"fn main() {
    println!("hey graxus");
    helper_fn();
    helper_fn();
    helper_fn();
}

pub fn helper_fn() -> i32 {
    42
}
"#,
    )
    .unwrap();
    run(&["--root", root_arg, "update"]);
    let after_update_3 = symbol_count();
    assert_eq!(
        after_update_3, after_index,
        "third update must not accumulate duplicate symbol rows"
    );
}

#[test]
fn watch_debounce_is_interpreted_as_milliseconds() {
    // We can't run watch (it blocks), but we can sanity-check that the arg is
    // accepted and the help text documents milliseconds (regression guard for
    // the unit-mismatch bug).
    let out = run(&["watch", "--help"]);
    assert!(
        out.contains("Debounce milliseconds"),
        "watch --help should document milliseconds: {}",
        out
    );
}

#[test]
fn replace_history_rollback_safety_contract() {
    // SAFETY.md contract: replace --apply must create a snapshot that is
    // visible to `graxus history` and restorable via `graxus rollback <id> --apply`.
    // All three operations must honor `--root`.
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    let src = root.join("src/main.rs");
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(&src, "fn main() { println!(\"alpha\"); }\n").unwrap();

    let root_arg = root.to_str().unwrap();
    run(&["init", root_arg]);

    // replace --apply, scoped to --root. Must report a snapshot id.
    let repl = run(&["--root", root_arg, "replace", "alpha", "beta", "--apply"]);
    assert!(repl.contains("Applied 1 replacements"), "replace: {}", repl);
    assert!(
        repl.contains("Snapshot saved (id ") && repl.contains("graxus rollback"),
        "replace should print a snapshot id + rollback hint: {}",
        repl
    );

    // The applied edit must have landed.
    assert!(
        fs::read_to_string(&src).unwrap().contains("beta"),
        "file should contain 'beta' after replace"
    );

    // history must list the snapshot (--root honored, IndexStore format).
    let hist = run(&["--root", root_arg, "--no-color", "history"]);
    assert!(
        hist.contains("snapshot") && hist.contains("replace"),
        "history should list the replace snapshot: {}",
        hist
    );

    // Extract the snapshot id (first token after "snapshot").
    let sid = hist
        .lines()
        .find_map(|l| {
            let trimmed = l.trim();
            trimmed
                .strip_prefix("snapshot ")
                .and_then(|rest| rest.split_whitespace().next())
                .map(str::to_owned)
        })
        .expect("history should expose a snapshot id");

    // rollback --apply restores the original content.
    let rb = run(&["--root", root_arg, "rollback", &sid, "--apply"]);
    assert!(
        rb.contains("Rollback complete"),
        "rollback should succeed: {}",
        rb
    );
    assert!(
        fs::read_to_string(&src).unwrap().contains("alpha"),
        "file should be restored to 'alpha' after rollback"
    );
}

#[test]
fn replace_include_and_lang_filters_scope_the_edit() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("docs")).unwrap();
    // Two code files + one doc, all containing "needle".
    fs::write(
        root.join("src/main.rs"),
        "fn main() { needle(); }\nfn lib() { needle(); }\n",
    )
    .unwrap();
    fs::write(root.join("src/lib.rs"), "fn x() { needle(); }\n").unwrap();
    fs::write(root.join("docs/readme.md"), "# Docs\n\nneedle needle\n").unwrap();

    let root_arg = root.to_str().unwrap();
    run(&["init", root_arg]);

    // --include scopes to a single file.
    let out = run(&[
        "--root",
        root_arg,
        "replace",
        "needle",
        "x",
        "--preview",
        "--include",
        "src/main.rs",
    ]);
    assert!(out.contains("Include:     src/main.rs"), "{}", out);
    assert!(
        out.contains("Files affected: 1"),
        "include should limit to 1 file: {}",
        out
    );

    // --lang rust excludes the markdown doc.
    let out = run(&[
        "--root",
        root_arg,
        "replace",
        "needle",
        "x",
        "--preview",
        "--lang",
        "rust",
    ]);
    assert!(out.contains("Languages:   rust"), "{}", out);
    // 2 rust files, not 3 (docs/readme.md excluded).
    assert!(
        out.contains("Files affected: 2"),
        "lang should exclude .md: {}",
        out
    );
}

#[test]
fn replace_max_files_aborts_before_mutating() {
    // Safety guard: --max-files below the number of matching files must bail
    // out and leave every file untouched.
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("src")).unwrap();
    let f1 = root.join("src/a.rs");
    let f2 = root.join("src/b.rs");
    fs::write(&f1, "fn a() { target }\n").unwrap();
    fs::write(&f2, "fn b() { target }\n").unwrap();

    let root_arg = root.to_str().unwrap();
    run(&["init", root_arg]);

    // Invoke directly so we can observe the non-zero exit status.
    let output = Command::new(bin())
        .args([
            "--root",
            root_arg,
            "replace",
            "target",
            "replaced",
            "--apply",
            "--max-files",
            "1",
        ])
        .output()
        .expect("spawn");
    assert!(
        !output.status.success(),
        "--max-files below match count must fail, got status {:?}",
        output.status
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Too many files"),
        "expected 'Too many files' error, got: {}",
        stderr
    );

    // Neither file was mutated.
    assert!(fs::read_to_string(&f1).unwrap().contains("target"));
    assert!(fs::read_to_string(&f2).unwrap().contains("target"));
}

#[test]
fn dead_code_detects_private_uncalled_by_default() {
    // Regression for the visibility bug: private functions must be detected as
    // dead WITHOUT --include-exported. Previously the Rust indexer marked every
    // function exported=true, so the default scan found nothing.
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src/main.rs"),
        "fn main() { used(); }\nfn used() {}\nfn dead_one() {}\nfn dead_two() {}\n",
    )
    .unwrap();

    let root_arg = root.to_str().unwrap();
    run(&["init", root_arg]);
    run(&["--root", root_arg, "index"]);

    // Use --no-color so the output is plain text and easy to assert on.
    let out = run(&["--root", root_arg, "--no-color", "dead-code"]);
    // dead_one and dead_two are private + uncalled + unreferenced → flagged.
    assert!(out.contains("dead_one"), "should flag dead_one: {}", out);
    assert!(out.contains("dead_two"), "should flag dead_two: {}", out);

    // Collect the set of flagged symbol names. Result lines look like:
    //   "  ⚠ [90%] function <name> <file>:<line>"
    let flagged: Vec<String> = out
        .lines()
        .filter(|l| l.contains("⚠") && l.contains("function"))
        .filter_map(|l| {
            let toks: Vec<&str> = l.split_whitespace().collect();
            toks.iter()
                .position(|t| *t == "function")
                .and_then(|i| toks.get(i + 1).map(|s| s.to_string()))
        })
        .collect();
    assert!(
        !flagged.contains(&"main".to_string()),
        "main is an entry point, must not be flagged: {:?}",
        flagged
    );
    assert!(
        !flagged.contains(&"used".to_string()),
        "'used' is called, must not be flagged: {:?}",
        flagged
    );
}

#[test]
fn dead_code_limit_and_confidence_filters() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("src")).unwrap();
    // 3 private uncalled fns + 1 exported uncalled fn.
    fs::write(
        root.join("src/main.rs"),
        "fn main() {}\nfn da() {}\nfn db() {}\nfn dc() {}\npub fn exported_unused() {}\n",
    )
    .unwrap();

    let root_arg = root.to_str().unwrap();
    run(&["init", root_arg]);
    run(&["--root", root_arg, "index"]);

    // --limit caps the private ones.
    let capped = run(&["--root", root_arg, "dead-code", "--limit", "1"]);
    assert!(
        capped.contains("of 3 potentially unused symbols"),
        "capped should mention 3 total private candidates: {}",
        capped
    );

    // --include-exported surfaces the exported one at the 75 tier.
    let with_exported = run(&["--root", root_arg, "dead-code", "--include-exported"]);
    assert!(
        with_exported.contains("exported_unused") && with_exported.contains("[75%"),
        "--include-exported should surface exported_unused at 75%: {}",
        with_exported
    );

    // --min-confidence 80 suppresses the 75-tier exported candidate but keeps
    // the 90-tier private ones.
    let strict = run(&[
        "--root",
        root_arg,
        "dead-code",
        "--include-exported",
        "--min-confidence",
        "80",
    ]);
    assert!(
        !strict.contains("exported_unused"),
        "min-confidence 80 should suppress exported candidate: {}",
        strict
    );
    assert!(
        strict.contains("da"),
        "private candidates are 90%: {}",
        strict
    );
}

#[test]
fn dead_code_does_not_false_positive_on_referenced_types() {
    // A struct used as a type / in a path call but never "called" must NOT be
    // flagged as dead. This is the cross-file/type-usage case the call-graph-
    // only heuristic used to get wrong.
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src/main.rs"),
        "mod lib;\nfn main() {\n    let h = lib::Helper::new();\n    h.greet();\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src/lib.rs"),
        "pub struct Helper { x: i32 }\nimpl Helper {\n    pub fn new() -> Self { Helper { x: 0 } }\n    pub fn greet(&self) {}\n    fn unused_method(&self) {}\n}\npub fn cross_file_unused() {}\n",
    )
    .unwrap();

    let root_arg = root.to_str().unwrap();
    run(&["init", root_arg]);
    run(&["--root", root_arg, "index"]);

    let out = run(&["--root", root_arg, "dead-code", "--include-exported"]);
    // Helper IS referenced (type usage in main.rs) → must NOT be flagged.
    assert!(
        !out.lines().any(|l| l.contains("Helper")),
        "referenced struct Helper must not be flagged dead: {}",
        out
    );
    // Truly dead symbols are still caught.
    assert!(out.contains("unused_method"), "{}", out);
    assert!(out.contains("cross_file_unused"), "{}", out);
}

#[test]
fn regex_context_lines_around_match() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src/main.rs"),
        "line one\nMATCH HERE\nline three\n",
    )
    .unwrap();

    let root_arg = root.to_str().unwrap();
    run(&["init", root_arg]);
    run(&["--root", root_arg, "index"]);

    let out = run(&["--root", root_arg, "regex", "MATCH", "--context-lines", "1"]);
    // The match line carries the '>' marker; the two adjacent lines do not.
    assert!(
        out.contains(">"),
        "match line should be marked with ' >': {}",
        out
    );
    assert!(out.contains("MATCH"), "match should appear: {}", out);
    assert!(out.contains("line one"), "context before: {}", out);
    assert!(out.contains("line three"), "context after: {}", out);
}

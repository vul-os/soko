//! site/docs/ must be a byte-exact mirror of docs/.
//!
//! `site/docs.html` is a client-side renderer: it fetches `./docs/<page>.md` at runtime and renders
//! the markdown in the browser. Relative to `site/`, that resolves to `site/docs/` — so that tree is
//! not a build artefact, it is the copy the published site serves. `docs/` is the copy contributors
//! edit. They were 25 byte-identical files with nothing holding them together, which meant editing
//! `docs/` and forgetting `site/docs/` published stale text with no signal anywhere: no build step
//! touched it, no test compared them, and the duplication was invisible precisely *because* the two
//! copies agreed.
//!
//! `tools/sync-docs.mjs` performs the copy. This test is the gate, and it lives in Rust on purpose:
//! it runs under the `cargo test --workspace` that CI already gates on, so it needs no Node
//! toolchain in the job and there is no `if` for it to be quietly wrapped in.
//!
//! It fails closed. A missing tree, an empty source tree, or an unreadable file is a failure, never
//! a skip — the notes in `tests/conformance.rs` record what a silent skip in this repository already
//! cost once, and `cargo test` discards captured output for tests that pass, so a "loud skip" is
//! invisible in exactly the run where it matters.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every file under `dir`, keyed by its path relative to `dir`, with its bytes.
///
/// Panics rather than returning an error: both trees are committed to this repository, so anything
/// unreadable here means the checkout is damaged, which is a failure and not an environment quirk.
fn tree(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = std::fs::read_dir(&current)
            .unwrap_or_else(|e| panic!("failed to read directory {}: {e}", current.display()));
        for entry in entries {
            let entry = entry.unwrap_or_else(|e| {
                panic!("failed to read an entry of {}: {e}", current.display())
            });
            let path = entry.path();
            let kind = entry
                .file_type()
                .unwrap_or_else(|e| panic!("failed to stat {}: {e}", path.display()));
            if kind.is_dir() {
                stack.push(path);
            } else if kind.is_file() {
                let rel = path
                    .strip_prefix(dir)
                    .expect("walked path is under the directory it came from")
                    .to_string_lossy()
                    .replace('\\', "/");
                let bytes = std::fs::read(&path)
                    .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
                out.insert(rel, bytes);
            }
        }
    }
    out
}

#[test]
fn site_docs_is_a_byte_exact_mirror_of_docs() {
    let root = repo_root();
    let src_dir = root.join("docs");
    let dst_dir = root.join("site/docs");

    // Fail closed on a broken checkout. In particular, an *empty* source tree must not read as
    // "in sync": a mirror check that passes because there was nothing to mirror reports success for
    // a repository that is actually damaged.
    assert!(
        src_dir.is_dir(),
        "{} is missing — it is the source of the site's docs mirror",
        src_dir.display()
    );
    assert!(
        dst_dir.is_dir(),
        "{} is missing — site/docs.html fetches these files at runtime, so the published site \
         would 404 on every page. Run `node tools/sync-docs.mjs`.",
        dst_dir.display()
    );

    let src = tree(&src_dir);
    let dst = tree(&dst_dir);
    assert!(
        !src.is_empty(),
        "{} holds no files; refusing to report the mirror as in sync when there is nothing in it",
        src_dir.display()
    );

    let missing: Vec<&String> = src.keys().filter(|k| !dst.contains_key(*k)).collect();
    let extra: Vec<&String> = dst.keys().filter(|k| !src.contains_key(*k)).collect();
    let differing: Vec<&String> = src
        .iter()
        .filter(|(k, v)| dst.get(*k).is_some_and(|d| d != *v))
        .map(|(k, _)| k)
        .collect();

    if missing.is_empty() && extra.is_empty() && differing.is_empty() {
        return;
    }

    let mut report = String::from(
        "site/docs/ has drifted from docs/.\n\n\
         site/docs.html fetches these files at runtime, so this is text the published site is \
         serving — not a stale build artefact.\n\n",
    );
    for f in &differing {
        report.push_str(&format!("  differs      docs/{f}  ->  site/docs/{f}\n"));
    }
    for f in &missing {
        report.push_str(&format!("  not copied   docs/{f}\n"));
    }
    for f in &extra {
        report.push_str(&format!("  orphaned     site/docs/{f}  (no docs/{f})\n"));
    }
    report.push_str("\nFix: run `node tools/sync-docs.mjs` and commit the result.\n");
    panic!("{report}");
}

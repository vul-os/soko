//! Cross-checks Soko's own computations against the TRACT conformance vectors — the corpus that
//! `conformance/vectors/` in the spec repository derives by hand, directly from the specification
//! text, without ever consulting this workspace (see `conformance/README.md` and
//! `conformance/vectors/README.md` there for why that separation is the whole point: a vector
//! exported from Soko could only ever prove Soko self-consistent, never that Soko agrees with the
//! spec).
//!
//! Before this file existed, Soko never read those vectors at all — the loop the discipline
//! describes was open on Soko's side: the vectors proved the spec had been read correctly, and
//! nothing proved Soko agreed with it. This file closes that loop for the slice of the corpus Soko
//! can actually check today: the four `"kind": "derived-value"` vectors, whose formulas map onto a
//! real Soko function.
//!
//! ## The corpus is vendored, and why that changed
//!
//! This file used to read the vectors out of a **sibling `../tract` checkout** and, when that
//! checkout was absent, print a skip notice and return. That arrangement failed in the quietest
//! possible way. `cargo test` captures a passing test's `println!` output and discards it, so on
//! every machine and every CI runner without the sibling checkout — which was all of them, the
//! spec having since moved into the KOTVA repository as a profile — this test printed `ok` and
//! verified nothing. It stayed that way long enough for the harness's own "no Soko function
//! computes this" notes to go stale: `Zone::select_bracket`, `RateCard::quote` and
//! `EscrowScope::intersect` were all written in the meantime, and nothing noticed that three
//! uncovered rows had become checkable, because the file that would have said so never ran.
//!
//! So the corpus is now **vendored** into `tests/tract-vectors/vectors/`, byte-for-byte, with a
//! digest over every file in `tests/tract-vectors/PROVENANCE.json`. Three consequences, all
//! deliberate:
//!
//! - the conformance check **always runs** — a bare `cargo test` in a bare `soko` checkout, with
//!   no network and no sibling repository, executes every check below. There is nothing left to
//!   silently skip;
//! - the vendored copies cannot be edited to make Soko agree with them, because
//!   [`vendored_corpus_matches_its_provenance`] recomputes the digest and fails;
//! - the copies can go **stale** against upstream, which is the new risk vendoring introduces.
//!   [`vendored_corpus_matches_the_upstream_spec_checkout`] is the guard for it: where a spec
//!   checkout is present it compares byte-for-byte and fails on any drift, and where one is not,
//!   it says so on the process's real stderr — the one channel `cargo test` cannot swallow.
//!
//! Vendoring the vectors is not the same act as embedding the specification. The vectors are
//! *data* the spec repository publishes under CC BY 4.0 for exactly this purpose, and the
//! discipline that matters — that a vector is derived from the prose by a human, never exported
//! from Soko — is a property of how they were *written*, which copying preserves and the digest
//! pins.
//!
//! ## What this deliberately does not do
//!
//! - It does not check the fifteen `"kind": "cbor-encoding"` vectors. Those pin exact §16 wire
//!   bytes (canonical CBOR with integer CDDL keys), and nothing in this workspace encodes with
//!   integer keys yet — `soko-core`'s round-trip tests (see `soko_core::roundtrip`) establish that
//!   the *current* derive-based encoding is internally *stable*, which is a different, narrower
//!   claim than "matches §16's CDDL byte-for-byte". Checking the CBOR vectors is future work for
//!   whenever Soko grows a real wire encoder.
//! - It does not check the two `"kind": "structural-check"` vectors (`TRACT-CAT-03`,
//!   `TRACT-PUBSEAL-03`) for the same reason: they assert decode-time rejection of a *specific
//!   encoded byte shape*, which needs the real encoder above.
//! - Where a derived-value vector's formula has no matching Soko function at all, this file does
//!   not invent one to "cover" it — it records the vector as **explicitly uncovered**, with the
//!   specific reason, in the summary this test prints. Silently skipping it would be worse than
//!   useless: a coverage gap that prints nothing looks identical to a coverage gap that was
//!   checked and passed.
//!
//! Every one of those exclusions is now *counted* rather than asserted in prose:
//! [`CORPUS`] lists all 21 vector files with their kind and case count, and
//! [`vendored_corpus_census_matches_the_files_on_disk`] fails if the corpus grows, shrinks, or if
//! any file gains a case this harness does not run. A corpus that grows can no longer under-run
//! this suite quietly.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::Value;

use soko_catalog::{Attribute, IdentityRung, ProductGroup, ProductRecord};
use soko_core::{roundtrip, ContentAddress, Country, Currency, IdentityKey, Money, Timestamp};
use soko_delivery::{
    Consignment, CustodyHandoff, DistributorCapacity, Leg, Parcel, RateCard, RateSource,
    RouteOption, RouteShape, WeightBracket, Zone,
};
use soko_gateway::{StoreBinding, StoreHost};
use soko_jurisdiction::{place_of_supply, Anchors, InRegionRepresentative, ResponsibleParties};
use soko_offer::{
    Availability, Consideration, Fulfilment, Item, Offer, PlaceRef, PriceTier, StockSignal,
};
use soko_order::{BoundedCounter, Cart, CartLine, Order, OrderState};
use soko_settle::{
    EscrowRuling, EscrowScope, EscrowState, PaymentAttestation, RailClass, RailSubstitution,
};
use soko_trust::{Attestor, PurchaseAttestation, Review, Subject};

// ==================================================================================================
// The vendored corpus: where it lives, what is in it, and how it is pinned
// ==================================================================================================

/// The vendored vector files. See `tests/tract-vectors/README.md`.
fn vendored_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/tract-vectors/vectors")
}

/// The digest record covering [`vendored_dir`].
fn provenance_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/tract-vectors/PROVENANCE.json")
}

/// Environment override pointing at an upstream `conformance/vectors` directory, for the drift
/// check. Set it when the spec checkout is somewhere other than the two default locations.
const UPSTREAM_ENV: &str = "SOKO_TRACT_VECTORS";

/// Set this to `1` to turn "no upstream spec checkout to compare against" from a loud notice into a
/// hard failure. Intended for a CI job that *does* check the spec out, where a missing checkout
/// means the job is misconfigured rather than that the machine is a plain `soko` clone.
const REQUIRE_UPSTREAM_ENV: &str = "SOKO_REQUIRE_TRACT_VECTORS";

/// What a vector file asserts, per its own `"kind"` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Pins exact §16 canonical-CBOR bytes. Not checkable until Soko has a real wire encoder.
    CborEncoding,
    /// Asserts a decode-time rejection of a specific encoded shape. Same blocker.
    StructuralCheck,
    /// Computes a value from a §-numbered formula. This is the checkable slice.
    DerivedValue,
}

/// Whether this harness checks a file, and if not, the specific reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    /// Every case in the file drives at least one check below.
    Checked,
    /// Not checked, for the stated reason. Never "not checked yet" without one.
    Uncovered(&'static str),
}

/// One vendored vector file, as this harness understands it.
struct VectorFile {
    file: &'static str,
    kind: Kind,
    /// Length of the file's `cases` array. Asserted against the file on disk, so a vector that
    /// grows a case fails here rather than being quietly half-run.
    cases: usize,
    status: Status,
}

/// The census of the vendored corpus. Every file, its kind, its case count, and whether this
/// harness checks it. A file on disk that is missing from this table — or a table row with no file
/// — fails [`vendored_corpus_census_matches_the_files_on_disk`].
const CORPUS: &[VectorFile] = &[
    VectorFile {
        file: "attribute.json",
        kind: Kind::CborEncoding,
        cases: 1,
        status: Status::Uncovered(NO_WIRE_ENCODER),
    },
    VectorFile {
        file: "billable-weight-and-local-rate-card-price-lookup-tract-deliv-01.json",
        kind: Kind::DerivedValue,
        cases: 3,
        status: Status::Checked,
    },
    VectorFile {
        file: "escrowscope-checkout-intersection.json",
        kind: Kind::DerivedValue,
        cases: 2,
        status: Status::Checked,
    },
    VectorFile {
        file: "escrowscope.json",
        kind: Kind::CborEncoding,
        cases: 1,
        status: Status::Uncovered(NO_WIRE_ENCODER),
    },
    VectorFile {
        file: "identityrung.json",
        kind: Kind::CborEncoding,
        cases: 3,
        status: Status::Uncovered(NO_WIRE_ENCODER),
    },
    VectorFile {
        file: "money.json",
        kind: Kind::CborEncoding,
        cases: 3,
        status: Status::Uncovered(NO_WIRE_ENCODER),
    },
    VectorFile {
        file: "offer-availability.json",
        kind: Kind::CborEncoding,
        cases: 8,
        status: Status::Uncovered(NO_WIRE_ENCODER),
    },
    VectorFile {
        file: "offer-consideration.json",
        kind: Kind::CborEncoding,
        cases: 6,
        status: Status::Uncovered(NO_WIRE_ENCODER),
    },
    VectorFile {
        file: "offer-fulfilment.json",
        kind: Kind::CborEncoding,
        cases: 8,
        status: Status::Uncovered(NO_WIRE_ENCODER),
    },
    VectorFile {
        file: "offer-item.json",
        kind: Kind::CborEncoding,
        cases: 5,
        status: Status::Uncovered(NO_WIRE_ENCODER),
    },
    VectorFile {
        file: "offer-missing-an-axis-is-structurally-rejected-tract-cat-03.json",
        kind: Kind::StructuralCheck,
        cases: 2,
        status: Status::Uncovered(NO_WIRE_DECODER),
    },
    VectorFile {
        file: "offer.json",
        kind: Kind::CborEncoding,
        cases: 1,
        status: Status::Uncovered(NO_WIRE_ENCODER),
    },
    VectorFile {
        file: "order.json",
        kind: Kind::CborEncoding,
        cases: 1,
        status: Status::Uncovered(NO_WIRE_ENCODER),
    },
    VectorFile {
        file: "paymentattestation.json",
        kind: Kind::CborEncoding,
        cases: 1,
        status: Status::Uncovered(NO_WIRE_ENCODER),
    },
    VectorFile {
        file:
            "place-of-supply-derivation-for-all-7-fulfilment-variants-tract-fulf-01-tract-fulf-02\
               .json",
        kind: Kind::DerivedValue,
        cases: 8,
        status: Status::Checked,
    },
    VectorFile {
        file: "placeref.json",
        kind: Kind::CborEncoding,
        cases: 2,
        status: Status::Uncovered(NO_WIRE_ENCODER),
    },
    VectorFile {
        file: "productrecord-canonical-byte-convergence-and-divergence-tract-cat-01-partial.json",
        kind: Kind::CborEncoding,
        cases: 2,
        status: Status::Uncovered(NO_WIRE_ENCODER),
    },
    VectorFile {
        file: "productrecord.json",
        kind: Kind::CborEncoding,
        cases: 2,
        status: Status::Uncovered(NO_WIRE_ENCODER),
    },
    VectorFile {
        file: "ratecard.json",
        kind: Kind::CborEncoding,
        cases: 1,
        // Read, but for its data rather than its bytes: TRACT-DELIV-01's cases price against
        // "zone 1's brackets from TRACT-WIRE-VEC-14's RateCard example", which is this file.
        status: Status::Uncovered(NO_WIRE_ENCODER),
    },
    VectorFile {
        file: "review.json",
        kind: Kind::StructuralCheck,
        cases: 3,
        status: Status::Uncovered(NO_WIRE_DECODER),
    },
    VectorFile {
        file: "route-totals.json",
        kind: Kind::DerivedValue,
        cases: 2,
        status: Status::Checked,
    },
];

const NO_WIRE_ENCODER: &str =
    "pins exact §16 canonical-CBOR bytes with integer CDDL keys; nothing in this workspace encodes \
     with integer keys yet";
const NO_WIRE_DECODER: &str =
    "asserts decode-time rejection of a specific encoded byte shape, which needs the §16 encoder \
     above";

/// How many checks a full run performs. Asserted exactly, so a check family that stops running —
/// because a helper returned early, or a `for` loop was fed an empty array by a reshaped vector —
/// fails instead of quietly reporting a smaller, still-green number.
const EXPECTED_CHECKS: usize = 21;

// ==================================================================================================
// Loud output
// ==================================================================================================

/// Write straight to the process's stderr handle.
///
/// `cargo test` captures the `print!`/`eprint!` macros and **discards** the capture for tests that
/// pass, which is precisely how this file's old skip notice stayed invisible for its whole life.
/// The `std::io::stderr()` handle is not part of that capture, so a message written through it
/// appears under a plain `cargo test`, with no `--nocapture`. Measured, not assumed — see
/// `tests/tract-vectors/README.md`.
fn loud(msg: &str) {
    let mut err = std::io::stderr();
    let _ = err.write_all(msg.as_bytes());
    let _ = err.flush();
}

// ==================================================================================================
// Vector loading and digests
// ==================================================================================================

/// Read and parse one vector file. Panics on failure: every file this reads is vendored into this
/// repository and pinned by [`provenance_path`], so a missing or malformed one means the checkout
/// is damaged, not that the environment is unusual.
fn load(path: &Path) -> Value {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read vector file {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| {
        panic!(
            "failed to parse vector file {} as JSON: {e}",
            path.display()
        )
    })
}

/// Every `*.json` in a directory, by file name, with its bytes. Sorted, because both the digest and
/// every comparison below depend on a stable order.
fn read_json_files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let entries =
        std::fs::read_dir(dir).unwrap_or_else(|e| panic!("failed to list {}: {e}", dir.display()));
    for entry in entries {
        let entry = entry.expect("failed to read a directory entry");
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".json") {
            continue;
        }
        let bytes = std::fs::read(entry.path())
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", entry.path().display()));
        out.insert(name, bytes);
    }
    out
}

/// BLAKE3 over the whole corpus: for each file in name order, the name, a `0x00` separator, then
/// the bytes. The separator is what stops a rename from being cancelled out by a matching edit.
///
/// BLAKE3 because it is already the workspace's hash (§16.2 takes it from the substrate) — pinning
/// the corpus must not drag a new dependency in behind it.
fn corpus_digest(files: &BTreeMap<String, Vec<u8>>) -> String {
    let mut hasher = blake3::Hasher::new();
    for (name, bytes) in files {
        hasher.update(name.as_bytes());
        hasher.update(&[0]);
        hasher.update(bytes);
    }
    hasher.finalize().to_hex().to_string()
}

fn file_digest(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

// ==================================================================================================
// Guard 1 — the vendored bytes are the bytes that were vendored
// ==================================================================================================

/// Fails if a vendored vector was edited, added, or removed without re-recording the provenance.
///
/// This is the guard that keeps vendoring honest. `conformance/README.md`'s rule — a disagreement
/// is resolved with the specification text as the tiebreaker, **never** by editing the vector to
/// match the implementation — is unenforceable against a local copy unless something notices the
/// local copy changing. This notices.
#[test]
fn vendored_corpus_matches_its_provenance() {
    let provenance = load(&provenance_path());
    let recorded = provenance["files"]
        .as_object()
        .expect("PROVENANCE.json needs a `files` object");
    let files = read_json_files(&vendored_dir());

    let mut problems = Vec::new();
    for (name, bytes) in &files {
        match recorded.get(name).and_then(Value::as_str) {
            None => problems.push(format!(
                "{name}: present on disk, absent from PROVENANCE.json"
            )),
            Some(expected) => {
                let actual = file_digest(bytes);
                if actual != *expected {
                    problems.push(format!(
                        "{name}: digest {actual} on disk, PROVENANCE.json records {expected}"
                    ));
                }
            }
        }
    }
    for name in recorded.keys() {
        if !files.contains_key(name) {
            problems.push(format!(
                "{name}: recorded in PROVENANCE.json, missing from disk"
            ));
        }
    }

    let actual_corpus = corpus_digest(&files);
    let expected_corpus = provenance["corpus_digest"]
        .as_str()
        .expect("PROVENANCE.json needs a `corpus_digest` string");
    if actual_corpus != expected_corpus {
        problems.push(format!(
            "corpus digest {actual_corpus} on disk, PROVENANCE.json records {expected_corpus}"
        ));
    }

    assert!(
        problems.is_empty(),
        "the vendored TRACT vectors no longer match their provenance record:\n  {}\n\nIf a vector \
         genuinely changed upstream, re-vendor it — do not edit PROVENANCE.json to match:\n  \
         SOKO_TRACT_VECTORS=<spec>/conformance/vectors cargo test -p soko-node --test conformance \
         -- --ignored refresh_vendored_tract_vectors\nIf a vector was edited locally to make a \
         check pass, that is the exact failure `conformance/README.md` exists to prevent: revert \
         it and take the disagreement to the specification text.",
        problems.join("\n  ")
    );
}

// ==================================================================================================
// Guard 2 — the census in this file describes the corpus on disk
// ==================================================================================================

/// Fails if the corpus grew, shrank, or reshaped without this harness being taught about it.
///
/// The recommendation this implements is "assert coverage counts so a growing corpus cannot
/// silently under-run". [`CORPUS`] is that assertion: a new vector file, or an existing one that
/// gained a case, has to be classified here — as checked, or as uncovered **with a reason** — and
/// there is no third option that compiles.
#[test]
fn vendored_corpus_census_matches_the_files_on_disk() {
    let files = read_json_files(&vendored_dir());

    let mut problems = Vec::new();
    for entry in CORPUS {
        let Some(bytes) = files.get(entry.file) else {
            problems.push(format!(
                "{}: in this file's CORPUS table, not on disk",
                entry.file
            ));
            continue;
        };
        let doc: Value = serde_json::from_slice(bytes).expect("vendored vector must parse");
        let kind = match doc["kind"].as_str() {
            Some("cbor-encoding") => Kind::CborEncoding,
            Some("structural-check") => Kind::StructuralCheck,
            Some("derived-value") => Kind::DerivedValue,
            other => {
                problems.push(format!("{}: unrecognised kind {other:?}", entry.file));
                continue;
            }
        };
        if kind != entry.kind {
            problems.push(format!(
                "{}: CORPUS says {:?}, the file says {kind:?}",
                entry.file, entry.kind
            ));
        }
        let cases = doc["cases"].as_array().map(Vec::len).unwrap_or(0);
        if cases != entry.cases {
            problems.push(format!(
                "{}: CORPUS says {} case(s), the file has {cases}",
                entry.file, entry.cases
            ));
        }
    }
    for name in files.keys() {
        if !CORPUS.iter().any(|v| v.file == name) {
            problems.push(format!(
                "{name}: vendored, but absent from this file's CORPUS table — classify it as \
                 checked or as uncovered with a reason"
            ));
        }
    }

    assert!(
        problems.is_empty(),
        "the TRACT vector corpus and this harness's census of it disagree:\n  {}\n\nA vector this \
         harness has never been told about is a coverage gap that reports nothing, which is the \
         state this file was rewritten to end.",
        problems.join("\n  ")
    );

    // The totals, stated once so they are visible without counting the table. These are facts
    // about the vendored corpus, not targets: they change when the corpus changes, in the same
    // commit that re-vendors it.
    let count = |k: Kind| CORPUS.iter().filter(|v| v.kind == k).count();
    assert_eq!(CORPUS.len(), 21, "vector files");
    assert_eq!(count(Kind::CborEncoding), 15, "cbor-encoding vectors");
    assert_eq!(count(Kind::StructuralCheck), 2, "structural-check vectors");
    assert_eq!(count(Kind::DerivedValue), 4, "derived-value vectors");

    let derived_cases: usize = CORPUS
        .iter()
        .filter(|v| v.kind == Kind::DerivedValue)
        .map(|v| v.cases)
        .sum();
    assert_eq!(derived_cases, 15, "cases across the derived-value vectors");

    // Every derived-value vector is checked. If that ever stops being true, the row needs a reason
    // and this assertion needs to move — deliberately, in a commit that says why.
    for entry in CORPUS.iter().filter(|v| v.kind == Kind::DerivedValue) {
        assert_eq!(
            entry.status,
            Status::Checked,
            "{} is a derived-value vector this harness does not check",
            entry.file
        );
    }
}

// ==================================================================================================
// Guard 3 — the vendored copy has not drifted from upstream
// ==================================================================================================

/// Where an upstream `conformance/vectors` directory might be, in the order tried.
///
/// TRACT was a standalone repository when this harness was first written; it is now a profile
/// inside KOTVA (`profiles/tract`), which is why `../tract` alone had stopped resolving anywhere.
/// Both are tried, so neither layout is privileged.
fn upstream_candidates() -> Vec<PathBuf> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    vec![
        workspace.join("../kotva/profiles/tract/conformance/vectors"),
        workspace.join("../tract/conformance/vectors"),
    ]
}

/// Locate an upstream corpus, or report every path that was tried.
fn upstream_dir() -> Result<PathBuf, Vec<PathBuf>> {
    if let Some(explicit) = std::env::var_os(UPSTREAM_ENV) {
        let dir = PathBuf::from(explicit);
        assert!(
            dir.is_dir(),
            "{UPSTREAM_ENV} is set to {}, which is not a directory. An explicit request to check \
             against upstream must not degrade into a skip — fix the path or unset the variable.",
            dir.display()
        );
        return Ok(dir);
    }
    let candidates = upstream_candidates();
    match candidates.iter().find(|p| p.is_dir()) {
        Some(dir) => Ok(dir.clone()),
        None => Err(candidates),
    }
}

/// Compares the vendored copy against a spec checkout, byte-for-byte, when one is present.
///
/// This is the only check in this file that can be skipped, and the skip is loud: the notice goes
/// to the process's real stderr (see [`loud`]) and names exactly what was **not** verified. What it
/// cannot verify is narrow and worth stating precisely — the conformance checks themselves still
/// ran, against the vendored bytes; what is unverified is only whether those bytes are still what
/// upstream publishes.
#[test]
fn vendored_corpus_matches_the_upstream_spec_checkout() {
    let require = std::env::var(REQUIRE_UPSTREAM_ENV).is_ok_and(|v| v == "1");

    let dir = match upstream_dir() {
        Ok(dir) => dir,
        Err(candidates) => {
            let looked = candidates
                .iter()
                .map(|p| format!("           {}", p.display()))
                .collect::<Vec<_>>()
                .join("\n");
            let notice = format!(
                "\n\
                 ============================================================================\n\
                 SKIPPED: vendored_corpus_matches_the_upstream_spec_checkout\n\
                 \n\
                 NOT VERIFIED in this run: that the 21 vendored TRACT conformance vectors in\n\
                 crates/soko-node/tests/tract-vectors/vectors/ still match the spec repository.\n\
                 If a vector changed upstream, this run did not notice.\n\
                 \n\
                 STILL VERIFIED in this run: every conformance check in tests/conformance.rs,\n\
                 which reads the vendored copies, plus their digests against PROVENANCE.json.\n\
                 A soko-only checkout verifies Soko against the corpus; it cannot verify the\n\
                 corpus against the spec.\n\
                 \n\
                 Looked for a spec checkout at:\n{looked}\n\
                 Point {UPSTREAM_ENV} at a checkout's conformance/vectors to run this check.\n\
                 Set {REQUIRE_UPSTREAM_ENV}=1 to make this skip a hard failure instead.\n\
                 ============================================================================\n\n"
            );
            loud(&notice);
            assert!(
                !require,
                "{REQUIRE_UPSTREAM_ENV}=1 but no upstream spec checkout was found — see the notice \
                 above. This job asked to be strict, so a missing checkout is a misconfiguration, \
                 not a skip."
            );
            return;
        }
    };

    let vendored = read_json_files(&vendored_dir());
    let upstream = read_json_files(&dir);

    let mut problems = Vec::new();
    for (name, bytes) in &vendored {
        match upstream.get(name) {
            None => problems.push(format!("{name}: vendored here, gone from upstream")),
            Some(up) if up != bytes => problems.push(format!(
                "{name}: differs from upstream ({} vendored bytes, {} upstream)",
                bytes.len(),
                up.len()
            )),
            Some(_) => {}
        }
    }
    for name in upstream.keys() {
        if !vendored.contains_key(name) {
            problems.push(format!(
                "{name}: upstream has it, this repository has not vendored it — the corpus grew"
            ));
        }
    }

    assert!(
        problems.is_empty(),
        "the vendored TRACT vectors have drifted from {}:\n  {}\n\nRe-vendor, then work through \
         whatever the re-vendored corpus now disagrees with:\n  SOKO_TRACT_VECTORS={} cargo test \
         -p soko-node --test conformance -- --ignored refresh_vendored_tract_vectors",
        dir.display(),
        problems.join("\n  "),
        dir.display()
    );
}

/// Re-vendors the corpus from an upstream checkout and rewrites `PROVENANCE.json`.
///
/// Ignored by default: it writes to the repository, and it is a maintenance action rather than a
/// check. It is a test rather than a separate binary so that re-vendoring needs no toolchain, no
/// extra crate and no build step a contributor has to discover — the failure messages above name
/// the exact command.
///
/// ```text
/// SOKO_TRACT_VECTORS=../kotva/profiles/tract/conformance/vectors \
///   cargo test -p soko-node --test conformance -- --ignored refresh_vendored_tract_vectors
/// ```
///
/// Re-vendoring is deliberately not a fix for a failing conformance check. If Soko disagrees with a
/// vector, the specification text is the tiebreaker; this command exists for when upstream has
/// genuinely moved.
#[test]
#[ignore = "maintenance action: rewrites the vendored corpus, run explicitly"]
fn refresh_vendored_tract_vectors() {
    let dir = upstream_dir().unwrap_or_else(|candidates| {
        panic!(
            "no upstream spec checkout found; set {UPSTREAM_ENV} or clone one at:\n  {}",
            candidates
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join("\n  ")
        )
    });

    let upstream = read_json_files(&dir);
    assert!(
        !upstream.is_empty(),
        "{} holds no *.json vectors",
        dir.display()
    );

    let target = vendored_dir();
    for name in read_json_files(&target).keys() {
        if !upstream.contains_key(name) {
            std::fs::remove_file(target.join(name)).expect("failed to remove a stale vector");
        }
    }
    for (name, bytes) in &upstream {
        std::fs::write(target.join(name), bytes).expect("failed to write a vector");
    }

    let mut files = serde_json::Map::new();
    for (name, bytes) in &upstream {
        files.insert(name.clone(), Value::String(file_digest(bytes)));
    }

    // Best-effort: a checkout the spec was copied out of usually is a git repository, and recording
    // which commit is the difference between "these bytes came from somewhere" and a reference
    // anyone can go and read. Absence is recorded as such rather than guessed at.
    let commit = std::process::Command::new("git")
        .args(["-C", &dir.to_string_lossy(), "rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown (source was not a git checkout)".to_string());

    let record = serde_json::json!({
        "_": "Provenance for the vendored TRACT conformance vectors in ./vectors/. \
              Recomputed by tests/conformance.rs::refresh_vendored_tract_vectors; \
              checked by tests/conformance.rs::vendored_corpus_matches_its_provenance.",
        "upstream_repo": "https://github.com/vul-os/kotva",
        "upstream_path": "profiles/tract/conformance/vectors",
        "upstream_commit": commit,
        "licence": "CC BY 4.0 — the TRACT specification and its conformance corpus (see \
                    profiles/tract/LICENSE.md upstream).",
        "digest": "BLAKE3, hex. corpus_digest hashes, for each file in name order: the file name, \
                   a 0x00 separator, then the file's bytes.",
        "corpus_digest": corpus_digest(&upstream),
        "files": Value::Object(files),
    });

    let mut text = serde_json::to_string_pretty(&record).expect("provenance must serialise");
    text.push('\n');
    std::fs::write(provenance_path(), text).expect("failed to write PROVENANCE.json");

    loud(&format!(
        "\nre-vendored {} vector file(s) from {}\n\n",
        upstream.len(),
        dir.display()
    ));
}

// ==================================================================================================
// JSON -> Soko value parsing, for the fields these four vectors actually use
// ==================================================================================================

fn country_code(s: &str) -> Country {
    let b = s.as_bytes();
    assert_eq!(
        b.len(),
        2,
        "country code must be 2 ASCII letters, got {s:?}"
    );
    Country([b[0], b[1]])
}

fn currency_code(s: &str) -> Currency {
    let b = s.as_bytes();
    assert_eq!(
        b.len(),
        3,
        "currency code must be 3 ASCII letters, got {s:?}"
    );
    Currency([b[0], b[1], b[2]])
}

fn place_ref(v: &Value) -> PlaceRef {
    let obj = v.as_object().expect("PlaceRef must be a JSON object");
    PlaceRef {
        country: country_code(
            obj["1"]
                .as_str()
                .expect("PlaceRef.1 (country) must be a string"),
        ),
        locality: obj["2"]
            .as_str()
            .expect("PlaceRef.2 (locality) must be a string")
            .to_string(),
    }
}

/// A `money` map — `{1 => minor_units, 2 => currency}` (§16.3).
fn money(v: &Value) -> Money {
    Money {
        minor_units: v["1"].as_i64().expect("money.1 must be an int"),
        currency: currency_code(v["2"].as_str().expect("money.2 must be a string")),
    }
}

/// `RailClass = 0 / 1` (§16.5.4) — a plain small integer, not a nested map.
fn rail_class(v: &Value) -> RailClass {
    match v.as_u64() {
        Some(0) => RailClass::CustodialReversible,
        Some(1) => RailClass::NonCustodialFinal,
        other => panic!("RailClass must be 0 or 1 per §16.5.4, got {other:?}"),
    }
}

/// Parse the vector's `fulfilment` object into `soko_offer::Fulfilment`.
///
/// Each variant is a map keyed by its §16.5.2 CDDL field number (`"0"`..`"6"`); most variants
/// carry their payload directly at that key, but `return-required` (key `"6"`) is a two-field
/// variant and carries its second field (`term_days`) as a **sibling** key `"7"` in the same map,
/// not nested — matching `offer-fulfilment.json`'s encoding exactly (see that vector's `return-
/// required` case).
fn fulfilment(v: &Value) -> Fulfilment {
    let obj = v
        .as_object()
        .expect("fulfilment must be an object keyed by its §16.5.2 CDDL field number");
    if let Some(to) = obj.get("0") {
        let to = to
            .as_array()
            .expect("Fulfilment.0 (ship destinations) must be an array")
            .iter()
            .map(|c| country_code(c.as_str().expect("destination must be a country string")))
            .collect();
        return Fulfilment::Ship { to };
    }
    if let Some(at) = obj.get("1") {
        return Fulfilment::Collect { at: place_ref(at) };
    }
    if obj.contains_key("2") {
        return Fulfilment::DigitalGrant;
    }
    if let Some(at) = obj.get("3") {
        return Fulfilment::PerformAtPlace { at: place_ref(at) };
    }
    if obj.contains_key("4") {
        return Fulfilment::PerformRemote;
    }
    if obj.contains_key("5") {
        let at = &obj["5"];
        return Fulfilment::AccessGrant {
            at: if at.is_null() {
                None
            } else {
                Some(place_ref(at))
            },
        };
    }
    if let Some(at) = obj.get("6") {
        let term_days = obj
            .get("7")
            .and_then(Value::as_u64)
            .expect("return-required (key 6) needs a sibling key 7 (term_days)")
            as u16;
        return Fulfilment::ReturnRequired {
            at: place_ref(at),
            term_days,
        };
    }
    panic!(
        "unrecognised Fulfilment CDDL keys in vector input: {:?}",
        obj.keys().collect::<Vec<_>>()
    );
}

// ==================================================================================================
// A small report so one test run can check every case in every vector and print one honest
// summary, rather than panicking (and stopping) on the first mismatch. A disagreement between a
// hand-derived vector and Soko is the single most valuable thing this file could find — stopping
// early would mean finding at most one per run.
// ==================================================================================================

#[derive(Default)]
struct Report {
    checked: Vec<String>,
    disagreements: Vec<String>,
    uncovered: Vec<String>,
}

impl Report {
    fn check(&mut self, name: impl Into<String>, ok: bool, detail: impl Into<String>) {
        let name = name.into();
        if ok {
            self.checked.push(name);
        } else {
            self.disagreements
                .push(format!("{name} — {}", detail.into()));
        }
    }

    fn uncovered(&mut self, name: impl Into<String>, case_count: usize, reason: impl Into<String>) {
        let plural = if case_count == 1 { "" } else { "s" };
        self.uncovered.push(format!(
            "{} ({case_count} case{plural}): {}",
            name.into(),
            reason.into()
        ));
    }

    /// Print the honest summary and fail loudly if anything disagreed. Uncovered cases are
    /// reported, never treated as a failure — they are an accurate statement of what Soko does not
    /// yet implement, not a bug in this test.
    fn finish(self) {
        println!("\n=== TRACT conformance summary (derived-value vectors) ===");
        println!("verified against Soko, all agreed: {}", self.checked.len());
        for c in &self.checked {
            println!("  ok  {c}");
        }
        println!(
            "explicitly uncovered:               {}",
            self.uncovered.len()
        );
        for u in &self.uncovered {
            println!("  --  {u}");
        }
        println!(
            "disagreements:                       {}",
            self.disagreements.len()
        );
        for d in &self.disagreements {
            println!("  !!  {d}");
        }
        println!("===========================================================\n");

        assert!(
            self.disagreements.is_empty(),
            "Soko disagrees with {} hand-derived TRACT vector case(s). Per conformance/README.md: \
             investigate which one is wrong, with the specification text as the tiebreaker — do \
             NOT edit the vector to match Soko, that is the exact failure mode the discipline \
             exists to prevent. Disagreements:\n{}",
            self.disagreements.len(),
            self.disagreements.join("\n")
        );

        assert_eq!(
            self.checked.len(),
            EXPECTED_CHECKS,
            "this run performed {} check(s), not the {EXPECTED_CHECKS} a full run performs. A \
             check family stopped running — most likely a vector was reshaped and a loop is now \
             iterating an empty array. An under-run that still prints `ok` is the failure this \
             count exists to catch.",
            self.checked.len()
        );
    }
}

// ==================================================================================================
// TRACT-DELIV-01 — billable weight, bracket selection, and the bracket's price
// ==================================================================================================

fn check_billable_weight(dir: &Path, report: &mut Report) {
    let doc =
        load(&dir.join("billable-weight-and-local-rate-card-price-lookup-tract-deliv-01.json"));
    let cases = doc["cases"].as_array().expect("cases must be an array");

    // "All three cases reuse zone 1's brackets from TRACT-WIRE-VEC-14's RateCard example" — that
    // example is `ratecard.json`, so the brackets are read from it rather than retyped here. A
    // second copy of a published price list is a second thing that can be wrong.
    let card = load(&dir.join("ratecard.json"));
    let zone_json = card["cases"][0]["input"]["3"]
        .as_array()
        .expect("RateCard.3 (zones) must be an array")
        .iter()
        .find(|z| z["1"].as_u64() == Some(1))
        .expect("ratecard.json must carry a zone 1");
    let zone = Zone {
        id: 1,
        brackets: zone_json["2"]
            .as_array()
            .expect("Zone.2 (brackets) must be an array")
            .iter()
            .map(|b| WeightBracket {
                max_grams: b["1"].as_u64().expect("WeightBracket.1 must be an int") as u32,
                price: money(&b["2"]),
            })
            .collect(),
        transit_days: zone_json["3"].as_u64().expect("Zone.3 must be an int") as u8,
    };

    for case in cases {
        let name = case["name"].as_str().unwrap_or("<unnamed case>");
        let input = &case["input"];
        let expected = &case["expected"];

        let actual_grams = input["actual_grams"].as_u64().unwrap() as u32;
        let l = input["l_cm"].as_u64().unwrap() as u32;
        let w = input["w_cm"].as_u64().unwrap() as u32;
        let h = input["h_cm"].as_u64().unwrap() as u32;
        let divisor = input["dim_divisor"].as_u64().unwrap() as u32;

        // `Parcel`'s fields are typed in millimetres and the vector's L/W/H are centimetres, but
        // `billable_grams`'s formula (`max(actual, L*W*H / divisor)`) is unit-agnostic
        // multiply-then-divide arithmetic with no unit conversion inside it — feeding it the
        // vector's cm-denominated numbers checks the identical arithmetic the vector checks. This
        // is not a claim that Soko treats these values as centimetres; it is reusing the one
        // function that performs this arithmetic to check the arithmetic itself.
        let parcel = Parcel {
            length_mm: l,
            width_mm: w,
            height_mm: h,
            weight_grams: actual_grams,
        };
        let billable = parcel.billable_grams(divisor);
        let expected_billable = expected["billable_grams"].as_u64().unwrap() as u32;

        report.check(
            format!("TRACT-DELIV-01 billable weight — {name}"),
            billable == expected_billable,
            format!(
                "Parcel::billable_grams({divisor}) = {billable}, vector expects \
                 billable_grams = {expected_billable}"
            ),
        );

        // The second half of the vector's `formula` field (`billable_weight, select_bracket`).
        // `Zone::select_bracket` and `RateCard::quote` did not exist when this harness was
        // written and it recorded them as uncovered; they exist now, and this is the check that
        // records would have caught the day they landed had the harness been running at all.
        let bracket = zone.select_bracket(billable);
        let expected_max = expected["selected_bracket_max_grams"].as_u64().unwrap() as u32;
        report.check(
            format!("TRACT-DELIV-01 bracket selection — {name}"),
            bracket.map(|b| b.max_grams) == Some(expected_max),
            format!(
                "Zone::select_bracket({billable}) = {:?}, vector expects a bracket with \
                 max_grams = {expected_max}",
                bracket.map(|b| b.max_grams)
            ),
        );

        let expected_price = expected["price_eur_minor_units"].as_i64().unwrap();
        report.check(
            format!("TRACT-DELIV-01 bracket price — {name}"),
            bracket.map(|b| (b.price.minor_units, b.price.currency))
                == Some((expected_price, Currency(*b"EUR"))),
            format!(
                "selected bracket price = {:?}, vector expects {expected_price} EUR minor units",
                bracket.map(|b| b.price)
            ),
        );
    }

    // What the vector states and Soko does not expose: the volumetric weight on its own.
    // `Parcel::billable_grams` computes `max(actual, volumetric)` and returns only the maximum, so
    // the three `volumetric_grams` values are checked only where they happen to be the maximum.
    // Not a gap worth a function on its own — a volumetric weight is not a quantity anything in
    // TRACT prices against — but stating it beats implying the whole `expected` object was checked.
    report.uncovered(
        "TRACT-DELIV-01 volumetric weight in isolation (volumetric_grams)",
        cases.len(),
        "Parcel::billable_grams folds volumetric weight into max(actual, volumetric) and returns \
         only the result; no function returns the volumetric component alone",
    );
}

// ==================================================================================================
// TRACT-SETTLE-01 — EscrowScope checkout intersection
// ==================================================================================================

/// Build an `EscrowScope` from the vector's §16.5.4-keyed map.
///
/// Keys 1 (operator) and 9 (authorities) are absent from this vector's scopes, and key 10
/// (`declines`, decision D1) postdates §16.5.4 as vendored — so all three are supplied empty here
/// and nothing below asserts anything about them.
fn escrow_scope(v: &Value) -> EscrowScope {
    let list = |key: &str| -> &Vec<Value> {
        v[key]
            .as_array()
            .unwrap_or_else(|| panic!("EscrowScope.{key} must be an array"))
    };
    EscrowScope {
        operator: IdentityKey(vec![0]),
        buyer_countries: list("2")
            .iter()
            .map(|c| country_code(c.as_str().unwrap()))
            .collect(),
        seller_countries: list("3")
            .iter()
            .map(|c| country_code(c.as_str().unwrap()))
            .collect(),
        supply_countries: list("4")
            .iter()
            .map(|c| country_code(c.as_str().unwrap()))
            .collect(),
        currencies: list("5")
            .iter()
            .map(|c| currency_code(c.as_str().unwrap()))
            .collect(),
        rail_classes: list("6").iter().map(rail_class).collect(),
        max_order_value: money(&v["7"]),
        excluded_categories: list("8")
            .iter()
            .map(|c| c.as_str().unwrap().to_string())
            .collect(),
        authorities: vec![],
        declines: vec![],
    }
}

fn check_escrow_scope_intersection(dir: &Path, report: &mut Report) {
    let doc = load(&dir.join("escrowscope-checkout-intersection.json"));
    let cases = doc["cases"].as_array().expect("cases must be an array");

    for case in cases {
        let name = case["name"].as_str().unwrap_or("<unnamed case>");
        let buyer = escrow_scope(&case["input"]["buyer_scope"]);
        let gateway = escrow_scope(&case["input"]["gateway_scope"]);
        let expected = &case["expected"];

        // §9.4 describes the buyer's node narrowing the gateway's declared scope, so the buyer's
        // scope is the receiver. The vector's `expected.scope` carries no key 1, i.e. the operator
        // identity is not part of what it derives — and Soko keeps the receiver's, which no case
        // here contradicts.
        let actual = buyer.intersect(&gateway);

        if expected["refused"].as_bool().expect("expected.refused") {
            report.check(
                format!("TRACT-SETTLE-01 empty intersection refused — {name}"),
                actual.is_none(),
                format!("EscrowScope::intersect(..) = {actual:?}, vector expects a refusal"),
            );
            continue;
        }

        let want = &expected["scope"];
        let mismatch = match &actual {
            None => Some("refused, but the vector expects a narrowed scope".to_string()),
            Some(got) => {
                let want_countries = |key: &str| -> Vec<Country> {
                    want[key]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|c| country_code(c.as_str().unwrap()))
                        .collect()
                };
                // Order is compared for the set-valued fields because every one of them intersects
                // to a single element in this vector, so there is no ordering question to answer.
                // `excluded_categories` is the exception: it unions to two elements, and the
                // vector's own note derives it as the SET union {weapons} ∪ {alcohol} while the
                // JSON has to serialise that set in some order. Comparing it as a set is therefore
                // comparing what was derived; comparing the order would be asserting something the
                // vector does not state.
                let mut got_excluded = got.excluded_categories.clone();
                got_excluded.sort();
                let mut want_excluded: Vec<String> = want["8"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|c| c.as_str().unwrap().to_string())
                    .collect();
                want_excluded.sort();

                let checks: [(&str, bool); 7] = [
                    (
                        "buyer_countries",
                        got.buyer_countries == want_countries("2"),
                    ),
                    (
                        "seller_countries",
                        got.seller_countries == want_countries("3"),
                    ),
                    (
                        "supply_countries",
                        got.supply_countries == want_countries("4"),
                    ),
                    (
                        "currencies",
                        got.currencies
                            == want["5"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|c| currency_code(c.as_str().unwrap()))
                                .collect::<Vec<_>>(),
                    ),
                    (
                        "rail_classes",
                        got.rail_classes
                            == want["6"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(rail_class)
                                .collect::<Vec<_>>(),
                    ),
                    ("max_order_value", got.max_order_value == money(&want["7"])),
                    ("excluded_categories", got_excluded == want_excluded),
                ];
                let wrong: Vec<&str> = checks
                    .iter()
                    .filter(|(_, ok)| !ok)
                    .map(|(field, _)| *field)
                    .collect();
                (!wrong.is_empty()).then(|| format!("fields disagreeing: {}", wrong.join(", ")))
            }
        };

        report.check(
            format!("TRACT-SETTLE-01 scope intersection — {name}"),
            mismatch.is_none(),
            format!(
                "EscrowScope::intersect(..) = {actual:?}; {}",
                mismatch.unwrap_or_default()
            ),
        );
    }

    // §9.4 requires an empty intersection be *disclosed*, and the vector's refusal case carries the
    // disclosure as a `reason` naming the field that emptied ("empty intersection on: currencies").
    // `EscrowScope::intersect` returns `Option<EscrowScope>`: `None` discloses that the scopes are
    // incompatible but not which dimension made them so, so the check above can only compare the
    // refusal, not its reason. Narrower than the vector, and said so rather than counted as passed.
    report.uncovered(
        "TRACT-SETTLE-01 refusal reason (which field intersected to empty)",
        1,
        "EscrowScope::intersect returns Option<EscrowScope>; None carries no indication of which \
         dimension emptied, so the vector's `reason` string has nothing on the Soko side to \
         compare against",
    );
}

// ==================================================================================================
// TRACT-FULF-01 / TRACT-FULF-02 — place-of-supply derivation across all 7 Fulfilment variants
// ==================================================================================================

fn check_place_of_supply(dir: &Path, report: &mut Report) {
    let doc = load(&dir.join(
        "place-of-supply-derivation-for-all-7-fulfilment-variants-tract-fulf-01-tract-fulf-02.json",
    ));

    // The vector's own description holds seller establishment (ZA) and buyer residence (DE)
    // constant across every case, specifically so a reader can see the derived anchor is never
    // either of those two values except where §4.3 says it should equal buyer residence. Those two
    // constants are NOT repeated in each case's `input` JSON (only in the vector's prose) — seller
    // establishment plays no role in `place_of_supply`'s signature at all (correctly: that is the
    // whole point of TRACT-FULF-01), so only buyer residence needs supplying here.
    let buyer_residence = country_code("DE");

    for case in doc["cases"].as_array().expect("cases must be an array") {
        let name = case["name"].as_str().unwrap_or("<unnamed case>");
        let input = &case["input"];
        let f = fulfilment(&input["fulfilment"]);
        let chosen_destination = input
            .get("chosen_destination")
            .and_then(Value::as_str)
            .map(country_code);
        let expected = country_code(
            case["expected"]
                .as_str()
                .expect("expected must be a country code string"),
        );

        let actual = place_of_supply(&f, buyer_residence, chosen_destination);
        report.check(
            format!("TRACT-FULF-01/02 place of supply — {name}"),
            actual == Ok(expected),
            format!("place_of_supply(..) = {actual:?}, vector expects Ok({expected:?})"),
        );
    }
}

// ==================================================================================================
// TRACT-DELIV-03 — route totals: same-currency sum, mixed-currency refusal
// ==================================================================================================

fn check_route_totals(dir: &Path, report: &mut Report) {
    let doc = load(&dir.join("route-totals.json"));

    for case in doc["cases"].as_array().expect("cases must be an array") {
        let name = case["name"].as_str().unwrap_or("<unnamed case>");
        let legs_json = case["input"]
            .as_array()
            .expect("input must be an array of legs");

        let legs: Vec<Leg> = legs_json
            .iter()
            .enumerate()
            .map(|(i, leg)| {
                let minor_units = leg["2"]
                    .as_i64()
                    .expect("leg.2 (minor_units) must be an int");
                let currency = currency_code(
                    leg["3"]
                        .as_str()
                        .expect("leg.3 (currency) must be a string"),
                );
                Leg {
                    carrier: IdentityKey(vec![i as u8]),
                    source: RateSource::LiveQuoteRequired,
                    cost: Money {
                        minor_units,
                        currency,
                    },
                    transit_days: 1,
                }
            })
            .collect();

        // Zero hub fees, denominated in the first leg's currency so hub_fees itself never
        // introduces a currency mismatch this vector is not testing for.
        let hub_currency = legs
            .first()
            .map(|l| l.cost.currency)
            .unwrap_or(Currency(*b"USD"));
        let route = RouteOption {
            shape: RouteShape::Direct,
            legs,
            wait_days: 0,
            hub_fees: Money {
                minor_units: 0,
                currency: hub_currency,
            },
        };

        let result = route.total();
        let expected_refused = case["expected"]["refused"]
            .as_bool()
            .expect("expected.refused must be a bool");

        match (&result, expected_refused) {
            (Ok(total), false) => {
                let exp = &case["expected"]["total"];
                let exp_minor = exp["1"].as_i64().expect("expected.total.1 must be an int");
                let exp_currency = currency_code(
                    exp["2"]
                        .as_str()
                        .expect("expected.total.2 must be a string"),
                );
                report.check(
                    format!("TRACT-DELIV-03 route total — {name}"),
                    total.minor_units == exp_minor && total.currency == exp_currency,
                    format!(
                        "RouteOption::total() = {total:?}, vector expects \
                         {{minor_units: {exp_minor}, currency: {exp_currency:?}}}"
                    ),
                );
            }
            (Err(_), true) => {
                report.check(format!("TRACT-DELIV-03 route total — {name}"), true, "");
            }
            _ => {
                report.check(
                    format!("TRACT-DELIV-03 route total — {name}"),
                    false,
                    format!(
                        "RouteOption::total() = {result:?}, vector expects refused = {expected_refused}"
                    ),
                );
            }
        }
    }
}

// ==================================================================================================
// The orchestrating test
// ==================================================================================================

/// Runs unconditionally. The vectors are vendored, so there is no checkout, network or environment
/// state that can turn this into a pass that verified nothing.
#[test]
fn tract_hand_derived_vectors_agree_with_soko() {
    let dir = vendored_dir();
    let mut report = Report::default();
    check_billable_weight(&dir, &mut report);
    check_escrow_scope_intersection(&dir, &mut report);
    check_place_of_supply(&dir, &mut report);
    check_route_totals(&dir, &mut report);
    report.finish();
}

// ==================================================================================================
// Round-trip serialisation coverage for every other crate's public serialisable types.
//
// `soko-core`'s own types are covered in `soko-core/src/lib.rs`'s tests, next to the
// `soko_core::roundtrip` helpers this file reuses (see that module's doc comment for why byte
// STABILITY, not mere round-trip equality, is the property worth checking, and why decoding needs
// its own explicit "all bytes consumed" check rather than trusting `ciborium::from_reader` alone).
// This section extends the same check across every other crate in the workspace, since soko-node
// already depends on all of them and is the one place a single test file can reach every type.
// ==================================================================================================

fn key(n: u8) -> IdentityKey {
    IdentityKey(vec![n])
}
fn addr(n: u8) -> ContentAddress {
    ContentAddress(vec![n])
}
fn zar(n: i64) -> Money {
    Money {
        minor_units: n,
        currency: Currency(*b"ZAR"),
    }
}
fn place(country: &str, locality: &str) -> PlaceRef {
    PlaceRef {
        country: country_code(country),
        locality: locality.to_string(),
    }
}

#[test]
fn catalog_types_roundtrip_stably() {
    let record = ProductRecord {
        name: "Field Notebook".into(),
        description: "pocket notebook".into(),
        attributes: vec![
            Attribute {
                key: "colour".into(),
                value: "black".into(),
            },
            Attribute {
                key: "size".into(),
                value: "A6".into(),
            },
        ],
        identity: vec![
            IdentityRung::ContentAddress(addr(1)),
            IdentityRung::ClaimedExternal {
                scheme: "gtin".into(),
                value: "0012345".into(),
            },
            IdentityRung::ManufacturerSigned(key(2)),
        ],
        group: Some(addr(9)),
        components: vec![addr(3), addr(4)],
    };
    roundtrip::assert_stable(&record);
    roundtrip::assert_truncation_is_rejected(&record);
    roundtrip::assert_trailing_garbage_is_rejected(&record);

    roundtrip::assert_stable(&Attribute {
        key: "colour".into(),
        value: "black".into(),
    });

    let group = ProductGroup {
        name: "Field Notebook".into(),
        varies_by: vec!["colour".into(), "size".into()],
        variants: vec![addr(1), addr(2)],
    };
    roundtrip::assert_stable(&group);
}

#[test]
fn offer_types_roundtrip_stably() {
    for item in [
        Item::Product(addr(1)),
        Item::VariantOfGroup {
            group: addr(2),
            variant: addr(3),
        },
        Item::Service(addr(4)),
        Item::Right(addr(5)),
        Item::Capacity(addr(6)),
    ] {
        roundtrip::assert_stable(&item);
    }

    for availability in [
        Availability::Count(StockSignal::Exact(10)),
        Availability::Count(StockSignal::InStock),
        Availability::Count(StockSignal::Low),
        Availability::Count(StockSignal::OutOfStock),
        Availability::TimeSlots {
            ical: "FREQ=WEEKLY".into(),
            slot_minutes: 30,
        },
        Availability::CapacityPerInterval {
            capacity: 40,
            ical: "FREQ=WEEKLY;BYDAY=SA".into(),
        },
        Availability::Unlimited,
        Availability::MadeToOrder { lead_days: 14 },
    ] {
        roundtrip::assert_stable(&availability);
    }

    let venue = place("FR", "Cannes");
    for fulfilment in [
        Fulfilment::Ship {
            to: vec![country_code("FR"), country_code("DE")],
        },
        Fulfilment::Collect {
            at: place("US", "Portland"),
        },
        Fulfilment::DigitalGrant,
        Fulfilment::PerformAtPlace { at: venue.clone() },
        Fulfilment::PerformRemote,
        Fulfilment::AccessGrant {
            at: Some(place("GB", "London")),
        },
        Fulfilment::AccessGrant { at: None },
        Fulfilment::ReturnRequired {
            at: place("JP", "Osaka"),
            term_days: 30,
        },
    ] {
        roundtrip::assert_stable(&fulfilment);
    }
    roundtrip::assert_stable(&venue);

    for consideration in [
        Consideration::Fixed(zar(45_000)),
        Consideration::Tiered(vec![
            PriceTier {
                min_qty: 1,
                unit_price: zar(100),
            },
            PriceTier {
                min_qty: 10,
                unit_price: zar(90),
            },
        ]),
        Consideration::Recurring {
            amount: zar(5_000),
            rrule: "FREQ=MONTHLY".into(),
        },
        Consideration::Metered {
            dimension: "gigabytes".into(),
            unit_price: zar(50),
        },
        Consideration::DepositBalance {
            deposit: zar(10_000),
            balance: zar(35_000),
        },
        Consideration::QuoteRequired,
    ] {
        roundtrip::assert_stable(&consideration);
    }

    let offer = Offer {
        item: Item::Product(addr(1)),
        availability: Availability::Count(StockSignal::Exact(10)),
        fulfilment: Fulfilment::Ship {
            to: vec![country_code("NZ")],
        },
        consideration: Consideration::Fixed(zar(45_000)),
        sell_to: vec![country_code("NZ"), country_code("ZA")],
        published: Timestamp(1_700_000_000_000),
    };
    roundtrip::assert_stable(&offer);
    roundtrip::assert_truncation_is_rejected(&offer);
    roundtrip::assert_trailing_garbage_is_rejected(&offer);
}

#[test]
fn delivery_types_roundtrip_stably() {
    let bracket = WeightBracket {
        max_grams: 1000,
        price: zar(500),
    };
    roundtrip::assert_stable(&bracket);

    let zone = Zone {
        id: 1,
        brackets: vec![
            bracket.clone(),
            WeightBracket {
                max_grams: 5000,
                price: zar(900),
            },
        ],
        transit_days: 3,
    };
    roundtrip::assert_stable(&zone);

    let card = RateCard {
        carrier: key(20),
        serves: vec![country_code("FR"), country_code("DE")],
        zones: vec![zone],
        dim_divisor: 5000,
        surcharge_pct: 3,
        excluded_categories: vec!["hazmat".into()],
        published: Timestamp(1_700_000_000_000),
    };
    roundtrip::assert_stable(&card);
    roundtrip::assert_truncation_is_rejected(&card);
    roundtrip::assert_trailing_garbage_is_rejected(&card);

    let parcel = Parcel {
        length_mm: 220,
        width_mm: 140,
        height_mm: 30,
        weight_grams: 300,
    };
    roundtrip::assert_stable(&parcel);

    for source in [
        RateSource::PublishedCard(addr(30)),
        RateSource::LiveQuoteRequired,
    ] {
        roundtrip::assert_stable(&source);
    }

    let leg = Leg {
        carrier: key(20),
        source: RateSource::PublishedCard(addr(30)),
        cost: zar(9_000),
        transit_days: 7,
    };
    roundtrip::assert_stable(&leg);

    let handoff = CustodyHandoff {
        from: key(1),
        to: key(2),
        at: Timestamp(1_700_000_100_000),
    };
    roundtrip::assert_stable(&handoff);

    let consignment = Consignment {
        order: addr(50),
        custodian: key(2),
        handoffs: vec![handoff],
    };
    roundtrip::assert_stable(&consignment);

    let capacity = DistributorCapacity {
        distributor: key(3),
        country: country_code("ZA"),
        locality: "Cape Town".into(),
        storage_per_day: zar(20),
        handling_fee: zar(150),
        slots: 50,
        excluded_categories: vec!["alcohol".into()],
    };
    roundtrip::assert_stable(&capacity);

    for shape in [
        RouteShape::Direct,
        RouteShape::HubNearBuyer,
        RouteShape::HubNearSellers,
    ] {
        roundtrip::assert_stable(&shape);
    }

    let route = RouteOption {
        shape: RouteShape::HubNearBuyer,
        legs: vec![leg],
        wait_days: 3,
        hub_fees: zar(1_200),
    };
    roundtrip::assert_stable(&route);
    roundtrip::assert_truncation_is_rejected(&route);
    roundtrip::assert_trailing_garbage_is_rejected(&route);
}

#[test]
fn order_types_roundtrip_stably() {
    let line = CartLine {
        offer: addr(1),
        seller: key(1),
        quantity: 2,
    };
    roundtrip::assert_stable(&line);

    let cart = Cart {
        lines: vec![
            line.clone(),
            CartLine {
                offer: addr(2),
                seller: key(2),
                quantity: 1,
            },
        ],
    };
    roundtrip::assert_stable(&cart);

    let counter = BoundedCounter::new(key(11), 6);
    roundtrip::assert_stable(&counter);

    for state in [
        OrderState::Draft,
        OrderState::Placed,
        OrderState::Accepted,
        OrderState::Declined,
        OrderState::Countered,
        OrderState::Fulfilling,
        OrderState::Delivered,
        OrderState::Closed,
        OrderState::Cancelled,
    ] {
        roundtrip::assert_stable(&state);
    }

    let order = Order {
        buyer: key(9),
        seller: key(1),
        lines: vec![line],
        total: zar(90_000),
        state: OrderState::Placed,
        placed: Timestamp(1_700_000_100_000),
    };
    roundtrip::assert_stable(&order);
    roundtrip::assert_truncation_is_rejected(&order);
    roundtrip::assert_trailing_garbage_is_rejected(&order);
}

#[test]
fn settle_types_roundtrip_stably() {
    for class in [RailClass::CustodialReversible, RailClass::NonCustodialFinal] {
        roundtrip::assert_stable(&class);
    }
    for sub in [RailSubstitution::Agreed, RailSubstitution::Refused] {
        roundtrip::assert_stable(&sub);
    }

    let attestation = PaymentAttestation {
        payer: key(1),
        payee: key(2),
        order: addr(50),
        amount: zar(90_000),
        rail_class: RailClass::CustodialReversible,
        external_ref: "ref-123".into(),
        at: Timestamp(1_700_100_000_000),
    };
    roundtrip::assert_stable(&attestation);

    let scope = EscrowScope {
        operator: key(40),
        buyer_countries: vec![country_code("NZ")],
        seller_countries: vec![country_code("ZA")],
        supply_countries: vec![country_code("NZ")],
        currencies: vec![Currency(*b"ZAR")],
        rail_classes: vec![RailClass::CustodialReversible],
        max_order_value: zar(500_000),
        excluded_categories: vec!["alcohol".into()],
        authorities: vec!["example authorisation".into()],
        declines: vec![],
    };
    roundtrip::assert_stable(&scope);
    roundtrip::assert_truncation_is_rejected(&scope);
    roundtrip::assert_trailing_garbage_is_rejected(&scope);

    for state in [
        EscrowState::Funded,
        EscrowState::Held,
        EscrowState::Released,
        EscrowState::Refunded,
        EscrowState::Split,
    ] {
        roundtrip::assert_stable(&state);
    }

    let ruling = EscrowRuling {
        operator: key(40),
        order: addr(50),
        state: EscrowState::Released,
        to_seller: zar(88_000),
        to_buyer: zar(0),
        reason: "delivered, no dispute".into(),
        at: Timestamp(1_700_200_000_000),
    };
    roundtrip::assert_stable(&ruling);
    roundtrip::assert_truncation_is_rejected(&ruling);
    roundtrip::assert_trailing_garbage_is_rejected(&ruling);
}

#[test]
fn trust_types_roundtrip_stably() {
    for attestor in [Attestor::Seller, Attestor::Escrow] {
        roundtrip::assert_stable(&attestor);
    }

    let purchase = PurchaseAttestation {
        attestor: Attestor::Escrow,
        issuer: key(40),
        order: addr(50),
        at: Timestamp(1_700_100_000_000),
    };
    roundtrip::assert_stable(&purchase);

    for subject in [
        Subject::Product(addr(1)),
        Subject::Seller(key(1)),
        Subject::Distributor(key(2)),
        Subject::Courier(key(3)),
    ] {
        roundtrip::assert_stable(&subject);
    }

    let review = Review {
        subject: Subject::Seller(key(1)),
        author: key(99),
        score: 5,
        body: "arrived intact".into(),
        attestation: Some(purchase),
        at: Timestamp(1_700_100_000_000),
    };
    roundtrip::assert_stable(&review);
    roundtrip::assert_truncation_is_rejected(&review);
    roundtrip::assert_trailing_garbage_is_rejected(&review);

    let unattested = Review {
        attestation: None,
        ..review
    };
    roundtrip::assert_stable(&unattested);
}

#[test]
fn jurisdiction_types_roundtrip_stably() {
    let anchors = Anchors {
        seller_establishment: country_code("ZA"),
        buyer_residence: country_code("NZ"),
        place_of_supply: country_code("NZ"),
        delivery_destination: Some(country_code("NZ")),
    };
    roundtrip::assert_stable(&anchors);
    roundtrip::assert_truncation_is_rejected(&anchors);
    roundtrip::assert_trailing_garbage_is_rejected(&anchors);

    let rep = InRegionRepresentative {
        region: country_code("DE"),
        who: key(7),
    };
    roundtrip::assert_stable(&rep);

    let parties = ResponsibleParties {
        seller_of_record: key(2),
        facilitator: Some(key(40)),
        importer_of_record: None,
        responsible_person: Some(rep),
    };
    roundtrip::assert_stable(&parties);
    roundtrip::assert_truncation_is_rejected(&parties);
    roundtrip::assert_trailing_garbage_is_rejected(&parties);
}

#[test]
fn gateway_types_roundtrip_stably() {
    for host in [
        StoreHost::Subdomain {
            label: "alice".into(),
            base: "soko.example".into(),
        },
        StoreHost::Custom("shop.example".into()),
    ] {
        roundtrip::assert_stable(&host);
    }

    let binding = StoreBinding {
        seller: key(1),
        host: StoreHost::Subdomain {
            label: "alice".into(),
            base: "soko.example".into(),
        },
    };
    roundtrip::assert_stable(&binding);
    roundtrip::assert_truncation_is_rejected(&binding);
    roundtrip::assert_trailing_garbage_is_rejected(&binding);
}

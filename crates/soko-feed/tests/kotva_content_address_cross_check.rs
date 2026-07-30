//! Cross-repo backlog A11: does `soko_feed::address` actually agree with the substrate's own
//! content-addressing code, or only with this crate's restatement of the spec text?
//!
//! TRACT §16.2 ("Conventions inherited, not reinvented") requires content addresses to use "the
//! DMTAP substrate['s]" hash construction — normatively, `kotva_core::id::ContentId`: a
//! multihash-style agility prefix (`0x1e` for BLAKE3-256) followed by the 32-byte digest
//! (`kotva/profiles/tract/16-wire-format.md` §16.2-§16.3, `kotva/crates/kotva-core/src/id.rs`).
//! `soko_feed::address` reimplements that construction independently — `soko` has no dependency,
//! path or crates.io, on `kotva-core` in its production build (workspace `Cargo.toml`'s comment:
//! "Signing and content addressing... implemented here rather than invented" — TRACT names the
//! substrate as the source of the convention, but the code was never wired to it).
//!
//! That is exactly the shape backlog item A11 warns about: an invariant (this exact byte
//! construction) that TWO implementations must agree on for TRACT objects to resolve against a
//! kotva-substrate holder, each hand-written from the same three sentences of prose, with nothing
//! machine-checking that they still agree. Prior art for closing this kind of gap already exists
//! in the sibling `magnetite` repo (`magnetite-kotva`'s
//! `kotva_content_id_is_magnetites_blake3_plus_an_agility_prefix`, which does the same thing for
//! magnetite's bare-`Hash` convention) — this test is the same idea applied to soko's
//! multihash-prefixed one.
//!
//! `kotva-core` is a **dev-dependency only** (see `Cargo.toml`): nothing in soko's production
//! dependency graph changes. This crate's own philosophy — "TRACT introduces no new hash
//! construction... implemented here rather than invented" — is unaffected; this test only proves
//! the existing implementation is honest about which construction it claims to implement.

use soko_feed::address;

/// For every one of these inputs, `soko_feed::address(bytes)` must be byte-identical to
/// `kotva_core::ContentId::of(bytes)` — same 33 bytes: the `0x1e` BLAKE3-256 agility prefix
/// followed by the 32-byte digest. If it isn't, a TRACT object soko publishes and a kotva
/// substrate holder resolves would disagree about that object's own identity.
#[test]
fn address_matches_kotva_contentid_byte_for_byte() {
    let inputs: Vec<&[u8]> = vec![
        b"",
        b"x",
        b"the very same product record",
        b"\x00\x01\x02\xff\xfe\xfd",
        &[0u8; 1],
        &[0xABu8; 1_000],
    ];
    assert_eq!(
        inputs.len(),
        6,
        "coverage: every listed input must actually run"
    );

    let mut checked = 0;
    for bytes in &inputs {
        let soko_addr = address(bytes);
        let kotva_id = kotva_core::ContentId::of(bytes);

        assert_eq!(
            soko_addr.0,
            kotva_id.as_bytes(),
            "soko_feed::address and kotva_core::ContentId disagree for input {bytes:?}"
        );
        checked += 1;
    }
    assert_eq!(
        checked,
        inputs.len(),
        "every input must have been compared, not skipped"
    );
}

/// Pin the two facts that make the byte-for-byte equality above meaningful rather than
/// coincidental: both sides actually use the `0x1e` multiformats BLAKE3 code, and both produce a
/// 33-byte value (1-byte prefix + 32-byte digest) — so a future accidental change to either
/// constant would be caught here even if some unlucky input happened to still round-trip above.
#[test]
fn both_sides_use_the_same_prefix_and_length() {
    let bytes = b"pin the shape, not just one sample";
    let soko_addr = address(bytes);
    let kotva_id = kotva_core::ContentId::of(bytes);

    assert_eq!(
        soko_addr.0.len(),
        33,
        "soko address is prefix + 32-byte digest"
    );
    assert_eq!(
        kotva_id.as_bytes().len(),
        33,
        "kotva ContentId is prefix + 32-byte digest"
    );
    assert_eq!(soko_addr.0[0], 0x1e, "soko's BLAKE3_PREFIX constant");
    assert_eq!(
        kotva_id.as_bytes()[0],
        0x1e,
        "kotva's MH_BLAKE3_256 constant"
    );
    assert_eq!(
        kotva_id.algorithm(),
        Some(kotva_core::id::MH_BLAKE3_256),
        "kotva side self-reports the algorithm it actually used"
    );
}

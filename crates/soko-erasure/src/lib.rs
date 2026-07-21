//! # soko-erasure — data-subject rights, honestly modelled
//!
//! Access, erasure, retention and tombstones, for POPIA (South Africa), GDPR (EU) and LGPD
//! (Brazil). TRACT §22 states the conflict; this crate is the part that can actually be built.
//!
//! ## The thing most implementations get wrong
//!
//! **The right to erasure is not absolute, and a system that pretends otherwise either breaks tax
//! compliance or lies to the subject.** POPIA section 14 requires a responsible party to stop
//! retaining personal information once the purpose is served — *unless retention is required by
//! law*. Tax law then requires exactly that: invoice and order records must be kept for years
//! (five under South Africa's Tax Administration Act, and comparable periods across the EU).
//!
//! So a request to erase an order is frequently refused, lawfully, and the honest system says
//! **which obligation** it is refusing under and **when** the obligation lapses. A system that
//! silently deletes has broken the seller's tax position; one that silently refuses has
//! misinformed the subject. [`Outcome::Retained`] carries the basis so neither happens.
//!
//! ## The two quadrants get genuinely different answers
//!
//! | Quadrant | What erasure means | Why |
//! |---|---|---|
//! | **Sealed** (orders, addresses, contact) | actual deletion at both endpoints, or a stated retention basis | the data exists in two known places and nowhere else, so deleting it is meaningful |
//! | **Public** (reviews) | a tombstone, honoured cooperatively | content-addressed and irrevocable — no holder can be compelled, and pretending otherwise is the lie §22 warns about |
//!
//! This asymmetry is the whole reason TRACT keeps personal data out of the public quadrant
//! (§0.5.1). Everything published is permanently un-erasable, so the defence is to publish
//! nothing personal — not to build a deletion mechanism that cannot deliver.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use serde::{Deserialize, Serialize};
use soko_core::{ContentAddress, IdentityKey, Timestamp};

/// What a data subject asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Request {
    /// "What do you hold about me?" — POPIA section 23, GDPR Art 15.
    Access,
    /// "Delete it." — POPIA section 24, GDPR Art 17.
    Erase,
}

/// A statutory reason data must be kept despite an erasure request.
///
/// Named rather than free text, because "we need it for legal reasons" is not an answer a subject
/// can check. Each variant carries when the obligation lapses, so the refusal has an expiry rather
/// than being permanent by default.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetentionBasis {
    /// Tax and accounting records. South Africa's Tax Administration Act requires five years;
    /// EU member states commonly require six to ten. The holder states which regime binds it.
    TaxRecord {
        /// The jurisdiction whose retention rule applies.
        jurisdiction: soko_core::Country,
        /// When the obligation lapses and the record becomes erasable.
        until: Timestamp,
    },
    /// A live dispute, or an escrow ruling not yet final. Erasing evidence mid-dispute would
    /// disadvantage whichever party did not hold the copy.
    ActiveDispute {
        /// The order concerned.
        order: ContentAddress,
    },
    /// Consumer-protection or product-safety duties that outlive the transaction.
    LegalObligation {
        /// What obliges retention, in words a subject can look up.
        cites: String,
        /// When it lapses.
        until: Timestamp,
    },
}

/// What actually happened to a request.
///
/// Deliberately not a boolean. "Did it work?" cannot express the common and lawful case where
/// part of the data was deleted, part must be kept until a tax period closes, and part was
/// published irrevocably and can only be tombstoned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    /// Genuinely deleted. Sealed quadrant, no retention obligation.
    Erased {
        /// How many records were removed.
        records: u32,
    },
    /// Lawfully kept, with the reason and the date it stops applying.
    Retained {
        /// Why it was kept.
        basis: RetentionBasis,
        /// How many records this covers.
        records: u32,
    },
    /// Published and irrevocable. A tombstone was issued and conformant holders will stop serving
    /// it — but no holder can be compelled, so this is **not** deletion and does not claim to be.
    Tombstoned {
        /// The superseding marker.
        tombstone: Tombstone,
    },
    /// Nothing matched the request.
    NothingHeld,
}

/// A published marker retracting an earlier public object.
///
/// Not a deletion. The original object remains fetchable from any holder that keeps it, which is
/// the residual §22 requires be disclosed to the author **before** they publish rather than after
/// they ask to erase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tombstone {
    /// The object being retracted.
    pub supersedes: ContentAddress,
    /// The subkey that published the original, which is the only key entitled to retract it.
    pub author: IdentityKey,
    /// When it was issued.
    pub at: Timestamp,
}

impl soko_core::public::Publishable for Tombstone {}

/// Everything a holder knows about one data subject, for an access request.
///
/// Split by quadrant because the two carry different promises: what is sealed can be deleted on
/// request, and what is public cannot. A subject reading an access report is entitled to know
/// which of their data is permanent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessReport {
    /// Sealed records — deletable, subject to any retention basis.
    pub sealed: Vec<ContentAddress>,
    /// Public objects authored by this subject. **Permanent.**
    pub public: Vec<ContentAddress>,
    /// Retention obligations currently blocking erasure of some sealed records.
    pub retained_under: Vec<RetentionBasis>,
}

impl AccessReport {
    /// Whether anything here can never be erased, whatever the subject asks.
    ///
    /// An access report that does not distinguish permanent from deletable data is misleading in
    /// the way that matters most: the subject cannot tell which of it they still control.
    pub fn has_permanent_data(&self) -> bool {
        !self.public.is_empty()
    }
}

/// A holder of personal data that can answer subject requests.
///
/// This is a seam, not an implementation: where records live is a deployment question (a node's
/// local store, a gateway's database), and this crate has no opinion about it. What it fixes is
/// the *shape of the answer*, so a conformant holder cannot report "deleted" for something it kept
/// or "kept" for something it published irrevocably.
pub trait SubjectRights {
    /// The error type a holder's storage layer can produce.
    type Error;

    /// Everything held about `subject`.
    fn access(&self, subject: &IdentityKey) -> Result<AccessReport, Self::Error>;

    /// Erase what can lawfully be erased, and report precisely what happened to the rest.
    ///
    /// Implementations return one [`Outcome`] per distinct disposition rather than a single
    /// summary, because a real request usually produces several at once: some deleted, some held
    /// until a tax period closes, some already public.
    fn erase(&mut self, subject: &IdentityKey, at: Timestamp) -> Result<Vec<Outcome>, Self::Error>;
}

/// Whether a retention basis still applies at `now`.
///
/// A refusal without an expiry is a permanent refusal wearing a temporary costume, so every
/// time-bounded basis is checked rather than assumed still live. [`RetentionBasis::ActiveDispute`]
/// has no date because it ends when the dispute does, not on a calendar.
pub fn still_binding(basis: &RetentionBasis, now: Timestamp) -> bool {
    match basis {
        RetentionBasis::TaxRecord { until, .. } | RetentionBasis::LegalObligation { until, .. } => {
            now.0 < until.0
        }
        RetentionBasis::ActiveDispute { .. } => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soko_core::Country;

    fn addr(n: u8) -> ContentAddress {
        ContentAddress(vec![n])
    }
    fn key(n: u8) -> IdentityKey {
        IdentityKey(vec![n])
    }

    /// The case a naive implementation gets wrong: an erasure request against an order inside its
    /// tax-retention window is lawfully refused, and the refusal has to name the obligation and
    /// its expiry. Silently deleting breaks the seller's tax position; silently refusing
    /// misinforms the subject.
    #[test]
    fn tax_retention_blocks_erasure_and_says_why_and_until_when() {
        let basis = RetentionBasis::TaxRecord {
            jurisdiction: Country(*b"ZA"),
            until: Timestamp(2_000_000_000_000),
        };
        assert!(still_binding(&basis, Timestamp(1_800_000_000_000)));
        let outcome = Outcome::Retained {
            basis: basis.clone(),
            records: 1,
        };
        // the refusal is legible: a subject can read which regime and when it lapses
        match outcome {
            Outcome::Retained {
                basis:
                    RetentionBasis::TaxRecord {
                        jurisdiction,
                        until,
                    },
                ..
            } => {
                assert_eq!(jurisdiction, Country(*b"ZA"));
                assert_eq!(until, Timestamp(2_000_000_000_000));
            }
            _ => panic!("a retention refusal must carry its basis"),
        }
    }

    /// Once the obligation lapses the data becomes erasable. A basis with no expiry check is a
    /// permanent refusal that merely looks temporary.
    #[test]
    fn retention_stops_binding_once_the_period_closes() {
        let basis = RetentionBasis::TaxRecord {
            jurisdiction: Country(*b"ZA"),
            until: Timestamp(2_000_000_000_000),
        };
        assert!(!still_binding(&basis, Timestamp(2_000_000_000_001)));
    }

    /// A dispute ends when it ends, not on a date, so it stays binding regardless of the clock.
    #[test]
    fn an_active_dispute_binds_without_an_expiry_date() {
        let basis = RetentionBasis::ActiveDispute { order: addr(1) };
        assert!(still_binding(&basis, Timestamp(i64::MAX)));
    }

    /// The distinction the whole crate exists to keep: a tombstone is not an erasure, and the type
    /// system should make it impossible to report one as the other.
    #[test]
    fn a_tombstone_is_not_an_erasure() {
        let t = Outcome::Tombstoned {
            tombstone: Tombstone {
                supersedes: addr(9),
                author: key(1),
                at: Timestamp(0),
            },
        };
        assert!(!matches!(t, Outcome::Erased { .. }));
    }

    /// An access report that does not flag permanent data is misleading in exactly the way that
    /// matters — the subject cannot tell which of their data they still control.
    #[test]
    fn access_report_flags_data_that_can_never_be_erased() {
        let deletable = AccessReport {
            sealed: vec![addr(1)],
            ..Default::default()
        };
        assert!(!deletable.has_permanent_data());

        let permanent = AccessReport {
            sealed: vec![addr(1)],
            public: vec![addr(2)], // a published review
            ..Default::default()
        };
        assert!(permanent.has_permanent_data());
    }

    #[test]
    fn nothing_held_is_distinguishable_from_erased() {
        assert_ne!(Outcome::NothingHeld, Outcome::Erased { records: 0 });
    }
}

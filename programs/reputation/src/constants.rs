use anchor_lang::prelude::*;

#[constant]
pub const PROFILE_SEED: &[u8] = b"profile";

#[constant]
pub const RATING_SEED: &[u8] = b"rating";

#[constant]
pub const BADGE_SEED: &[u8] = b"badge";

/// Seed for the escrow program's CPI-signer PDA. Only a signature derived
/// from this seed under `ESCROW_PROGRAM_ID` is accepted as the trusted
/// caller for settlement-triggered instructions.
#[constant]
pub const ESCROW_AUTHORITY_SEED: &[u8] = b"escrow_authority";

/// The escrow program's deployed address. Hardcoded (not a crate dependency)
/// so the reputation program can verify CPI callers without depending on
/// the escrow crate.
///
/// ponytail: same value is hardcoded in `gig::constants::ESCROW_PROGRAM_ID`.
/// Redeploying escrow to a new address requires redeploying gig + reputation
/// in lockstep, or every escrow-only instruction starts rejecting valid
/// callers with no clear error signal. Move to a shared registry account if
/// independent upgrades are ever needed.
pub const ESCROW_PROGRAM_ID: Pubkey = pubkey!("7cvNPW6wKmQLVaygQi1cwrSHz1GNLtzQsurENYyfAkd4");

pub const MIN_RATING: u8 = 1;
pub const MAX_RATING: u8 = 5;

/// `average_rating` is stored scaled by 100 (e.g. 450 == 4.50 stars).
pub const RATING_SCALE: u64 = 100;

/// Completed-job count at which the volume component of the reputation
/// score saturates.
pub const VOLUME_SCORE_CAP: u64 = 100;

/// Lifetime earnings (in mint base units) at which the earnings component
/// of the reputation score saturates.
pub const EARNINGS_SCORE_CAP: u64 = 1_000_000_000;

pub const CANCELLATION_PENALTY_PER_JOB: u64 = 5;
pub const MAX_REPUTATION_SCORE: u64 = 1000;

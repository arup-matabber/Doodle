use anchor_lang::prelude::*;

#[constant]
pub const GIG_SEED: &[u8] = b"gig";

pub const MAX_TITLE_LEN: usize = 100;
pub const MAX_DESCRIPTION_LEN: usize = 500;
pub const MAX_SKILLS_LEN: usize = 200;
pub const MAX_CATEGORY_LEN: usize = 50;

pub const MIN_DEADLINE_SECS: i64 = 86_400; // 1 day from now minimum deadline

/// Seed for the escrow program's CPI-signer PDA. Only a signature derived from
/// this seed under `ESCROW_PROGRAM_ID` is accepted by the escrow-only instructions.
#[constant]
pub const ESCROW_AUTHORITY_SEED: &[u8] = b"escrow_authority";

/// The escrow program's deployed address. Hardcoded (not a crate dependency) so the
/// gig program can verify CPI callers without depending on the escrow crate.
///
/// ponytail: redeploying escrow to a new address silently breaks every
/// escrow-only instruction here (and in `reputation`, which hardcodes the
/// same value). Redeploy gig + reputation together whenever this changes,
/// or move to a shared registry account if independent upgrades matter.
pub const ESCROW_PROGRAM_ID: Pubkey = pubkey!("7cvNPW6wKmQLVaygQi1cwrSHz1GNLtzQsurENYyfAkd4");

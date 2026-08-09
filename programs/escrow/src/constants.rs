use anchor_lang::prelude::*;

#[constant]
pub const MILESTONE_SEED: &[u8] = b"milestone";

#[constant]
pub const VAULT_SEED: &[u8] = b"vault";

/// Seed for escrow's CPI-signer PDA, used to authorize calls into the gig program.
#[constant]
pub const ESCROW_AUTHORITY_SEED: &[u8] = b"escrow_authority";

pub const SECONDS_PER_DAY: i64 = 86_400;

pub const PARTIAL_RELEASE_PERCENT: u64 = 20;
pub const FULL_RELEASE_PERCENT: u64 = 80;

pub const PARTIAL_TIMEOUT: i64 = 72 * 3_600; // 72 hours
pub const FULL_TIMEOUT: i64 = 7 * SECONDS_PER_DAY; // 7 days

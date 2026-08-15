use anchor_lang::prelude::*;

use crate::constants::PROFILE_SEED;
use crate::state::UserProfile;

#[derive(Accounts)]
pub struct GetProfile<'info> {
    #[account(
        seeds = [PROFILE_SEED, profile.authority.as_ref()],
        bump = profile.bump,
    )]
    pub profile: Account<'info, UserProfile>,
}

/// Read-only helper that surfaces a profile's reputation score via return
/// data, so other programs can fetch it through a CPI instead of parsing
/// the account manually. Off-chain clients should just deserialize the
/// PDA directly; this exists for on-chain composability.
pub fn handler(ctx: Context<GetProfile>) -> Result<u64> {
    Ok(ctx.accounts.profile.reputation_score)
}

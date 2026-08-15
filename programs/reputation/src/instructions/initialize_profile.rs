use anchor_lang::prelude::*;

use crate::constants::PROFILE_SEED;
use crate::events::ProfileCreated;
use crate::state::UserProfile;

#[derive(Accounts)]
pub struct InitializeProfile<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = UserProfile::INIT_SPACE,
        seeds = [PROFILE_SEED, authority.key().as_ref()],
        bump,
    )]
    pub profile: Account<'info, UserProfile>,

    pub system_program: Program<'info, System>,
}

/// Creates a profile PDA for `authority`. The PDA's `init` constraint
/// already rejects a second call for the same authority, so no separate
/// existence check is needed here.
pub fn handler(ctx: Context<InitializeProfile>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let profile = &mut ctx.accounts.profile;

    profile.authority = ctx.accounts.authority.key();
    profile.completed_jobs = 0;
    profile.successful_jobs = 0;
    profile.cancelled_jobs = 0;
    profile.total_earnings = 0;
    profile.rating_sum = 0;
    profile.rating_count = 0;
    profile.average_rating = 0;
    profile.reputation_score = 0;
    profile.badges_earned = 0;
    profile.created_at = now;
    profile.updated_at = now;
    profile.bump = ctx.bumps.profile;

    emit!(ProfileCreated {
        profile: profile.key(),
        authority: profile.authority,
        created_at: now,
    });

    Ok(())
}

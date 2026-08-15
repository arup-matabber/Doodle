use anchor_lang::prelude::*;

use crate::constants::{ESCROW_AUTHORITY_SEED, ESCROW_PROGRAM_ID, PROFILE_SEED};
use crate::events::CompletionUpdated;
use crate::state::UserProfile;
use crate::utils::{checked_add, compute_reputation_score};

#[derive(Accounts)]
pub struct UpdateCompletion<'info> {
    /// Callable only via CPI from escrow, signed by its `escrow_authority` PDA.
    /// A PDA has no private key, so this can never be satisfied by a direct,
    /// top-level transaction -- only escrow's own `invoke_signed` can produce
    /// a valid signature for it.
    #[account(
        seeds = [ESCROW_AUTHORITY_SEED],
        bump,
        seeds::program = ESCROW_PROGRAM_ID,
    )]
    pub escrow_authority: Signer<'info>,

    #[account(
        mut,
        seeds = [PROFILE_SEED, profile.authority.as_ref()],
        bump = profile.bump,
    )]
    pub profile: Account<'info, UserProfile>,
}

/// Records the outcome of a completed job: increments `completed_jobs`
/// and either `successful_jobs` or `cancelled_jobs`, adds any earnings,
/// and recomputes the deterministic reputation score.
///
/// `earnings` is credited regardless of outcome, because it reports money the
/// freelancer was actually paid out of the vault -- a gig can end Cancelled
/// with earlier milestones already released, and dropping that credit would
/// understate lifetime earnings. The success/cancel split is the outcome
/// signal; `total_earnings` is the payment record.
pub fn handler(
    ctx: Context<UpdateCompletion>,
    successful: bool,
    earnings: u64,
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let profile = &mut ctx.accounts.profile;

    profile.completed_jobs = checked_add(profile.completed_jobs, 1)?;
    if successful {
        profile.successful_jobs = checked_add(profile.successful_jobs, 1)?;
    } else {
        profile.cancelled_jobs = checked_add(profile.cancelled_jobs, 1)?;
    }
    profile.total_earnings = checked_add(profile.total_earnings, earnings)?;

    profile.reputation_score = compute_reputation_score(profile)?;
    profile.updated_at = now;

    emit!(CompletionUpdated {
        profile: profile.key(),
        completed_jobs: profile.completed_jobs,
        successful_jobs: profile.successful_jobs,
        total_earnings: profile.total_earnings,
        reputation_score: profile.reputation_score,
    });

    Ok(())
}

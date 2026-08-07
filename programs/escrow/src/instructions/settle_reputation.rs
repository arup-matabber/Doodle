use anchor_lang::prelude::*;
use reputation::cpi::accounts::UpdateCompletion;
use reputation::UserProfile;

use crate::constants::{ESCROW_AUTHORITY_SEED, VAULT_SEED};
use crate::errors::EscrowError;
use crate::state::EscrowVault;
use gig::{Gig, GigStatus};

#[derive(Accounts)]
pub struct SettleReputation<'info> {
    pub gig: Account<'info, Gig>,

    #[account(
        mut,
        seeds = [VAULT_SEED, gig.key().as_ref()],
        bump = vault.bump,
        constraint = vault.gig == gig.key() @ EscrowError::Unauthorized,
        // Every milestone ever created must be accounted for -- either fully
        // released or cancelled before funding. Counting only `active_milestone`
        // meant a single cancelled milestone left this constraint permanently
        // unsatisfiable, silently blocking settlement and rating for the gig.
        constraint = vault.milestone_count > 0
            && vault.active_milestone.saturating_add(vault.cancelled_milestones)
                >= vault.milestone_count
            @ EscrowError::InvalidStatus,
        constraint = !vault.reputation_synced @ EscrowError::InvalidStatus,
    )]
    pub vault: Account<'info, EscrowVault>,

    /// CHECK: PDA identity only; used purely as escrow's CPI-signer into the reputation program.
    #[account(seeds = [ESCROW_AUTHORITY_SEED], bump)]
    pub escrow_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [reputation::PROFILE_SEED, gig.freelancer.as_ref()],
        bump = freelancer_profile.bump,
        seeds::program = reputation::ID,
    )]
    pub freelancer_profile: Account<'info, UserProfile>,

    pub reputation_program: Program<'info, reputation::program::Reputation>,
}

/// Permissionlessly notifies the Reputation Program once every milestone in
/// this gig has been accounted for -- escrow's own state is the sole trigger,
/// and `vault.reputation_synced` ensures this CPI (and its earnings credit) can
/// only ever fire once per gig.
///
/// The success flag is derived from the gig's own terminal status rather than
/// hardcoded. Passing `true` unconditionally made `cancelled_jobs` unreachable
/// from every code path, which in turn pinned `success_rate` at 100 for every
/// profile and made the cancellation penalty in `compute_reputation_score`
/// dead code.
pub fn handler(ctx: Context<SettleReputation>) -> Result<()> {
    let authority_bump = ctx.bumps.escrow_authority;
    let authority_signer_seeds: &[&[&[u8]]] = &[&[ESCROW_AUTHORITY_SEED, &[authority_bump]]];

    let earnings = ctx.accounts.vault.total_released;
    let successful = ctx.accounts.gig.status == GigStatus::Completed;

    reputation::cpi::update_completion(
        CpiContext::new_with_signer(
            ctx.accounts.reputation_program.key(),
            UpdateCompletion {
                escrow_authority: ctx.accounts.escrow_authority.to_account_info(),
                profile: ctx.accounts.freelancer_profile.to_account_info(),
            },
            authority_signer_seeds,
        ),
        successful,
        earnings,
    )?;

    ctx.accounts.vault.reputation_synced = true;

    Ok(())
}

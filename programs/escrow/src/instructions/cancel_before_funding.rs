use anchor_lang::prelude::*;
use gig::cpi::accounts::MarkCancelledByEscrow;
use gig::{Gig, GigStatus};

use crate::constants::{ESCROW_AUTHORITY_SEED, VAULT_SEED};
use crate::errors::EscrowError;
use crate::events::MilestoneCancelledBeforeFunding;
use crate::state::{EscrowVault, Milestone, MilestoneStatus};
use crate::utils::checked_add;

#[derive(Accounts)]
pub struct CancelBeforeFunding<'info> {
    #[account(mut)]
    pub client: Signer<'info>,

    #[account(mut, has_one = client @ EscrowError::Unauthorized)]
    pub gig: Account<'info, Gig>,

    #[account(
        mut,
        close = client,
        constraint = milestone.gig == gig.key() @ EscrowError::Unauthorized,
        constraint = milestone.status == MilestoneStatus::PendingFunding @ EscrowError::AlreadyFunded,
    )]
    pub milestone: Account<'info, Milestone>,

    #[account(
        mut,
        seeds = [VAULT_SEED, gig.key().as_ref()],
        bump = vault.bump,
        constraint = vault.gig == gig.key() @ EscrowError::Unauthorized,
    )]
    pub vault: Account<'info, EscrowVault>,

    /// CHECK: PDA identity only; used purely as escrow's CPI-signer into the gig program.
    #[account(seeds = [ESCROW_AUTHORITY_SEED], bump)]
    pub escrow_authority: UncheckedAccount<'info>,

    pub gig_program: Program<'info, gig::program::Gig>,
}

/// Closes a milestone that was never funded and refunds its rent to the client.
///
/// The gig itself is cancelled via CPI only when no money has ever entered the
/// vault. Cancelling unconditionally meant that closing one unfunded milestone
/// tore down a gig whose other milestones were funded and in flight; it also
/// left `milestone_count` unreachable by `active_milestone`, which permanently
/// blocked `settle_reputation` (and therefore `rate_freelancer`) for that gig.
pub fn handler(ctx: Context<CancelBeforeFunding>) -> Result<()> {
    emit!(MilestoneCancelledBeforeFunding {
        gig: ctx.accounts.gig.key(),
        milestone: ctx.accounts.milestone.key(),
        index: ctx.accounts.milestone.index,
    });

    let vault = &mut ctx.accounts.vault;
    vault.cancelled_milestones = checked_add(vault.cancelled_milestones as u64, 1)? as u32;

    // Only unwind the gig if nothing was ever locked. A gig that has taken
    // funds stays alive so its remaining milestones can still settle. The
    // status guard keeps repeat cancellations from failing at the CPI once
    // the gig has already been moved to Cancelled.
    let should_cancel_gig = vault.total_locked == 0
        && (ctx.accounts.gig.status == GigStatus::Assigned
            || ctx.accounts.gig.status == GigStatus::InProgress);

    if should_cancel_gig {
        let authority_bump = ctx.bumps.escrow_authority;
        let authority_signer_seeds: &[&[&[u8]]] = &[&[ESCROW_AUTHORITY_SEED, &[authority_bump]]];

        gig::cpi::mark_cancelled_by_escrow(CpiContext::new_with_signer(
            ctx.accounts.gig_program.key(),
            MarkCancelledByEscrow {
                escrow_authority: ctx.accounts.escrow_authority.to_account_info(),
                gig: ctx.accounts.gig.to_account_info(),
            },
            authority_signer_seeds,
        ))?;
    }

    Ok(())
}

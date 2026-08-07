use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Mint, TransferChecked};
use gig::cpi::accounts::MarkCompletedByEscrow;
use gig::Gig;

use crate::constants::{ESCROW_AUTHORITY_SEED, FULL_TIMEOUT, VAULT_SEED};
use crate::errors::EscrowError;
use crate::events::FullReleaseExecuted;
use crate::state::{EscrowVault, Milestone, MilestoneStatus};
use crate::utils::{checked_add, checked_sub};

#[derive(Accounts)]
pub struct FullTimeoutRelease<'info> {
    #[account(mut)]
    pub gig: Account<'info, Gig>,

    #[account(
        mut,
        constraint = milestone.gig == gig.key() @ EscrowError::Unauthorized,
        // Both `Submitted` and `PartialReleased` are accepted so the 7-day
        // guarantee stands on its own. Requiring `PartialReleased` made the
        // full release depend on the 72h partial having succeeded first, which
        // is not always possible: for a milestone small enough that 20% rounds
        // down to zero, `partial_timeout_release` rejects, and the funds were
        // then unreachable by any path.
        constraint = milestone.status == MilestoneStatus::Submitted
            || milestone.status == MilestoneStatus::PartialReleased
            @ EscrowError::InvalidStatus,
    )]
    pub milestone: Account<'info, Milestone>,

    #[account(
        mut,
        seeds = [VAULT_SEED, gig.key().as_ref()],
        bump = vault.bump,
        has_one = mint @ EscrowError::InvalidMint,
    )]
    pub vault: Account<'info, EscrowVault>,

    #[account(mut, address = vault.token_account @ EscrowError::Unauthorized)]
    pub vault_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = freelancer_token_account.mint == mint.key() @ EscrowError::InvalidMint,
        constraint = freelancer_token_account.owner == gig.freelancer @ EscrowError::Unauthorized,
    )]
    pub freelancer_token_account: Account<'info, TokenAccount>,

    pub mint: Account<'info, Mint>,

    /// CHECK: PDA identity only; used purely as escrow's CPI-signer into the gig program.
    #[account(seeds = [ESCROW_AUTHORITY_SEED], bump)]
    pub escrow_authority: UncheckedAccount<'info>,

    pub gig_program: Program<'info, gig::program::Gig>,

    pub token_program: Program<'info, Token>,
}

/// Permissionlessly releases the remaining balance once 7 days have elapsed
/// since delivery submission without client approval. CPIs into the gig program
/// to mark it Completed once the last milestone clears.
pub fn handler(ctx: Context<FullTimeoutRelease>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let milestone = &ctx.accounts.milestone;

    require!(
        now >= milestone.submitted_at.saturating_add(FULL_TIMEOUT),
        EscrowError::TimeoutNotReached
    );

    let remaining = checked_sub(milestone.amount, milestone.released)?;
    require!(remaining > 0, EscrowError::InsufficientFunds);

    let gig_key = ctx.accounts.gig.key();
    let vault_bump = ctx.accounts.vault.bump;
    let vault_signer_seeds: &[&[&[u8]]] = &[&[VAULT_SEED, gig_key.as_ref(), &[vault_bump]]];

    token::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            TransferChecked {
                from: ctx.accounts.vault_token_account.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.freelancer_token_account.to_account_info(),
                authority: ctx.accounts.vault.to_account_info(),
            },
            vault_signer_seeds,
        ),
        remaining,
        ctx.accounts.mint.decimals,
    )?;

    let vault = &mut ctx.accounts.vault;
    vault.total_released = checked_add(vault.total_released, remaining)?;

    let milestone = &mut ctx.accounts.milestone;
    milestone.released = checked_add(milestone.released, remaining)?;
    milestone.status = MilestoneStatus::Completed;

    // Mirrors `approve_milestone`: cancelled milestones are accounted for, so a
    // gig with a cancelled milestone can still reach Completed.
    let is_last_milestone = checked_add(
        checked_add(vault.active_milestone as u64, 1)?,
        vault.cancelled_milestones as u64,
    )? as u32
        >= vault.milestone_count;
    vault.active_milestone = checked_add(vault.active_milestone as u64, 1)? as u32;

    emit!(FullReleaseExecuted {
        gig: gig_key,
        milestone: milestone.key(),
        amount_released: remaining,
    });

    if is_last_milestone {
        let authority_bump = ctx.bumps.escrow_authority;
        let authority_signer_seeds: &[&[&[u8]]] = &[&[ESCROW_AUTHORITY_SEED, &[authority_bump]]];

        gig::cpi::mark_completed_by_escrow(CpiContext::new_with_signer(
            ctx.accounts.gig_program.key(),
            MarkCompletedByEscrow {
                escrow_authority: ctx.accounts.escrow_authority.to_account_info(),
                gig: ctx.accounts.gig.to_account_info(),
            },
            authority_signer_seeds,
        ))?;
    }

    Ok(())
}

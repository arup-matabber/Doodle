use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Mint, TransferChecked};
use gig::cpi::accounts::MarkCompletedByEscrow;
use gig::Gig;

use crate::constants::{ESCROW_AUTHORITY_SEED, VAULT_SEED};
use crate::errors::EscrowError;
use crate::events::MilestoneApproved;
use crate::state::{EscrowVault, Milestone, MilestoneStatus};
use crate::utils::{checked_add, checked_sub};

#[derive(Accounts)]
pub struct ApproveMilestone<'info> {
    pub client: Signer<'info>,

    #[account(mut, has_one = client @ EscrowError::Unauthorized)]
    pub gig: Account<'info, Gig>,

    #[account(
        mut,
        constraint = milestone.gig == gig.key() @ EscrowError::Unauthorized,
        // `PartialReleased` is accepted alongside `Submitted` so the client can
        // still approve (and pay the remaining balance) after the permissionless
        // 72h partial has fired. Gating on `Submitted` alone meant a single
        // partial release permanently removed the client's only payment path
        // until the 7-day full timeout.
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

    /// CHECK: destination ownership validated via `freelancer_token_account.owner` constraint below.
    #[account(address = gig.freelancer @ EscrowError::Unauthorized)]
    pub freelancer: UncheckedAccount<'info>,

    #[account(
        mut,
        constraint = freelancer_token_account.mint == mint.key() @ EscrowError::InvalidMint,
        constraint = freelancer_token_account.owner == freelancer.key() @ EscrowError::Unauthorized,
    )]
    pub freelancer_token_account: Account<'info, TokenAccount>,

    pub mint: Account<'info, Mint>,

    /// CHECK: PDA identity only; used purely as escrow's CPI-signer into the gig program.
    #[account(seeds = [ESCROW_AUTHORITY_SEED], bump)]
    pub escrow_authority: UncheckedAccount<'info>,

    pub gig_program: Program<'info, gig::program::Gig>,

    pub token_program: Program<'info, Token>,
}

/// Releases the remaining balance of an approved milestone to the freelancer.
/// CPIs into the gig program to mark it Completed once the last milestone clears.
pub fn handler(ctx: Context<ApproveMilestone>) -> Result<()> {
    let milestone = &ctx.accounts.milestone;
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

    let now = Clock::get()?.unix_timestamp;

    let vault = &mut ctx.accounts.vault;
    vault.total_released = checked_add(vault.total_released, remaining)?;

    let milestone = &mut ctx.accounts.milestone;
    milestone.released = checked_add(milestone.released, remaining)?;
    milestone.approved_at = now;
    milestone.status = MilestoneStatus::Completed;

    // Cancelled milestones count toward "everything is accounted for" -- they
    // never increment `active_milestone`, so leaving them out meant a gig with
    // any cancelled milestone could never reach Completed.
    let is_last_milestone = checked_add(
        checked_add(vault.active_milestone as u64, 1)?,
        vault.cancelled_milestones as u64,
    )? as u32
        >= vault.milestone_count;
    vault.active_milestone = checked_add(vault.active_milestone as u64, 1)? as u32;

    emit!(MilestoneApproved {
        gig: gig_key,
        milestone: milestone.key(),
        amount_released: remaining,
        approved_at: now,
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

use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Mint, TransferChecked};
use gig::Gig;

use crate::constants::{PARTIAL_RELEASE_PERCENT, PARTIAL_TIMEOUT, VAULT_SEED};
use crate::errors::EscrowError;
use crate::events::PartialReleaseExecuted;
use crate::state::{EscrowVault, Milestone, MilestoneStatus};
use crate::utils::{checked_add, percent_of};

#[derive(Accounts)]
pub struct PartialTimeoutRelease<'info> {
    pub gig: Account<'info, Gig>,

    #[account(
        mut,
        constraint = milestone.gig == gig.key() @ EscrowError::Unauthorized,
        constraint = milestone.status == MilestoneStatus::Submitted @ EscrowError::InvalidStatus,
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

    pub token_program: Program<'info, Token>,
}

/// Permissionlessly releases 20% of a milestone once 72 hours have elapsed
/// since delivery submission without client action.
pub fn handler(ctx: Context<PartialTimeoutRelease>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let milestone = &ctx.accounts.milestone;

    require!(
        now >= milestone.submitted_at.saturating_add(PARTIAL_TIMEOUT),
        EscrowError::TimeoutNotReached
    );

    let release_amount = percent_of(milestone.amount, PARTIAL_RELEASE_PERCENT)?;
    require!(release_amount > 0, EscrowError::InsufficientFunds);

    let gig_key = ctx.accounts.gig.key();
    let vault_bump = ctx.accounts.vault.bump;
    let signer_seeds: &[&[&[u8]]] = &[&[VAULT_SEED, gig_key.as_ref(), &[vault_bump]]];

    token::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            TransferChecked {
                from: ctx.accounts.vault_token_account.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.freelancer_token_account.to_account_info(),
                authority: ctx.accounts.vault.to_account_info(),
            },
            signer_seeds,
        ),
        release_amount,
        ctx.accounts.mint.decimals,
    )?;

    let vault = &mut ctx.accounts.vault;
    vault.total_released = checked_add(vault.total_released, release_amount)?;

    let milestone = &mut ctx.accounts.milestone;
    milestone.released = checked_add(milestone.released, release_amount)?;
    milestone.status = MilestoneStatus::PartialReleased;

    emit!(PartialReleaseExecuted {
        gig: gig_key,
        milestone: milestone.key(),
        amount_released: release_amount,
    });

    Ok(())
}

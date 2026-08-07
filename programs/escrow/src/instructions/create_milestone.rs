use anchor_lang::prelude::*;
use gig::{Gig, GigStatus};

use crate::constants::{MILESTONE_SEED, VAULT_SEED};
use crate::errors::EscrowError;
use crate::events::MilestoneCreated;
use crate::state::{EscrowVault, Milestone, MilestoneStatus};
use crate::utils::checked_add;

#[derive(Accounts)]
pub struct CreateMilestone<'info> {
    #[account(mut)]
    pub client: Signer<'info>,

    /// Ownership (Gig program), PDA derivation, and freshness are all checked by
    /// the `seeds`/`seeds::program` constraint below, which re-derives the Gig
    /// PDA from its own `id`/`bump` and rejects any account that doesn't match.
    #[account(
        has_one = client @ EscrowError::Unauthorized,
        seeds = [gig::GIG_SEED, gig.client.as_ref(), gig.id.to_le_bytes().as_ref()],
        bump = gig.bump,
        seeds::program = gig::ID,
        constraint = gig.status == GigStatus::Assigned || gig.status == GigStatus::InProgress
            @ EscrowError::GigNotFundable,
    )]
    pub gig: Account<'info, Gig>,

    #[account(
        init_if_needed,
        payer = client,
        space = EscrowVault::INIT_SPACE,
        seeds = [VAULT_SEED, gig.key().as_ref()],
        bump,
    )]
    pub vault: Account<'info, EscrowVault>,

    #[account(
        init,
        payer = client,
        space = Milestone::INIT_SPACE,
        seeds = [MILESTONE_SEED, gig.key().as_ref(), vault.milestone_count.to_le_bytes().as_ref()],
        bump,
    )]
    pub milestone: Account<'info, Milestone>,

    pub system_program: Program<'info, System>,
}

/// Creates the next sequential Milestone PDA for `gig`, awaiting funding.
pub fn handler(ctx: Context<CreateMilestone>, amount: u64) -> Result<()> {
    require!(amount > 0, EscrowError::InvalidAmount);

    let vault = &mut ctx.accounts.vault;
    if vault.gig == Pubkey::default() {
        vault.gig = ctx.accounts.gig.key();
        vault.bump = ctx.bumps.vault;
    }
    let index = vault.milestone_count;

    let milestone = &mut ctx.accounts.milestone;
    milestone.gig = ctx.accounts.gig.key();
    milestone.index = index;
    milestone.amount = amount;
    milestone.released = 0;
    milestone.status = MilestoneStatus::PendingFunding;
    milestone.submitted_at = 0;
    milestone.approved_at = 0;
    milestone.bump = ctx.bumps.milestone;

    vault.milestone_count = checked_add(vault.milestone_count as u64, 1)? as u32;

    emit!(MilestoneCreated {
        gig: ctx.accounts.gig.key(),
        milestone: milestone.key(),
        index,
        amount,
    });

    Ok(())
}

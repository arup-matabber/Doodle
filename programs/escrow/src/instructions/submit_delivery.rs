use anchor_lang::prelude::*;
use gig::Gig;

use crate::errors::EscrowError;
use crate::events::DeliverySubmitted;
use crate::state::{Milestone, MilestoneStatus};

#[derive(Accounts)]
pub struct SubmitDelivery<'info> {
    pub freelancer: Signer<'info>,

    #[account(has_one = freelancer @ EscrowError::Unauthorized)]
    pub gig: Account<'info, Gig>,

    #[account(
        mut,
        constraint = milestone.gig == gig.key() @ EscrowError::Unauthorized,
    )]
    pub milestone: Account<'info, Milestone>,
}

/// Records the freelancer's delivery submission and starts the timeout clocks.
pub fn handler(ctx: Context<SubmitDelivery>) -> Result<()> {
    require!(
        ctx.accounts.milestone.status != MilestoneStatus::Submitted,
        EscrowError::MilestoneAlreadySubmitted
    );
    require!(
        ctx.accounts.milestone.status == MilestoneStatus::Funded,
        EscrowError::InvalidStatus
    );

    let now = Clock::get()?.unix_timestamp;

    let milestone = &mut ctx.accounts.milestone;
    milestone.submitted_at = now;
    milestone.status = MilestoneStatus::Submitted;

    emit!(DeliverySubmitted {
        gig: ctx.accounts.gig.key(),
        milestone: milestone.key(),
        submitted_at: now,
    });

    Ok(())
}

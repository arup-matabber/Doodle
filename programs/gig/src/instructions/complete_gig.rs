use anchor_lang::prelude::*;

use crate::errors::GigError;
use crate::events::GigCompleted;
use crate::state::{Gig, GigStatus};

/// Manual completion path for gigs that never entered escrow (no milestones funded).
/// Gigs that were funded transition to Completed only via escrow's CPI
/// (see `mark_completed_by_escrow`), never through this instruction.
#[derive(Accounts)]
pub struct CompleteGig<'info> {
    #[account(mut)]
    pub client: Signer<'info>,

    #[account(
        mut,
        has_one = client @ GigError::Unauthorized,
        constraint = gig.status == GigStatus::Assigned @ GigError::NotAssignedStatus,
    )]
    pub gig: Account<'info, Gig>,
}

pub fn handler(ctx: Context<CompleteGig>) -> Result<()> {
    let gig = &mut ctx.accounts.gig;
    gig.status = GigStatus::Completed;
    gig.updated_at = Clock::get()?.unix_timestamp;

    emit!(GigCompleted {
        gig: gig.key(),
        id: gig.id,
    });

    Ok(())
}

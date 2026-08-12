use anchor_lang::prelude::*;

use crate::constants::{ESCROW_AUTHORITY_SEED, ESCROW_PROGRAM_ID};
use crate::errors::GigError;
use crate::events::GigCancelled;
use crate::state::{Gig, GigStatus};

/// Callable only via CPI from escrow, signed by its `escrow_authority` PDA.
/// Used when escrow cancels a milestone before it was funded, or otherwise
/// unwinds a gig that was already InProgress.
#[derive(Accounts)]
pub struct MarkCancelledByEscrow<'info> {
    #[account(
        seeds = [ESCROW_AUTHORITY_SEED],
        bump,
        seeds::program = ESCROW_PROGRAM_ID,
    )]
    pub escrow_authority: Signer<'info>,

    #[account(
        mut,
        constraint = gig.status == GigStatus::Assigned || gig.status == GigStatus::InProgress
            @ GigError::InvalidStatus,
    )]
    pub gig: Account<'info, Gig>,
}

pub fn handler(ctx: Context<MarkCancelledByEscrow>) -> Result<()> {
    let gig = &mut ctx.accounts.gig;
    gig.status = GigStatus::Cancelled;
    gig.updated_at = Clock::get()?.unix_timestamp;

    emit!(GigCancelled {
        gig: gig.key(),
        id: gig.id,
    });

    Ok(())
}

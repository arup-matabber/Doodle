use anchor_lang::prelude::*;

use crate::constants::{ESCROW_AUTHORITY_SEED, ESCROW_PROGRAM_ID};
use crate::errors::GigError;
use crate::events::GigCompleted;
use crate::state::{Gig, GigStatus};

/// Callable only via CPI from escrow, signed by its `escrow_authority` PDA.
#[derive(Accounts)]
pub struct MarkCompletedByEscrow<'info> {
    #[account(
        seeds = [ESCROW_AUTHORITY_SEED],
        bump,
        seeds::program = ESCROW_PROGRAM_ID,
    )]
    pub escrow_authority: Signer<'info>,

    #[account(
        mut,
        constraint = gig.status == GigStatus::InProgress @ GigError::NotInProgressStatus,
    )]
    pub gig: Account<'info, Gig>,
}

/// Escrow calls this once the final milestone has been released.
pub fn handler(ctx: Context<MarkCompletedByEscrow>) -> Result<()> {
    let gig = &mut ctx.accounts.gig;
    gig.status = GigStatus::Completed;
    gig.updated_at = Clock::get()?.unix_timestamp;

    emit!(GigCompleted {
        gig: gig.key(),
        id: gig.id,
    });

    Ok(())
}

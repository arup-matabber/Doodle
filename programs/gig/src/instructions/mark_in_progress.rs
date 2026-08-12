use anchor_lang::prelude::*;

use crate::constants::{ESCROW_AUTHORITY_SEED, ESCROW_PROGRAM_ID};
use crate::errors::GigError;
use crate::events::GigInProgress;
use crate::state::{Gig, GigStatus};

/// Callable only via CPI from the escrow program, signed by escrow's own
/// `escrow_authority` PDA. The seeds::program constraint means the Solana
/// runtime itself rejects any signer whose PDA wasn't derived — and signed
/// for — by the escrow program, so no other caller can forge this signature.
#[derive(Accounts)]
pub struct MarkInProgress<'info> {
    #[account(
        seeds = [ESCROW_AUTHORITY_SEED],
        bump,
        seeds::program = ESCROW_PROGRAM_ID,
    )]
    pub escrow_authority: Signer<'info>,

    #[account(
        mut,
        constraint = gig.status == GigStatus::Assigned @ GigError::NotAssignedStatus,
    )]
    pub gig: Account<'info, Gig>,
}

/// Escrow calls this when the first milestone is funded, moving the gig
/// from Assigned into active work.
pub fn handler(ctx: Context<MarkInProgress>) -> Result<()> {
    let gig = &mut ctx.accounts.gig;
    gig.status = GigStatus::InProgress;
    gig.updated_at = Clock::get()?.unix_timestamp;

    emit!(GigInProgress {
        gig: gig.key(),
        id: gig.id,
    });

    Ok(())
}

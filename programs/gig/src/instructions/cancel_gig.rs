use anchor_lang::prelude::*;

use crate::errors::GigError;
use crate::events::GigCancelled;
use crate::state::{Gig, GigStatus};

/// Client-driven cancellation, available only before any milestone has been funded.
/// Once a gig is InProgress, cancellation must go through escrow
/// (see `mark_cancelled_by_escrow`), since escrow owns locked-fund state.
#[derive(Accounts)]
pub struct CancelGig<'info> {
    #[account(mut)]
    pub client: Signer<'info>,

    #[account(
        mut,
        has_one = client @ GigError::Unauthorized,
        constraint = gig.status == GigStatus::Draft
            || gig.status == GigStatus::Published
            || gig.status == GigStatus::Assigned
            @ GigError::InvalidStatus,
    )]
    pub gig: Account<'info, Gig>,
}

pub fn handler(ctx: Context<CancelGig>) -> Result<()> {
    let gig = &mut ctx.accounts.gig;
    gig.status = GigStatus::Cancelled;
    gig.updated_at = Clock::get()?.unix_timestamp;

    emit!(GigCancelled {
        gig: gig.key(),
        id: gig.id,
    });

    Ok(())
}

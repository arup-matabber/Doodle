use anchor_lang::prelude::*;

use crate::errors::GigError;
use crate::events::GigArchived;
use crate::state::{Gig, GigStatus};

#[derive(Accounts)]
pub struct ArchiveGig<'info> {
    #[account(mut)]
    pub client: Signer<'info>,

    #[account(
        mut,
        has_one = client @ GigError::Unauthorized,
        constraint = gig.status == GigStatus::Completed @ GigError::NotCompletedStatus,
    )]
    pub gig: Account<'info, Gig>,
}

pub fn handler(ctx: Context<ArchiveGig>) -> Result<()> {
    let gig = &mut ctx.accounts.gig;
    gig.status = GigStatus::Archived;
    gig.updated_at = Clock::get()?.unix_timestamp;

    emit!(GigArchived {
        gig: gig.key(),
        id: gig.id,
    });

    Ok(())
}

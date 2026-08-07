use anchor_lang::prelude::*;
use gig::{Gig, GigStatus};
use reputation::cpi::accounts::SubmitRating;
use reputation::UserProfile;

use crate::constants::{ESCROW_AUTHORITY_SEED, VAULT_SEED};
use crate::errors::EscrowError;
use crate::state::EscrowVault;

#[derive(Accounts)]
pub struct RateFreelancer<'info> {
    #[account(mut)]
    pub client: Signer<'info>,

    #[account(
        has_one = client @ EscrowError::Unauthorized,
        constraint = gig.status == GigStatus::Completed @ EscrowError::InvalidStatus,
    )]
    pub gig: Account<'info, Gig>,

    #[account(
        seeds = [VAULT_SEED, gig.key().as_ref()],
        bump = vault.bump,
        constraint = vault.gig == gig.key() @ EscrowError::Unauthorized,
        // A rating may only be recorded once earnings/completion have been
        // settled with the reputation program, so a Rating PDA can never
        // exist for a job whose UserProfile was never credited.
        constraint = vault.reputation_synced @ EscrowError::InvalidStatus,
    )]
    pub vault: Account<'info, EscrowVault>,

    /// CHECK: only used as the seed/reference the freelancer profile is derived from.
    #[account(address = gig.freelancer @ EscrowError::Unauthorized)]
    pub freelancer: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [reputation::PROFILE_SEED, freelancer.key().as_ref()],
        bump = freelancer_profile.bump,
        seeds::program = reputation::ID,
    )]
    pub freelancer_profile: Account<'info, UserProfile>,

    /// CHECK: validated by CPI into the reputation program (PDA seeds + init).
    #[account(mut)]
    pub rating: UncheckedAccount<'info>,

    /// CHECK: PDA identity only; used purely as escrow's CPI-signer into the reputation program.
    #[account(seeds = [ESCROW_AUTHORITY_SEED], bump)]
    pub escrow_authority: UncheckedAccount<'info>,

    pub reputation_program: Program<'info, reputation::program::Reputation>,

    pub system_program: Program<'info, System>,
}

/// Client submits the final 1-5 rating for a completed gig. Escrow attests
/// (via its CPI-signer PDA) that `gig.id` really is a completed job between
/// `client` and `freelancer`; the reputation program enforces the one-rating-
/// per-job and valid-range rules on its own end.
pub fn handler(ctx: Context<RateFreelancer>, score: u8, review_hash: [u8; 32]) -> Result<()> {
    let authority_bump = ctx.bumps.escrow_authority;
    let authority_signer_seeds: &[&[&[u8]]] = &[&[ESCROW_AUTHORITY_SEED, &[authority_bump]]];

    reputation::cpi::submit_rating(
        CpiContext::new_with_signer(
            ctx.accounts.reputation_program.key(),
            SubmitRating {
                client: ctx.accounts.client.to_account_info(),
                escrow_authority: ctx.accounts.escrow_authority.to_account_info(),
                freelancer: ctx.accounts.freelancer.to_account_info(),
                freelancer_profile: ctx.accounts.freelancer_profile.to_account_info(),
                rating: ctx.accounts.rating.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
            },
            authority_signer_seeds,
        ),
        ctx.accounts.gig.id,
        score,
        review_hash,
    )
}

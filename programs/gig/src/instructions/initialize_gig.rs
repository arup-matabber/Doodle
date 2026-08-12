use anchor_lang::prelude::*;
use anchor_spl::token::Mint;

use crate::constants::*;
use crate::errors::GigError;
use crate::events::GigCreated;
use crate::state::{Gig, GigStatus};

#[derive(Accounts)]
#[instruction(id: u64, title: String, description: String, category: String, budget: u64, deadline: i64)]
pub struct InitializeGig<'info> {
    #[account(mut)]
    pub client: Signer<'info>,

    pub mint: Account<'info, Mint>,

    #[account(
        init,
        payer = client,
        space = Gig::INIT_SPACE,
        // Scoped to the creating client. With `id` alone the gig address space
        // was global and caller-chosen, so anyone could pre-create a Gig at any
        // id and make a legitimate client's `initialize_gig` fail. `id` now only
        // has to be unique per client.
        seeds = [GIG_SEED, client.key().as_ref(), id.to_le_bytes().as_ref()],
        bump,
    )]
    pub gig: Account<'info, Gig>,

    pub system_program: Program<'info, System>,
}

/// Creates a new Gig PDA in Draft status with the given metadata.
pub fn handler(
    ctx: Context<InitializeGig>,
    id: u64,
    title: String,
    description: String,
    category: String,
    budget: u64,
    deadline: i64,
) -> Result<()> {
    require!(title.len() <= MAX_TITLE_LEN, GigError::TitleTooLong);
    require!(!title.is_empty(), GigError::TitleTooLong);
    require!(description.len() <= MAX_DESCRIPTION_LEN, GigError::DescriptionTooLong);
    require!(category.len() <= MAX_CATEGORY_LEN, GigError::CategoryTooLong);
    require!(budget > 0, GigError::InvalidBudget);

    let now = Clock::get()?.unix_timestamp;
    require!(deadline > now + MIN_DEADLINE_SECS, GigError::InvalidDeadline);

    let gig = &mut ctx.accounts.gig;
    gig.id = id;
    gig.client = ctx.accounts.client.key();
    gig.freelancer = Pubkey::default();
    gig.mint = ctx.accounts.mint.key();
    gig.status = GigStatus::Draft;
    gig.created_at = now;
    gig.updated_at = now;
    gig.title = title;
    gig.description = description;
    gig.skills = String::new();
    gig.category = category;
    gig.budget = budget;
    gig.deadline = deadline;
    gig.bump = ctx.bumps.gig;

    emit!(GigCreated {
        gig: gig.key(),
        id,
        client: gig.client,
        freelancer: gig.freelancer,
        created_at: now,
    });

    Ok(())
}

use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::GigError;
use crate::events::GigUpdated;
use crate::state::{Gig, GigStatus};

#[derive(Accounts)]
pub struct UpdateGig<'info> {
    #[account(mut)]
    pub client: Signer<'info>,

    #[account(
        mut,
        has_one = client @ GigError::Unauthorized,
        constraint = gig.status == GigStatus::Draft @ GigError::NotDraftStatus,
    )]
    pub gig: Account<'info, Gig>,
}

pub fn handler(
    ctx: Context<UpdateGig>,
    title: String,
    description: String,
    skills: String,
    category: String,
    budget: u64,
    deadline: i64,
) -> Result<()> {
    require!(title.len() <= MAX_TITLE_LEN, GigError::TitleTooLong);
    require!(!title.is_empty(), GigError::TitleTooLong);
    require!(description.len() <= MAX_DESCRIPTION_LEN, GigError::DescriptionTooLong);
    require!(skills.len() <= MAX_SKILLS_LEN, GigError::SkillsTooLong);
    require!(category.len() <= MAX_CATEGORY_LEN, GigError::CategoryTooLong);
    require!(budget > 0, GigError::InvalidBudget);

    let now = Clock::get()?.unix_timestamp;
    require!(deadline > now + MIN_DEADLINE_SECS, GigError::InvalidDeadline);

    let gig = &mut ctx.accounts.gig;
    gig.updated_at = now;
    gig.title = title;
    gig.description = description;
    gig.skills = skills;
    gig.category = category;
    gig.budget = budget;
    gig.deadline = deadline;

    emit!(GigUpdated {
        gig: gig.key(),
        id: gig.id,
        title: gig.title.clone(),
        description: gig.description.clone(),
        skills: gig.skills.clone(),
        category: gig.category.clone(),
        budget: gig.budget,
        deadline: gig.deadline,
    });

    Ok(())
}

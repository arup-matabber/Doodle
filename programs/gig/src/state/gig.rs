use anchor_lang::prelude::*;

use super::enums::GigStatus;
use crate::constants::*;

#[account]
pub struct Gig {
    pub id: u64,
    pub client: Pubkey,
    pub freelancer: Pubkey,
    pub mint: Pubkey,
    pub status: GigStatus,
    pub created_at: i64,
    pub updated_at: i64,
    pub title: String,
    pub description: String,
    pub skills: String,
    pub category: String,
    pub budget: u64,
    pub deadline: i64,
    pub bump: u8,
}

impl Gig {
    pub const INIT_SPACE: usize = 8 // discriminator
        + 8 // id
        + 32 // client
        + 32 // freelancer
        + 32 // mint
        + 1 // status
        + 8 // created_at
        + 8 // updated_at
        + 4 + MAX_TITLE_LEN // title
        + 4 + MAX_DESCRIPTION_LEN // description
        + 4 + MAX_SKILLS_LEN // skills
        + 4 + MAX_CATEGORY_LEN // category
        + 8 // budget
        + 8 // deadline
        + 1; // bump
}

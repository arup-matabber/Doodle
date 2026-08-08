use anchor_lang::prelude::*;

use super::enums::MilestoneStatus;

#[account]
pub struct Milestone {
    pub gig: Pubkey,
    pub index: u32,
    pub amount: u64,
    pub released: u64,
    pub status: MilestoneStatus,
    pub submitted_at: i64,
    pub approved_at: i64,
    pub bump: u8,
}

impl Milestone {
    pub const INIT_SPACE: usize = 8 // discriminator
        + 32 // gig
        + 4 // index
        + 8 // amount
        + 8 // released
        + 1 // status
        + 8 // submitted_at
        + 8 // approved_at
        + 1; // bump
}

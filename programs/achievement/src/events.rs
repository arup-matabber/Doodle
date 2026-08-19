use anchor_lang::prelude::*;

#[event]
pub struct AchievementClaimed {
    pub achievement: Pubkey,
    pub owner: Pubkey,
    pub badge_type: reputation::BadgeType,
    pub mint: Pubkey,
    pub claimed_at: i64,
}

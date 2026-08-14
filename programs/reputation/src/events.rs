use anchor_lang::prelude::*;

use crate::state::BadgeType;

#[event]
pub struct ProfileCreated {
    pub profile: Pubkey,
    pub authority: Pubkey,
    pub created_at: i64,
}

#[event]
pub struct RatingSubmitted {
    pub rating: Pubkey,
    pub job_id: u64,
    pub client: Pubkey,
    pub freelancer: Pubkey,
    pub score: u8,
    pub new_average_rating: u32,
}

#[event]
pub struct CompletionUpdated {
    pub profile: Pubkey,
    pub completed_jobs: u64,
    pub successful_jobs: u64,
    pub total_earnings: u64,
    pub reputation_score: u64,
}

#[event]
pub struct BadgeAwarded {
    pub badge: Pubkey,
    pub profile: Pubkey,
    pub badge_type: BadgeType,
    pub issued_at: i64,
}

#[event]
pub struct ProfileUpdated {
    pub profile: Pubkey,
    pub reputation_score: u64,
    pub updated_at: i64,
}

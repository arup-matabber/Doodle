pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;
pub mod utils;

use anchor_lang::prelude::*;

pub use constants::*;
pub use errors::*;
pub use events::*;
pub use instructions::*;
pub use state::*;

declare_id!("HHcJGbKX7fzaPE1UYxqfhGgToDHCDhiWCMawbkumewtt");

#[program]
pub mod reputation {
    use super::*;

    /// Creates a user's reputation profile PDA. Callable once per authority.
    pub fn initialize_profile(ctx: Context<InitializeProfile>) -> Result<()> {
        initialize_profile::handler(ctx)
    }

    /// Client submits an immutable 1-5 rating for a completed job.
    pub fn submit_rating(
        ctx: Context<SubmitRating>,
        job_id: u64,
        score: u8,
        review_hash: [u8; 32],
    ) -> Result<()> {
        submit_rating::handler(ctx, job_id, score, review_hash)
    }

    /// Records a job completion (success or cancellation) and its earnings.
    pub fn update_completion(
        ctx: Context<UpdateCompletion>,
        successful: bool,
        earnings: u64,
    ) -> Result<()> {
        update_completion::handler(ctx, successful, earnings)
    }

    /// Awards a deterministically-eligible badge to a profile.
    pub fn award_badge(
        ctx: Context<AwardBadge>,
        badge_type: BadgeType,
        metadata: String,
    ) -> Result<()> {
        award_badge::handler(ctx, badge_type, metadata)
    }

    /// Read-only helper returning a profile's current reputation score.
    pub fn get_profile(ctx: Context<GetProfile>) -> Result<u64> {
        get_profile::handler(ctx)
    }
}

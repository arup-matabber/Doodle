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

declare_id!("7cvNPW6wKmQLVaygQi1cwrSHz1GNLtzQsurENYyfAkd4");

#[program]
pub mod escrow {
    use super::*;

    /// Creates the next milestone for a gig, awaiting funding.
    pub fn create_milestone(ctx: Context<CreateMilestone>, amount: u64) -> Result<()> {
        create_milestone::handler(ctx, amount)
    }

    /// Funds a milestone by transferring USDC from the client into the escrow vault.
    /// On the gig's first funding, CPIs into the gig program to move it InProgress.
    pub fn fund_milestone(ctx: Context<FundMilestone>) -> Result<()> {
        fund_milestone::handler(ctx)
    }

    /// Records the freelancer's delivery submission.
    pub fn submit_delivery(ctx: Context<SubmitDelivery>) -> Result<()> {
        submit_delivery::handler(ctx)
    }

    /// Client-approved release of the full remaining milestone balance.
    pub fn approve_milestone(ctx: Context<ApproveMilestone>) -> Result<()> {
        approve_milestone::handler(ctx)
    }

    /// Permissionless 20% release after 72 hours of client inaction.
    pub fn partial_timeout_release(ctx: Context<PartialTimeoutRelease>) -> Result<()> {
        partial_timeout_release::handler(ctx)
    }

    /// Permissionless release of the remaining balance after 7 days of client inaction.
    pub fn full_timeout_release(ctx: Context<FullTimeoutRelease>) -> Result<()> {
        full_timeout_release::handler(ctx)
    }

    /// Cancels and closes a milestone that was never funded, cancelling the gig via CPI.
    pub fn cancel_before_funding(ctx: Context<CancelBeforeFunding>) -> Result<()> {
        cancel_before_funding::handler(ctx)
    }

    /// Client submits the final 1-5 rating for a completed gig, CPI-attested by escrow.
    pub fn rate_freelancer(
        ctx: Context<RateFreelancer>,
        score: u8,
        review_hash: [u8; 32],
    ) -> Result<()> {
        rate_freelancer::handler(ctx, score, review_hash)
    }

    /// Permissionlessly notifies the Reputation Program once a gig's vault is
    /// fully released. Callable exactly once per gig.
    pub fn settle_reputation(ctx: Context<SettleReputation>) -> Result<()> {
        settle_reputation::handler(ctx)
    }
}

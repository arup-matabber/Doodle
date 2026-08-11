pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use errors::*;
pub use events::*;
pub use instructions::*;
pub use state::*;

declare_id!("ALa1EKfsHHMefH139SYuSNxd2XbUPj9EN3xRTbPHQNwA");

#[program]
pub mod gig {
    use super::*;

    /// Creates a new gig in Draft status with title, description, category, budget, and deadline.
    pub fn initialize_gig(
        ctx: Context<InitializeGig>,
        id: u64,
        title: String,
        description: String,
        category: String,
        budget: u64,
        deadline: i64,
    ) -> Result<()> {
        initialize_gig::handler(ctx, id, title, description, category, budget, deadline)
    }

    /// Updates a Draft gig's metadata.
    pub fn update_gig(
        ctx: Context<UpdateGig>,
        title: String,
        description: String,
        skills: String,
        category: String,
        budget: u64,
        deadline: i64,
    ) -> Result<()> {
        update_gig::handler(ctx, title, description, skills, category, budget, deadline)
    }

    /// Publishes a Draft gig, making it visible for freelancer assignment.
    pub fn publish_gig(ctx: Context<PublishGig>) -> Result<()> {
        publish_gig::handler(ctx)
    }

    /// Assigns a freelancer to a Published gig, transitioning it to Assigned.
    pub fn assign_freelancer(ctx: Context<AssignFreelancer>) -> Result<()> {
        assign_freelancer::handler(ctx)
    }

    /// Manual completion path for gigs that never entered escrow.
    pub fn complete_gig(ctx: Context<CompleteGig>) -> Result<()> {
        complete_gig::handler(ctx)
    }

    /// Archives a Completed gig.
    pub fn archive_gig(ctx: Context<ArchiveGig>) -> Result<()> {
        archive_gig::handler(ctx)
    }

    /// Client-driven cancellation before any milestone funding.
    pub fn cancel_gig(ctx: Context<CancelGig>) -> Result<()> {
        cancel_gig::handler(ctx)
    }

    /// CPI-only: escrow signals its first milestone funding, moving Assigned -> InProgress.
    pub fn mark_in_progress(ctx: Context<MarkInProgress>) -> Result<()> {
        mark_in_progress::handler(ctx)
    }

    /// CPI-only: escrow signals its final milestone release, moving InProgress -> Completed.
    pub fn mark_completed_by_escrow(ctx: Context<MarkCompletedByEscrow>) -> Result<()> {
        mark_completed_by_escrow::handler(ctx)
    }

    /// CPI-only: escrow signals a cancellation, moving Assigned/InProgress -> Cancelled.
    pub fn mark_cancelled_by_escrow(ctx: Context<MarkCancelledByEscrow>) -> Result<()> {
        mark_cancelled_by_escrow::handler(ctx)
    }
}

use anchor_lang::prelude::*;

#[event]
pub struct MilestoneCreated {
    pub gig: Pubkey,
    pub milestone: Pubkey,
    pub index: u32,
    pub amount: u64,
}

#[event]
pub struct MilestoneFunded {
    pub gig: Pubkey,
    pub milestone: Pubkey,
    pub amount: u64,
}

#[event]
pub struct DeliverySubmitted {
    pub gig: Pubkey,
    pub milestone: Pubkey,
    pub submitted_at: i64,
}

#[event]
pub struct MilestoneApproved {
    pub gig: Pubkey,
    pub milestone: Pubkey,
    pub amount_released: u64,
    pub approved_at: i64,
}

#[event]
pub struct PartialReleaseExecuted {
    pub gig: Pubkey,
    pub milestone: Pubkey,
    pub amount_released: u64,
}

#[event]
pub struct FullReleaseExecuted {
    pub gig: Pubkey,
    pub milestone: Pubkey,
    pub amount_released: u64,
}

#[event]
pub struct MilestoneCancelledBeforeFunding {
    pub gig: Pubkey,
    pub milestone: Pubkey,
    pub index: u32,
}

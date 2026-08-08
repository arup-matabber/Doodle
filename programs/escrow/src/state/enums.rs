use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum MilestoneStatus {
    PendingFunding,
    Funded,
    Submitted,
    PartialReleased,
    Completed,
}

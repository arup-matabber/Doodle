use anchor_lang::prelude::*;

#[error_code]
pub enum EscrowError {
    #[msg("Signer is not authorized to perform this action")]
    Unauthorized,
    #[msg("Account is not in the required status for this action")]
    InvalidStatus,
    #[msg("Milestone has already been funded")]
    AlreadyFunded,
    #[msg("Vault does not hold sufficient funds")]
    InsufficientFunds,
    #[msg("Token mint does not match the expected mint")]
    InvalidMint,
    #[msg("Milestone has already been submitted")]
    MilestoneAlreadySubmitted,
    #[msg("Timeout window has not yet elapsed")]
    TimeoutNotReached,
    #[msg("Arithmetic overflow")]
    Overflow,
    #[msg("Arithmetic error")]
    MathError,
    #[msg("Milestone amount must be greater than zero")]
    InvalidAmount,
    #[msg("Gig must be in Assigned or InProgress status to create/fund a milestone")]
    GigNotFundable,
}

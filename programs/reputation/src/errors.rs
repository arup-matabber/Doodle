use anchor_lang::prelude::*;

#[error_code]
pub enum ReputationError {
    #[msg("Profile already exists")]
    ProfileAlreadyExists,
    #[msg("Profile not found")]
    ProfileNotFound,
    #[msg("Rating must be between 1 and 5")]
    InvalidRating,
    #[msg("This job has already been rated")]
    DuplicateRating,
    #[msg("Badge has already been awarded to this profile")]
    BadgeAlreadyOwned,
    #[msg("Profile is not eligible for this badge yet")]
    BadgeNotEligible,
    #[msg("Signer is not authorized to perform this action")]
    Unauthorized,
    #[msg("Arithmetic overflow")]
    MathOverflow,
    #[msg("Client and freelancer must be different accounts")]
    SelfDealing,
    #[msg("Total earnings must not decrease")]
    InvalidEarnings,
    #[msg("Badge metadata exceeds maximum length")]
    MetadataTooLong,
}

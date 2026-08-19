use anchor_lang::prelude::*;

#[error_code]
pub enum AchievementError {
    #[msg("Achievement collection has already been initialized")]
    CollectionAlreadyInitialized,
    #[msg("Badge has not been earned by this profile")]
    BadgeNotEarned,
    #[msg("Badge does not belong to the claiming profile")]
    ProfileMismatch,
    #[msg("Achievement has already been claimed")]
    AlreadyClaimed,
    #[msg("Signer is not the achievement config admin")]
    Unauthorized,
    #[msg("Metadata base URI is empty or exceeds the maximum length")]
    BaseUriTooLong,
}

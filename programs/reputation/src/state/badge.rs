use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum BadgeType {
    FirstGig,
    TenCompletedJobs,
    HundredCompletedJobs,
    FiveStarPerformer,
    TrustedFreelancer,
    FastDeliverer,
    TopRated,
}

impl BadgeType {
    pub fn as_seed(&self) -> u8 {
        *self as u8
    }
}

#[account]
pub struct Badge {
    pub profile: Pubkey,
    pub badge_type: BadgeType,
    pub issuer: Pubkey,
    pub issued_at: i64,
    /// Free-form metadata (e.g. a URI to badge artwork or criteria snapshot).
    pub metadata: String,
    pub bump: u8,
}

impl Badge {
    pub const MAX_METADATA_LEN: usize = 128;

    pub const INIT_SPACE: usize = 8 // discriminator
        + 32 // profile
        + 1 // badge_type
        + 32 // issuer
        + 8 // issued_at
        + 4 + Self::MAX_METADATA_LEN // metadata (String prefix + bytes)
        + 1; // bump
}

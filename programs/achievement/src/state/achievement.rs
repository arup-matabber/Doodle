use anchor_lang::prelude::*;

/// One PDA per (owner, badge_type). Existence + `claimed = true` is the
/// on-chain record that the badge's NFT has been minted; the PDA's `init`
/// constraint at claim time is what makes a duplicate claim impossible.
#[account]
pub struct Achievement {
    pub owner: Pubkey,
    pub badge_type: reputation::BadgeType,
    pub mint: Pubkey,
    pub claimed: bool,
    pub claimed_at: i64,
    pub bump: u8,
}

impl Achievement {
    pub const INIT_SPACE: usize = 8 // discriminator
        + 32 // owner
        + 1 // badge_type
        + 32 // mint
        + 1 // claimed
        + 8 // claimed_at
        + 1; // bump
}

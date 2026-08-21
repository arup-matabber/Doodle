use anchor_lang::prelude::*;

/// Singleton config holding the shared Metaplex Core collection that every
/// claimed achievement NFT is minted into, plus the base URI its metadata is
/// served from.
#[account]
pub struct AchievementConfig {
    pub admin: Pubkey,
    pub collection: Pubkey,
    /// Base URI for off-chain badge metadata JSON; the badge's slug is appended
    /// (e.g. `<base_uri>/first-gig.json`). Stored on-chain rather than compiled
    /// in as a `const` so a wrong or moved metadata host can be corrected with
    /// `set_base_uri` instead of a program redeploy.
    pub base_uri: String,
    pub bump: u8,
}

impl AchievementConfig {
    pub const MAX_BASE_URI_LEN: usize = 128;

    pub const INIT_SPACE: usize = 8 // discriminator
        + 32 // admin
        + 32 // collection
        + 4 + Self::MAX_BASE_URI_LEN // base_uri (String prefix + bytes)
        + 1; // bump
}

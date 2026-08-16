use anchor_lang::prelude::*;

#[account]
pub struct Rating {
    pub job_id: u64,
    pub client: Pubkey,
    pub freelancer: Pubkey,
    pub score: u8,
    /// Hash (e.g. SHA-256) of the off-chain review text, keeping the raw
    /// review content off-chain while still binding it immutably here.
    pub review_hash: [u8; 32],
    pub submitted_at: i64,
    pub bump: u8,
}

impl Rating {
    pub const INIT_SPACE: usize = 8 // discriminator
        + 8 // job_id
        + 32 // client
        + 32 // freelancer
        + 1 // score
        + 32 // review_hash
        + 8 // submitted_at
        + 1; // bump
}

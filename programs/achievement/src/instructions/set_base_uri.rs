use anchor_lang::prelude::*;

use crate::constants::CONFIG_SEED;
use crate::errors::AchievementError;
use crate::state::AchievementConfig;

/// Admin-only update of the metadata host. Existing NFTs keep the URI they were
/// minted with -- this only affects assets minted after the change.
#[derive(Accounts)]
pub struct SetBaseUri<'info> {
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = admin @ AchievementError::Unauthorized,
    )]
    pub config: Account<'info, AchievementConfig>,
}

pub fn handler(ctx: Context<SetBaseUri>, base_uri: String) -> Result<()> {
    require!(!base_uri.is_empty(), AchievementError::BaseUriTooLong);
    require!(
        base_uri.len() <= AchievementConfig::MAX_BASE_URI_LEN,
        AchievementError::BaseUriTooLong
    );

    ctx.accounts.config.base_uri = base_uri;

    Ok(())
}

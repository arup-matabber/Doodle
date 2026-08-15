use anchor_lang::prelude::*;

use crate::constants::{BADGE_SEED, PROFILE_SEED};
use crate::errors::ReputationError;
use crate::events::BadgeAwarded;
use crate::state::{Badge, BadgeType, UserProfile};
use crate::utils::{checked_add, is_eligible_for_badge};

#[derive(Accounts)]
#[instruction(badge_type: BadgeType, metadata: String)]
pub struct AwardBadge<'info> {
    /// Permissionless: eligibility is recomputed from the profile's own public
    /// fields (see `is_eligible_for_badge`), so there is no privileged data to
    /// gate here. Only pays the badge account's rent.
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [PROFILE_SEED, profile.authority.as_ref()],
        bump = profile.bump,
    )]
    pub profile: Account<'info, UserProfile>,

    #[account(
        init,
        payer = payer,
        space = Badge::INIT_SPACE,
        seeds = [BADGE_SEED, profile.authority.as_ref(), &[badge_type.as_seed()]],
        bump,
    )]
    pub badge: Account<'info, Badge>,

    pub system_program: Program<'info, System>,
}

/// Awards `badge_type` to `profile` if eligibility rules are met. The PDA
/// seeds include the badge type, so a duplicate award for the same
/// profile+type fails at account-init time rather than needing a manual
/// existence check.
pub fn handler(ctx: Context<AwardBadge>, badge_type: BadgeType, metadata: String) -> Result<()> {
    require!(
        metadata.len() <= Badge::MAX_METADATA_LEN,
        ReputationError::MetadataTooLong
    );
    require!(
        is_eligible_for_badge(&ctx.accounts.profile, badge_type),
        ReputationError::BadgeNotEligible
    );

    let now = Clock::get()?.unix_timestamp;

    let badge = &mut ctx.accounts.badge;
    badge.profile = ctx.accounts.profile.key();
    badge.badge_type = badge_type;
    badge.issuer = ctx.accounts.payer.key();
    badge.issued_at = now;
    badge.metadata = metadata;
    badge.bump = ctx.bumps.badge;

    let profile = &mut ctx.accounts.profile;
    profile.badges_earned = u32::try_from(checked_add(profile.badges_earned as u64, 1)?)
        .map_err(|_| ReputationError::MathOverflow)?;
    profile.updated_at = now;

    emit!(BadgeAwarded {
        badge: badge.key(),
        profile: profile.key(),
        badge_type,
        issued_at: now,
    });

    Ok(())
}

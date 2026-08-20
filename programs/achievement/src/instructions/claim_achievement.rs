use anchor_lang::prelude::*;
use mpl_core::instructions::CreateV2CpiBuilder;
use mpl_core::types::DataState;
use reputation::{Badge, BadgeType, UserProfile};

use crate::constants::{badge_name, badge_uri, ACHIEVEMENT_SEED, CONFIG_SEED};
use crate::errors::AchievementError;
use crate::events::AchievementClaimed;
use crate::state::{Achievement, AchievementConfig};

#[derive(Accounts)]
#[instruction(badge_type: BadgeType)]
pub struct ClaimAchievement<'info> {
    #[account(mut)]
    pub claimer: Signer<'info>,

    #[account(
        seeds = [reputation::PROFILE_SEED, claimer.key().as_ref()],
        bump = profile.bump,
        seeds::program = reputation::ID,
    )]
    pub profile: Account<'info, UserProfile>,

    /// Existence of this PDA, owned by the reputation program and derived
    /// from `claimer` + `badge_type`, IS the eligibility proof: reputation's
    /// own `award_badge` instruction already checked the earning criteria
    /// before creating it. Achievement never recomputes or stores that logic.
    #[account(
        seeds = [reputation::BADGE_SEED, claimer.key().as_ref(), &[badge_type.as_seed()]],
        bump = badge.bump,
        seeds::program = reputation::ID,
        constraint = badge.profile == profile.key() @ AchievementError::ProfileMismatch,
    )]
    pub badge: Account<'info, Badge>,

    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
    )]
    pub config: Account<'info, AchievementConfig>,

    /// One PDA per (owner, badge_type); `init` fails outright on a repeat
    /// claim, so duplicate/replay claims are rejected by the runtime itself.
    #[account(
        init,
        payer = claimer,
        space = Achievement::INIT_SPACE,
        seeds = [ACHIEVEMENT_SEED, claimer.key().as_ref(), &[badge_type.as_seed()]],
        bump,
    )]
    pub achievement: Account<'info, Achievement>,

    /// CHECK: fresh account for the new Metaplex Core asset, created by the CPI below.
    #[account(mut)]
    pub asset: Signer<'info>,

    /// CHECK: verified against the config's recorded collection.
    #[account(mut, address = config.collection)]
    pub collection: UncheckedAccount<'info>,

    /// CHECK: verified against the hardcoded Metaplex Core program id.
    #[account(address = mpl_core::ID)]
    pub mpl_core_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<ClaimAchievement>, badge_type: BadgeType) -> Result<()> {
    require!(
        ctx.accounts.badge.badge_type == badge_type,
        AchievementError::BadgeNotEarned
    );

    let config_bump = ctx.accounts.config.bump;
    let config_signer_seeds: &[&[&[u8]]] = &[&[CONFIG_SEED, &[config_bump]]];

    CreateV2CpiBuilder::new(&ctx.accounts.mpl_core_program.to_account_info())
        .asset(&ctx.accounts.asset.to_account_info())
        .collection(Some(&ctx.accounts.collection.to_account_info()))
        .authority(Some(&ctx.accounts.config.to_account_info()))
        .payer(&ctx.accounts.claimer.to_account_info())
        .owner(Some(&ctx.accounts.claimer.to_account_info()))
        .system_program(&ctx.accounts.system_program.to_account_info())
        .data_state(DataState::AccountState)
        .name(badge_name(badge_type).to_string())
        .uri(badge_uri(&ctx.accounts.config.base_uri, badge_type))
        .invoke_signed(config_signer_seeds)?;

    let now = Clock::get()?.unix_timestamp;

    let achievement = &mut ctx.accounts.achievement;
    achievement.owner = ctx.accounts.claimer.key();
    achievement.badge_type = badge_type;
    achievement.mint = ctx.accounts.asset.key();
    achievement.claimed = true;
    achievement.claimed_at = now;
    achievement.bump = ctx.bumps.achievement;

    emit!(AchievementClaimed {
        achievement: achievement.key(),
        owner: achievement.owner,
        badge_type,
        mint: achievement.mint,
        claimed_at: now,
    });

    Ok(())
}

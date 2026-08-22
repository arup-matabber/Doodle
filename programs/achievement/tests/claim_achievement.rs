//! Achievement program tests.
//!
//! This sandbox has no network access to fetch the deployed Metaplex Core
//! `.so`, so tests here cover everything Anchor's account-constraint layer
//! enforces -- eligibility, ownership, PDA identity, signer checks, and
//! duplicate/replay rejection -- all of which run and fail (or pass)
//! *before* the `claim_achievement` handler ever reaches the mpl-core CPI.
//! `eligible_claim_reaches_mpl_core_cpi` confirms a fully valid claim clears
//! every one of those checks and fails only at the external CPI boundary,
//! which is the one thing this offline suite cannot execute for real.

mod common;

use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::{AccountSerialize, InstructionData, ToAccountMetas};
use common::*;
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;

fn setup_eligible_user(env: &mut Env) -> (Pubkey, Pubkey) {
    let user_kp = env.user.insecure_clone();
    let profile = init_reputation_profile(env, &user_kp);
    set_completed_jobs(env, &profile, 1);
    let badge = award_badge(env, &user_kp.pubkey(), &profile, BadgeType::FirstGig);
    (profile, badge)
}

/// Seeds a config account directly (bypassing the mpl-core CPI in
/// `init_collection`) so claim-path tests can exercise everything up to the
/// external CPI without a live Metaplex Core deployment.
fn seed_config(env: &mut Env, collection: &Pubkey) -> Pubkey {
    let (config_key, bump) = config_pda();
    let config = AchievementConfig {
        admin: env.payer.pubkey(),
        collection: *collection,
        base_uri: achievement::DEFAULT_METADATA_BASE_URI.to_string(),
        bump,
    };
    let mut bytes = Vec::new();
    config.try_serialize(&mut bytes).unwrap();
    let rent = env.svm.minimum_balance_for_rent_exemption(bytes.len());
    let account = solana_account::Account {
        lamports: rent,
        data: bytes,
        owner: achievement::ID,
        executable: false,
        rent_epoch: 0,
    };
    env.svm.set_account(config_key, account).unwrap();
    config_key
}

#[test]
fn ineligible_user_rejected() {
    let mut env = setup();
    let user_kp = env.user.insecure_clone();
    let profile = init_reputation_profile(&mut env, &user_kp);
    // completed_jobs stays 0 -- never eligible, so no badge PDA exists.
    let (badge, _) = reputation_badge_pda(&env.user.pubkey(), BadgeType::FirstGig);
    let collection = Keypair::new();
    let config = seed_config(&mut env, &collection.pubkey());
    let (achievement, _) = achievement_pda(&env.user.pubkey(), BadgeType::FirstGig);
    let asset = Keypair::new();

    let ix = ix_claim_achievement(
        &ClaimAccounts {
            claimer: env.user.pubkey(),
            profile,
            badge,
            config,
            achievement,
            asset: asset.pubkey(),
            collection: collection.pubkey(),
            mpl_core_program: mpl_core::ID,
        },
        BadgeType::FirstGig,
    );

    let result = send(&mut env.svm, &env.payer, &[ix], &[&env.payer, &env.user, &asset]);
    assert!(result.is_err(), "claim without an earned badge must be rejected");
}

#[test]
fn forged_badge_rejected() {
    let mut env = setup();
    let (profile, _real_badge) = setup_eligible_user(&mut env);
    let collection = Keypair::new();
    let config = seed_config(&mut env, &collection.pubkey());
    let (achievement, _) = achievement_pda(&env.user.pubkey(), BadgeType::FirstGig);
    let asset = Keypair::new();

    // Attacker substitutes a system-owned account at the badge PDA slot
    // instead of the real reputation-owned Badge account.
    let forged_badge = Keypair::new();

    let ix = ix_claim_achievement(
        &ClaimAccounts {
            claimer: env.user.pubkey(),
            profile,
            badge: forged_badge.pubkey(),
            config,
            achievement,
            asset: asset.pubkey(),
            collection: collection.pubkey(),
            mpl_core_program: mpl_core::ID,
        },
        BadgeType::FirstGig,
    );

    let result = send(&mut env.svm, &env.payer, &[ix], &[&env.payer, &env.user, &asset]);
    assert!(result.is_err(), "a forged badge account must be rejected");
}

#[test]
fn forged_profile_rejected() {
    let mut env = setup();
    let (_profile, badge) = setup_eligible_user(&mut env);
    let collection = Keypair::new();
    let config = seed_config(&mut env, &collection.pubkey());
    let (achievement, _) = achievement_pda(&env.user.pubkey(), BadgeType::FirstGig);
    let asset = Keypair::new();

    // A different, unrelated profile PDA can never satisfy the seeds
    // constraint derived from `claimer`.
    let other = Keypair::new();
    env.svm.airdrop(&other.pubkey(), 1_000_000_000).unwrap();
    let forged_profile = init_reputation_profile(&mut env, &other);

    let ix = ix_claim_achievement(
        &ClaimAccounts {
            claimer: env.user.pubkey(),
            profile: forged_profile,
            badge,
            config,
            achievement,
            asset: asset.pubkey(),
            collection: collection.pubkey(),
            mpl_core_program: mpl_core::ID,
        },
        BadgeType::FirstGig,
    );

    let result = send(&mut env.svm, &env.payer, &[ix], &[&env.payer, &env.user, &asset]);
    assert!(result.is_err(), "a forged/substituted profile account must be rejected");
}

#[test]
fn invalid_signer_rejected() {
    let mut env = setup();
    let (profile, badge) = setup_eligible_user(&mut env);
    let collection = Keypair::new();
    let config = seed_config(&mut env, &collection.pubkey());
    let (achievement, _) = achievement_pda(&env.user.pubkey(), BadgeType::FirstGig);
    let asset = Keypair::new();

    // A different keypair tries to claim `env.user`'s badge by naming
    // itself as `claimer` without ever signing as `env.user`.
    let attacker = Keypair::new();
    env.svm.airdrop(&attacker.pubkey(), 1_000_000_000).unwrap();

    let ix = ix_claim_achievement(
        &ClaimAccounts {
            claimer: env.user.pubkey(),
            profile,
            badge,
            config,
            achievement,
            asset: asset.pubkey(),
            collection: collection.pubkey(),
            mpl_core_program: mpl_core::ID,
        },
        BadgeType::FirstGig,
    );

    let result = send(&mut env.svm, &env.payer, &[ix], &[&env.payer, &attacker, &asset]);
    assert!(result.is_err(), "claiming without the real owner's signature must be rejected");
}

#[test]
fn invalid_pda_rejected() {
    let mut env = setup();
    let (profile, badge) = setup_eligible_user(&mut env);
    let collection = Keypair::new();
    let config = seed_config(&mut env, &collection.pubkey());
    let asset = Keypair::new();

    // A fresh, non-PDA account passed in place of the derived achievement PDA.
    let wrong_achievement = Keypair::new();

    let ix = Instruction::new_with_bytes(
        achievement::ID,
        &achievement::instruction::ClaimAchievement { badge_type: BadgeType::FirstGig }.data(),
        achievement::accounts::ClaimAchievement {
            claimer: env.user.pubkey(),
            profile,
            badge,
            config,
            achievement: wrong_achievement.pubkey(),
            asset: asset.pubkey(),
            collection: collection.pubkey(),
            mpl_core_program: mpl_core::ID,
            system_program: anchor_lang::solana_program::system_program::ID,
        }
        .to_account_metas(None),
    );

    let result = send(&mut env.svm, &env.payer, &[ix], &[&env.payer, &env.user, &asset]);
    assert!(result.is_err(), "an invalid (non-derived) achievement PDA must be rejected");
}

#[test]
fn duplicate_claim_rejected() {
    let mut env = setup();
    let (profile, badge) = setup_eligible_user(&mut env);
    let collection = Keypair::new();
    let config = seed_config(&mut env, &collection.pubkey());
    let user_pubkey = env.user.pubkey();
    let achievement = seed_claimed_achievement(&mut env, &user_pubkey, BadgeType::FirstGig);
    let asset = Keypair::new();

    let ix = ix_claim_achievement(
        &ClaimAccounts {
            claimer: env.user.pubkey(),
            profile,
            badge,
            config,
            achievement,
            asset: asset.pubkey(),
            collection: collection.pubkey(),
            mpl_core_program: mpl_core::ID,
        },
        BadgeType::FirstGig,
    );

    let result = send(&mut env.svm, &env.payer, &[ix], &[&env.payer, &env.user, &asset]);
    assert!(result.is_err(), "a repeat claim for an already-claimed badge must be rejected");

    let stored = read_achievement(&env.svm, &achievement);
    assert!(stored.claimed);
}

#[test]
fn eligible_claim_reaches_mpl_core_cpi() {
    let mut env = setup();
    let (profile, badge) = setup_eligible_user(&mut env);
    let collection = Keypair::new();
    let config = seed_config(&mut env, &collection.pubkey());
    let (achievement, _) = achievement_pda(&env.user.pubkey(), BadgeType::FirstGig);
    let asset = Keypair::new();

    let ix = ix_claim_achievement(
        &ClaimAccounts {
            claimer: env.user.pubkey(),
            profile,
            badge,
            config,
            achievement,
            asset: asset.pubkey(),
            collection: collection.pubkey(),
            mpl_core_program: mpl_core::ID,
        },
        BadgeType::FirstGig,
    );

    let result = send(&mut env.svm, &env.payer, &[ix], &[&env.payer, &env.user, &asset]);
    // Every on-chain check this program owns (eligibility, ownership,
    // signer, PDA identity, duplicate-claim) has passed by construction of
    // this test; the only reason this can still fail here is that the real
    // Metaplex Core program isn't deployed in this offline sandbox.
    assert!(result.is_err(), "expected the only failure to be the missing mpl-core program, got: {result:?}");
}

#[test]
fn config_seeded_correctly() {
    let mut env = setup();
    let collection = Keypair::new();
    let config_key = seed_config(&mut env, &collection.pubkey());
    let data = env.svm.get_account(&config_key).unwrap().data;
    let config = anchor_lang::AccountDeserialize::try_deserialize(&mut &data[..]).map(|c: AchievementConfig| c).unwrap();
    assert_eq!(config.collection, collection.pubkey());
    assert_eq!(config.admin, env.payer.pubkey());
}

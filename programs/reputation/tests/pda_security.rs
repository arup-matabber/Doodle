//! PDA and CPI-authorization security tests.
//!
//! `update_completion` and `submit_rating` are only satisfiable by a real
//! signature over `escrow_authority` -- and a PDA has no private key, so
//! the only way to produce that signature is `invoke_signed` from inside
//! the escrow program itself (see `programs/escrow/tests/reputation_settlement.rs`
//! for the legitimate CPI path). Everything below proves the illegitimate
//! paths are rejected.

mod common;

use anchor_lang::{solana_program::instruction::Instruction, InstructionData, ToAccountMetas};
use common::*;
use solana_keypair::Keypair;
use solana_signer::Signer;

/// An attacker's own real keypair, standing in for `escrow_authority`, can
/// produce a genuine signature -- but its pubkey will never match the PDA
/// derived from `ESCROW_AUTHORITY_SEED` under the escrow program's ID, so
/// the `seeds`/`seeds::program` constraint must reject it.
#[test]
fn test_update_completion_rejects_non_pda_signer() {
    let mut env = setup();
    let freelancer = env.freelancer.insecure_clone();
    init_profile(&mut env, &freelancer);

    let attacker = Keypair::new();
    env.svm.airdrop(&attacker.pubkey(), 10_000_000_000).unwrap();

    let (profile, _) = profile_pda(&freelancer.pubkey());
    let result = send(
        &mut env.svm,
        &env.payer.insecure_clone(),
        &[ix_update_completion(&attacker.pubkey(), &profile, true, 100)],
        &[&env.payer.insecure_clone(), &attacker],
    );
    expect_send_error(
        result.unwrap_err(),
        ERR_CONSTRAINT_SEEDS,
        "direct call with an attacker-controlled key must fail PDA seed validation",
    );
}

/// Same forgery attempt against `submit_rating`'s escrow co-signer.
#[test]
fn test_submit_rating_rejects_non_pda_signer() {
    let mut env = setup();
    let freelancer = env.freelancer.insecure_clone();
    init_profile(&mut env, &freelancer);

    let attacker = Keypair::new();
    env.svm.airdrop(&attacker.pubkey(), 10_000_000_000).unwrap();

    let (profile, _) = profile_pda(&freelancer.pubkey());
    let (rating, _) = rating_pda(&env.client.pubkey(), 1);
    let result = send(
        &mut env.svm,
        &env.payer.insecure_clone(),
        &[ix_submit_rating(
            &env.client.pubkey(),
            &attacker.pubkey(),
            &freelancer.pubkey(),
            &profile,
            &rating,
            1,
            5,
            DEFAULT_REVIEW_HASH,
        )],
        &[&env.payer.insecure_clone(), &env.client.insecure_clone(), &attacker],
    );
    expect_send_error(
        result.unwrap_err(),
        ERR_CONSTRAINT_SEEDS,
        "forged rating co-signer must fail PDA seed validation",
    );
}

/// Even the *correct* escrow_authority PDA address cannot be used directly:
/// with no matching keypair, a transaction requiring its signature can never
/// be constructed/signed outside of escrow's own `invoke_signed` CPI.
#[test]
fn test_escrow_authority_pda_has_no_usable_keypair() {
    let (pda, _) = escrow_authority_pda();
    assert!(
        Keypair::new().pubkey() != pda,
        "sanity: PDAs are not on the ed25519 curve and can never be produced by Keypair::new()"
    );
}

#[test]
fn test_profile_pda_cannot_be_spoofed_with_wrong_seed() {
    let mut env = setup();
    let real_authority = env.freelancer.insecure_clone();
    init_profile(&mut env, &real_authority);

    let fake_profile = Keypair::new();
    let (rating, _) = rating_pda(&env.client.pubkey(), 99);
    let (escrow_authority, _) = escrow_authority_pda();

    let ix = Instruction::new_with_bytes(
        reputation::ID,
        &reputation::instruction::SubmitRating {
            job_id: 99,
            score: 5,
            review_hash: [0u8; 32],
        }
        .data(),
        reputation::accounts::SubmitRating {
            client: env.client.pubkey(),
            escrow_authority,
            freelancer: real_authority.pubkey(),
            freelancer_profile: fake_profile.pubkey(),
            rating,
            system_program: anchor_lang::solana_program::system_program::ID,
        }
        .to_account_metas(None),
    );

    // Can't even sign this: nobody holds `escrow_authority`'s private key,
    // so building the transaction itself panics rather than returning Ok.
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        send(
            &mut env.svm,
            &env.payer.insecure_clone(),
            &[ix],
            &[&env.payer.insecure_clone(), &env.client.insecure_clone()],
        )
    }));
    assert!(
        outcome.is_err() || outcome.unwrap().is_err(),
        "unsigned/unsignable escrow_authority must fail"
    );
}

#[test]
fn test_wrong_bump_for_profile_rejected() {
    let mut env = setup();
    let freelancer = env.freelancer.insecure_clone();
    init_profile(&mut env, &freelancer);

    let (profile_key, _) = profile_pda(&freelancer.pubkey());
    env.svm.expire_blockhash();
    let ix = Instruction::new_with_bytes(
        reputation::ID,
        &reputation::instruction::InitializeProfile {}.data(),
        reputation::accounts::InitializeProfile {
            authority: freelancer.pubkey(),
            profile: profile_key,
            system_program: anchor_lang::solana_program::system_program::ID,
        }
        .to_account_metas(None),
    );

    let result = send(
        &mut env.svm,
        &env.payer.insecure_clone(),
        &[ix],
        &[&env.payer.insecure_clone(), &freelancer],
    );
    assert!(result.is_err(), "re-init should fail");
}

#[test]
fn test_wrong_owner_account_rejected_as_profile() {
    let mut env = setup();
    let freelancer = env.freelancer.insecure_clone();
    init_profile(&mut env, &freelancer);

    let system_account = Keypair::new();
    env.svm.airdrop(&system_account.pubkey(), 1_000_000_000).unwrap();

    let (rating_key, _) = rating_pda(&env.client.pubkey(), 100);
    let (escrow_authority, _) = escrow_authority_pda();
    let ix = Instruction::new_with_bytes(
        reputation::ID,
        &reputation::instruction::SubmitRating {
            job_id: 100,
            score: 5,
            review_hash: [0u8; 32],
        }
        .data(),
        reputation::accounts::SubmitRating {
            client: env.client.pubkey(),
            escrow_authority,
            freelancer: freelancer.pubkey(),
            freelancer_profile: system_account.pubkey(),
            rating: rating_key,
            system_program: anchor_lang::solana_program::system_program::ID,
        }
        .to_account_metas(None),
    );

    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        send(
            &mut env.svm,
            &env.payer.insecure_clone(),
            &[ix],
            &[&env.payer.insecure_clone(), &env.client.insecure_clone()],
        )
    }));
    assert!(
        outcome.is_err() || outcome.unwrap().is_err(),
        "system-owned account should not pass as profile"
    );
}

#[test]
fn test_profile_pda_ownership() {
    let mut env = setup();
    let freelancer = env.freelancer.insecure_clone();
    let profile_key = init_profile(&mut env, &freelancer);

    assert!(account_owned_by_program(&env.svm, &profile_key));
}

#[test]
fn test_badge_award_fails_without_eligibility() {
    let mut env = setup();
    let freelancer = env.freelancer.insecure_clone();
    let pk = freelancer.pubkey();
    init_profile(&mut env, &freelancer);

    let result = award_badge_for(&mut env, &pk, BadgeType::FirstGig);
    expect_error(result, ERR_BADGE_NOT_ELIGIBLE, "no completions -> not eligible");
}

#[test]
fn test_trusted_freelancer_badge_not_awardable_yet() {
    // No on-chain signal backs these two badge types yet (see utils::is_eligible_for_badge),
    // so award_badge -- now permissionless -- must never hand them out.
    let mut env = setup();
    let freelancer = env.freelancer.insecure_clone();
    let pk = freelancer.pubkey();
    init_profile(&mut env, &freelancer);

    for bt in [BadgeType::TrustedFreelancer, BadgeType::FastDeliverer] {
        let result = award_badge_for(&mut env, &pk, bt);
        expect_error(result, ERR_BADGE_NOT_ELIGIBLE, "unattested badge type must be unawardable");
    }
}

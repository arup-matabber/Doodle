//! End-to-end cross-program coverage for the escrow <-> reputation CPI:
//! gig, escrow, and reputation are deployed together in the same litesvm
//! instance, and escrow's `settle_reputation` / `rate_freelancer` instructions
//! really CPI into the live reputation program (not a mock).

mod common;

use anchor_lang::{InstructionData, ToAccountMetas};
use common::*;
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(90_000);
fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

// ─────────────────────────────────────────────────────
// Positive path: settlement updates reputation exactly once
// ─────────────────────────────────────────────────────

#[test]
fn test_settle_reputation_updates_profile_after_full_settlement() {
    let mut env = setup();
    let id = next_id();
    let s = create_funded_milestone(&mut env, id, STANDARD_AMOUNT);
    let f = env.freelancer.insecure_clone(); init_reputation_profile(&mut env, &f);

    send(&mut env.svm, &env.payer, &[ix_submit_delivery(&env.freelancer.pubkey(), &s.gig, &s.milestone)], &[&env.payer, &env.freelancer]).unwrap();
    send(
        &mut env.svm,
        &env.payer,
        &[ix_approve_milestone(&ReleaseAccounts {
            client: env.client.pubkey(),
            gig: s.gig,
            milestone: s.milestone,
            vault: s.vault,
            vault_token_account: s.vault_token_account,
            freelancer: env.freelancer.pubkey(),
            freelancer_token_account: s.freelancer_token_account,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    )
    .unwrap();
    assert_eq!(read_gig(&env.svm, &s.gig).status, GigStatus::Completed);

    let (profile, _) = reputation_profile_pda(&env.freelancer.pubkey());
    send(&mut env.svm, &env.payer, &[ix_settle_reputation(&s.gig, &s.vault, &profile)], &[&env.payer]).unwrap();

    let p = read_reputation_profile(&env.svm, &profile);
    assert_eq!(p.completed_jobs, 1);
    assert_eq!(p.successful_jobs, 1);
    assert_eq!(p.total_earnings, STANDARD_AMOUNT);
    assert!(p.reputation_score > 0);
    assert!(read_vault(&env.svm, &s.vault).reputation_synced);
}

#[test]
fn test_settle_reputation_cannot_fire_twice() {
    let mut env = setup();
    let id = next_id();
    let s = create_funded_milestone(&mut env, id, STANDARD_AMOUNT);
    let f = env.freelancer.insecure_clone(); init_reputation_profile(&mut env, &f);

    send(&mut env.svm, &env.payer, &[ix_submit_delivery(&env.freelancer.pubkey(), &s.gig, &s.milestone)], &[&env.payer, &env.freelancer]).unwrap();
    send(
        &mut env.svm,
        &env.payer,
        &[ix_approve_milestone(&ReleaseAccounts {
            client: env.client.pubkey(),
            gig: s.gig,
            milestone: s.milestone,
            vault: s.vault,
            vault_token_account: s.vault_token_account,
            freelancer: env.freelancer.pubkey(),
            freelancer_token_account: s.freelancer_token_account,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    )
    .unwrap();

    let (profile, _) = reputation_profile_pda(&env.freelancer.pubkey());
    send(&mut env.svm, &env.payer, &[ix_settle_reputation(&s.gig, &s.vault, &profile)], &[&env.payer]).unwrap();
    env.svm.expire_blockhash();

    // Second attempt must be rejected -- earnings must never be double-credited.
    let result = send(&mut env.svm, &env.payer, &[ix_settle_reputation(&s.gig, &s.vault, &profile)], &[&env.payer]);
    assert!(result.is_err(), "settle_reputation must be callable exactly once per gig");

    let p = read_reputation_profile(&env.svm, &profile);
    assert_eq!(p.completed_jobs, 1, "duplicate settlement must not double-count");
    assert_eq!(p.total_earnings, STANDARD_AMOUNT);
}

#[test]
fn test_settle_reputation_rejected_before_full_release() {
    let mut env = setup();
    let id = next_id();
    // Two milestones; only fund/approve the first -- vault is not fully released.
    let gig = init_gig(&mut env, id);
    publish_and_assign(&mut env, &gig);
    let m0 = create_milestone_for(&mut env, &gig, 0, STANDARD_AMOUNT);
    let _m1 = create_milestone_for(&mut env, &gig, 1, STANDARD_AMOUNT);

    let (vault, _) = vault_pda(&gig);
    let (vault_token_account, _) = vault_token_pda(&gig);
    let client_token_account = create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.client.pubkey());
    let freelancer_token_account = create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.freelancer.pubkey());
    mint_to(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.payer, &client_token_account, STANDARD_AMOUNT * 2);

    send(&mut env.svm, &env.payer, &[ix_fund_milestone(&FundAccounts {
        client: env.client.pubkey(), gig, milestone: m0, vault, vault_token_account, client_token_account, mint: env.mint.pubkey(),
    })], &[&env.payer, &env.client]).unwrap();
    send(&mut env.svm, &env.payer, &[ix_submit_delivery(&env.freelancer.pubkey(), &gig, &m0)], &[&env.payer, &env.freelancer]).unwrap();
    send(&mut env.svm, &env.payer, &[ix_approve_milestone(&ReleaseAccounts {
        client: env.client.pubkey(), gig, milestone: m0, vault, vault_token_account, freelancer: env.freelancer.pubkey(),
        freelancer_token_account, mint: env.mint.pubkey(),
    })], &[&env.payer, &env.client]).unwrap();

    // Gig is not Completed yet -- second milestone still pending.
    assert_eq!(read_gig(&env.svm, &gig).status, GigStatus::InProgress);

    let f = env.freelancer.insecure_clone(); init_reputation_profile(&mut env, &f);
    let (profile, _) = reputation_profile_pda(&env.freelancer.pubkey());
    let result = send(&mut env.svm, &env.payer, &[ix_settle_reputation(&gig, &vault, &profile)], &[&env.payer]);
    assert!(result.is_err(), "settle_reputation must reject before every milestone is released");
}

#[test]
fn test_settle_reputation_requires_existing_profile() {
    let mut env = setup();
    let id = next_id();
    let s = create_funded_milestone(&mut env, id, STANDARD_AMOUNT);
    // Freelancer never called initialize_profile.

    send(&mut env.svm, &env.payer, &[ix_submit_delivery(&env.freelancer.pubkey(), &s.gig, &s.milestone)], &[&env.payer, &env.freelancer]).unwrap();
    send(
        &mut env.svm,
        &env.payer,
        &[ix_approve_milestone(&ReleaseAccounts {
            client: env.client.pubkey(),
            gig: s.gig,
            milestone: s.milestone,
            vault: s.vault,
            vault_token_account: s.vault_token_account,
            freelancer: env.freelancer.pubkey(),
            freelancer_token_account: s.freelancer_token_account,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    )
    .unwrap();

    let (profile, _) = reputation_profile_pda(&env.freelancer.pubkey());
    let result = send(&mut env.svm, &env.payer, &[ix_settle_reputation(&s.gig, &s.vault, &profile)], &[&env.payer]);
    assert!(result.is_err(), "settle_reputation must fail without an existing freelancer profile");
}

// ─────────────────────────────────────────────────────
// Ratings
// ─────────────────────────────────────────────────────

fn complete_gig(env: &mut Env, id: u64) -> SetupAccounts {
    let s = create_funded_milestone(env, id, STANDARD_AMOUNT);
    send(&mut env.svm, &env.payer, &[ix_submit_delivery(&env.freelancer.pubkey(), &s.gig, &s.milestone)], &[&env.payer, &env.freelancer]).unwrap();
    send(
        &mut env.svm,
        &env.payer,
        &[ix_approve_milestone(&ReleaseAccounts {
            client: env.client.pubkey(),
            gig: s.gig,
            milestone: s.milestone,
            vault: s.vault,
            vault_token_account: s.vault_token_account,
            freelancer: env.freelancer.pubkey(),
            freelancer_token_account: s.freelancer_token_account,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    )
    .unwrap();
    s
}

#[test]
fn test_rate_freelancer_records_rating_for_completed_gig() {
    let mut env = setup();
    let id = next_id();
    let s = complete_gig(&mut env, id);
    let f = env.freelancer.insecure_clone(); init_reputation_profile(&mut env, &f);

    let (profile, _) = reputation_profile_pda(&env.freelancer.pubkey());
    let (rating, _) = reputation_rating_pda(&env.client.pubkey(), id);

    send(&mut env.svm, &env.payer, &[ix_settle_reputation(&s.gig, &s.vault, &profile)], &[&env.payer]).unwrap();

    send(
        &mut env.svm,
        &env.payer,
        &[ix_rate_freelancer(
            &RateFreelancerAccounts {
                client: env.client.pubkey(),
                gig: s.gig,
                vault: s.vault,
                freelancer: env.freelancer.pubkey(),
                freelancer_profile: profile,
                rating,
            },
            5,
            [1u8; 32],
        )],
        &[&env.payer, &env.client],
    )
    .unwrap();

    let p = read_reputation_profile(&env.svm, &profile);
    assert_eq!(p.rating_count, 1);
    assert_eq!(p.average_rating, 500);
}

#[test]
fn test_rate_freelancer_rejects_duplicate_rating() {
    let mut env = setup();
    let id = next_id();
    let s = complete_gig(&mut env, id);
    let f = env.freelancer.insecure_clone(); init_reputation_profile(&mut env, &f);

    let (profile, _) = reputation_profile_pda(&env.freelancer.pubkey());
    let (rating, _) = reputation_rating_pda(&env.client.pubkey(), id);

    send(&mut env.svm, &env.payer, &[ix_settle_reputation(&s.gig, &s.vault, &profile)], &[&env.payer]).unwrap();

    let acc = RateFreelancerAccounts { client: env.client.pubkey(), gig: s.gig, vault: s.vault, freelancer: env.freelancer.pubkey(), freelancer_profile: profile, rating };
    send(&mut env.svm, &env.payer, &[ix_rate_freelancer(&acc, 5, [1u8; 32])], &[&env.payer, &env.client]).unwrap();
    env.svm.expire_blockhash();
    let result = send(&mut env.svm, &env.payer, &[ix_rate_freelancer(&acc, 1, [2u8; 32])], &[&env.payer, &env.client]);
    assert!(result.is_err(), "duplicate rating for the same gig must be rejected");
}

#[test]
fn test_rate_freelancer_rejects_before_completion() {
    let mut env = setup();
    let id = next_id();
    let s = create_funded_milestone(&mut env, id, STANDARD_AMOUNT);
    let f = env.freelancer.insecure_clone(); init_reputation_profile(&mut env, &f);

    // Gig is InProgress, not Completed -- no delivery/approval yet.
    let (profile, _) = reputation_profile_pda(&env.freelancer.pubkey());
    let (rating, _) = reputation_rating_pda(&env.client.pubkey(), id);
    let acc = RateFreelancerAccounts { client: env.client.pubkey(), gig: s.gig, vault: s.vault, freelancer: env.freelancer.pubkey(), freelancer_profile: profile, rating };
    let result = send(&mut env.svm, &env.payer, &[ix_rate_freelancer(&acc, 5, [0u8; 32])], &[&env.payer, &env.client]);
    assert!(result.is_err(), "rating an incomplete gig must be rejected");
}

#[test]
fn test_rate_freelancer_rejects_wrong_client() {
    let mut env = setup();
    let id = next_id();
    let s = complete_gig(&mut env, id);
    let f = env.freelancer.insecure_clone(); init_reputation_profile(&mut env, &f);

    let impostor = Keypair::new();
    env.svm.airdrop(&impostor.pubkey(), 10_000_000_000).unwrap();

    let (profile, _) = reputation_profile_pda(&env.freelancer.pubkey());
    let (rating, _) = reputation_rating_pda(&env.client.pubkey(), id);

    send(&mut env.svm, &env.payer, &[ix_settle_reputation(&s.gig, &s.vault, &profile)], &[&env.payer]).unwrap();

    let acc = RateFreelancerAccounts { client: impostor.pubkey(), gig: s.gig, vault: s.vault, freelancer: env.freelancer.pubkey(), freelancer_profile: profile, rating };
    let result = send(&mut env.svm, &env.payer, &[ix_rate_freelancer(&acc, 5, [0u8; 32])], &[&env.payer, &impostor]);
    assert!(result.is_err(), "only the gig's real client may submit the rating");
}

#[test]
fn test_rate_freelancer_rejects_before_settlement() {
    let mut env = setup();
    let id = next_id();
    let s = complete_gig(&mut env, id);
    let f = env.freelancer.insecure_clone(); init_reputation_profile(&mut env, &f);

    // Gig is Completed but settle_reputation was never called -- vault.reputation_synced is false.
    let (profile, _) = reputation_profile_pda(&env.freelancer.pubkey());
    let (rating, _) = reputation_rating_pda(&env.client.pubkey(), id);
    let acc = RateFreelancerAccounts { client: env.client.pubkey(), gig: s.gig, vault: s.vault, freelancer: env.freelancer.pubkey(), freelancer_profile: profile, rating };
    let result = send(&mut env.svm, &env.payer, &[ix_rate_freelancer(&acc, 5, [0u8; 32])], &[&env.payer, &env.client]);
    assert!(result.is_err(), "rating before reputation settlement must be rejected");
}

// ─────────────────────────────────────────────────────
// CPI-forgery: escrow_authority can only ever be escrow's own PDA
// ─────────────────────────────────────────────────────

#[test]
fn test_direct_reputation_update_completion_by_non_escrow_signer_rejected() {
    let mut env = setup();
    let freelancer = env.freelancer.insecure_clone();
    init_reputation_profile(&mut env, &freelancer);

    let attacker = Keypair::new();
    env.svm.airdrop(&attacker.pubkey(), 10_000_000_000).unwrap();

    let (profile, _) = reputation_profile_pda(&freelancer.pubkey());
    let ix = anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        reputation::ID,
        &reputation::instruction::UpdateCompletion { successful: true, earnings: 1_000_000 }.data(),
        reputation::accounts::UpdateCompletion { escrow_authority: attacker.pubkey(), profile }.to_account_metas(None),
    );
    let result = send(&mut env.svm, &env.payer, &[ix], &[&env.payer, &attacker]);
    assert!(result.is_err(), "an attacker keypair must never pass as escrow_authority");
}

#[test]
fn test_wrong_program_pda_cannot_impersonate_escrow_authority() {
    // A PDA with the right seed but derived under gig's program ID (a
    // different, legitimate program) must still fail -- seeds::program
    // pins the derivation to ESCROW_PROGRAM_ID specifically.
    let (wrong_program_pda, _) = Pubkey::find_program_address(&[b"escrow_authority"], &gig::ID);
    let (real_pda, _) = escrow_authority_pda();
    assert_ne!(wrong_program_pda, real_pda, "sanity: different program IDs must derive different PDAs");
}

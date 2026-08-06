//! End-to-end cross-program coverage: the gig and escrow programs are deployed together
//! in the same litesvm instance, and escrow's payment instructions really CPI into the
//! live gig program (not a mock) to drive Gig status transitions.

mod common;

use common::*;
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(50_000);
fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

// ─────────────────────────────────────────────────────
// Full happy-path flow: create -> publish -> assign -> fund -> InProgress
// -> fund/approve final milestone -> Completed
// ─────────────────────────────────────────────────────

#[test]
fn test_full_flow_create_publish_assign_fund_transitions_to_in_progress() {
    let mut env = setup();
    let id = next_id();

    let gig = init_gig(&mut env, id);
    assert_eq!(read_gig(&env.svm, &gig).status, GigStatus::Draft);

    publish_gig(&mut env, &gig);
    assert_eq!(read_gig(&env.svm, &gig).status, GigStatus::Published);

    let freelancer = env.freelancer.pubkey();
    assign_freelancer_to(&mut env, &gig, &freelancer);
    assert_eq!(read_gig(&env.svm, &gig).status, GigStatus::Assigned);

    let milestone = create_milestone_for(&mut env, &gig, 0, STANDARD_AMOUNT);

    let (vault, _) = vault_pda(&gig);
    let (vault_token_account, _) = vault_token_pda(&gig);
    let client_token_account =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.client.pubkey());
    mint_to(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.payer, &client_token_account, STANDARD_AMOUNT);

    // Funding the first milestone must CPI into gig::mark_in_progress.
    send(
        &mut env.svm,
        &env.payer,
        &[ix_fund_milestone(&FundAccounts {
            client: env.client.pubkey(),
            gig,
            milestone,
            vault,
            vault_token_account,
            client_token_account,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    )
    .unwrap();

    assert_eq!(read_gig(&env.svm, &gig).status, GigStatus::InProgress, "fund_milestone must CPI mark_in_progress");
}

#[test]
fn test_fund_final_milestone_and_approve_transitions_gig_to_completed() {
    let mut env = setup();
    let id = next_id();
    let s = create_funded_milestone(&mut env, id, STANDARD_AMOUNT);
    assert_eq!(read_gig(&env.svm, &s.gig).status, GigStatus::InProgress);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &s.gig, &s.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

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

    // Only milestone -> last milestone -> approve_milestone must CPI mark_completed_by_escrow.
    assert_eq!(read_gig(&env.svm, &s.gig).status, GigStatus::Completed);
}

#[test]
fn test_cancel_before_funding_transitions_gig_to_cancelled_via_cpi() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_and_assign(&mut env, &gig);
    let milestone = create_milestone_for(&mut env, &gig, 0, STANDARD_AMOUNT);

    assert_eq!(read_gig(&env.svm, &gig).status, GigStatus::Assigned);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_cancel_before_funding(&env.client.pubkey(), &gig, &milestone)],
        &[&env.payer, &env.client],
    )
    .unwrap();

    // cancel_before_funding must CPI mark_cancelled_by_escrow -- escrow no longer owns Gig
    // and cannot mutate it directly.
    assert_eq!(read_gig(&env.svm, &gig).status, GigStatus::Cancelled);
}

// ─────────────────────────────────────────────────────
// create_milestone's seeds::program check against the gig program
// ─────────────────────────────────────────────────────

#[test]
fn test_create_milestone_rejects_wrong_owner_gig_account() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_and_assign(&mut env, &gig);

    // A milestone/vault account (owned by escrow, not gig) supplied in the `gig` slot.
    let (milestone, _) = milestone_pda(&gig, 0);
    let (vault, _) = vault_pda(&gig);
    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_create_milestone(&env.client.pubkey(), &vault, &milestone, STANDARD_AMOUNT)],
        &[&env.payer, &env.client],
    )
    .unwrap_err();
    assert!(!err.is_empty(), "wrong-owner account must be rejected: {err}");
}

#[test]
fn test_create_milestone_rejects_invalid_pda_wrong_seeds() {
    let mut env = setup();
    let id = next_id();
    let _real_gig = init_gig(&mut env, id);

    // A "gig" pubkey that was never derived by gig::GIG_SEED at all.
    let fake_gig = Pubkey::new_unique();
    let (milestone, _) = milestone_pda(&fake_gig, 0);
    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_create_milestone(&env.client.pubkey(), &fake_gig, &milestone, STANDARD_AMOUNT)],
        &[&env.payer, &env.client],
    )
    .unwrap_err();
    assert!(err.contains("0xbc4"), "Expected AccountNotInitialized for a non-existent gig, got: {err}");
}

#[test]
fn test_create_milestone_rejects_wrong_client() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_and_assign(&mut env, &gig);

    let random = Keypair::new();
    env.svm.airdrop(&random.pubkey(), 1_000_000_000).unwrap();

    let (milestone, _) = milestone_pda(&gig, 0);
    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_create_milestone(&random.pubkey(), &gig, &milestone, STANDARD_AMOUNT)],
        &[&env.payer, &random],
    )
    .unwrap_err();
    assert!(!err.is_empty());
}

#[test]
fn test_submit_delivery_rejects_wrong_freelancer() {
    let mut env = setup();
    let id = next_id();
    let s = create_funded_milestone(&mut env, id, STANDARD_AMOUNT);

    let random = Keypair::new();
    env.svm.airdrop(&random.pubkey(), 1_000_000_000).unwrap();

    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&random.pubkey(), &s.gig, &s.milestone)],
        &[&env.payer, &random],
    )
    .unwrap_err();
    assert!(!err.is_empty());
}

// ─────────────────────────────────────────────────────
// Unauthorized direct CPI-target calls (privilege escalation attempts)
// ─────────────────────────────────────────────────────

#[test]
fn test_direct_mark_in_progress_by_non_escrow_signer_rejected() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_and_assign(&mut env, &gig);

    let attacker = Keypair::new();
    env.svm.airdrop(&attacker.pubkey(), 1_000_000_000).unwrap();

    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_mark_in_progress(&attacker.pubkey(), &gig)],
        &[&env.payer, &attacker],
    )
    .unwrap_err();
    assert!(err.contains("0x7d6") || err.to_lowercase().contains("seeds"), "expected ConstraintSeeds, got: {err}");
    assert_eq!(read_gig(&env.svm, &gig).status, GigStatus::Assigned);
}

#[test]
fn test_direct_mark_completed_by_escrow_by_non_escrow_signer_rejected() {
    let mut env = setup();
    let id = next_id();
    let s = create_funded_milestone(&mut env, id, STANDARD_AMOUNT);
    assert_eq!(read_gig(&env.svm, &s.gig).status, GigStatus::InProgress);

    let attacker = Keypair::new();
    env.svm.airdrop(&attacker.pubkey(), 1_000_000_000).unwrap();

    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_mark_completed_by_escrow(&attacker.pubkey(), &s.gig)],
        &[&env.payer, &attacker],
    )
    .unwrap_err();
    assert!(err.contains("0x7d6") || err.to_lowercase().contains("seeds"), "expected ConstraintSeeds, got: {err}");
    assert_eq!(read_gig(&env.svm, &s.gig).status, GigStatus::InProgress, "gig must not be force-completed");
}

#[test]
fn test_direct_mark_cancelled_by_escrow_by_non_escrow_signer_rejected() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_and_assign(&mut env, &gig);

    let attacker = Keypair::new();
    env.svm.airdrop(&attacker.pubkey(), 1_000_000_000).unwrap();

    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_mark_cancelled_by_escrow(&attacker.pubkey(), &gig)],
        &[&env.payer, &attacker],
    )
    .unwrap_err();
    assert!(err.contains("0x7d6") || err.to_lowercase().contains("seeds"), "expected ConstraintSeeds, got: {err}");
    assert_eq!(read_gig(&env.svm, &gig).status, GigStatus::Assigned, "gig must not be force-cancelled");
}

// ─────────────────────────────────────────────────────
// Regression: single-program-era behaviors still correct after the split
// ─────────────────────────────────────────────────────

#[test]
fn test_milestone_amounts_and_released_tracking_regression() {
    let mut env = setup();
    let id = next_id();
    let s = create_funded_milestone(&mut env, id, STANDARD_AMOUNT);

    let milestone = read_milestone(&env.svm, &s.milestone);
    assert_eq!(milestone.amount, STANDARD_AMOUNT);
    assert_eq!(milestone.released, 0);
    assert_eq!(milestone.status, MilestoneStatus::Funded);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &s.gig, &s.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    warp_seconds(&mut env.svm, 73 * 3_600);
    send(
        &mut env.svm,
        &env.payer,
        &[ix_partial_timeout_release(&TimeoutAccounts {
            gig: s.gig,
            milestone: s.milestone,
            vault: s.vault,
            vault_token_account: s.vault_token_account,
            freelancer_token_account: s.freelancer_token_account,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer],
    )
    .unwrap();

    let expected_partial = STANDARD_AMOUNT * 20 / 100;
    let milestone = read_milestone(&env.svm, &s.milestone);
    assert_eq!(milestone.released, expected_partial);
    assert_eq!(milestone.status, MilestoneStatus::PartialReleased);

    // Gig must still be InProgress -- partial release never completes a milestone.
    assert_eq!(read_gig(&env.svm, &s.gig).status, GigStatus::InProgress);

    warp_seconds(&mut env.svm, 7 * 86_400);
    send(
        &mut env.svm,
        &env.payer,
        &[ix_full_timeout_release(&TimeoutAccounts {
            gig: s.gig,
            milestone: s.milestone,
            vault: s.vault,
            vault_token_account: s.vault_token_account,
            freelancer_token_account: s.freelancer_token_account,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer],
    )
    .unwrap();

    let milestone = read_milestone(&env.svm, &s.milestone);
    assert_eq!(milestone.released, STANDARD_AMOUNT);
    assert_eq!(milestone.status, MilestoneStatus::Completed);
    verify_vault_invariant(&env.svm, &s.vault, &s.vault_token_account);

    // Last (only) milestone released -> gig CPI'd to Completed.
    assert_eq!(read_gig(&env.svm, &s.gig).status, GigStatus::Completed);
}

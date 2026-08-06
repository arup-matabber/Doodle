mod common;

use common::*;
use std::sync::atomic::{AtomicU64, Ordering};
use solana_signer::Signer;

static NEXT_ID: AtomicU64 = AtomicU64::new(100);
fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

// ── State helpers ──

fn submitted(env: &mut Env, id: u64) -> SetupAccounts {
    let s = create_funded_milestone(env, id, STANDARD_AMOUNT);
    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &s.gig, &s.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();
    s
}

fn completed(env: &mut Env, id: u64) -> SetupAccounts {
    let s = submitted(env, id);
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

// ─────────────────────────────────────────────────────
//  1. Approve before funding
// ─────────────────────────────────────────────────────

#[test]
fn test_approve_before_funding() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_and_assign(&mut env, &gig);
    let milestone = create_milestone_for(&mut env, &gig, 0, STANDARD_AMOUNT);

    let (vault, _) = vault_pda(&gig);
    let (vault_token, _) = vault_token_pda(&gig);
    let freelancer_token = create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.freelancer.pubkey());

    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_approve_milestone(&ReleaseAccounts {
            client: env.client.pubkey(),
            gig,
            milestone,
            vault,
            vault_token_account: vault_token,
            freelancer: env.freelancer.pubkey(),
            freelancer_token_account: freelancer_token,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    )
    .unwrap_err();

    // Vault hasn't been created yet (no funding), so approve fails with
    // AccountNotInitialized (0xbc4) before reaching the milestone status check.
    assert!(err.contains("0xbc4"), "Expected AccountNotInitialized, got: {err}");
}

// ─────────────────────────────────────────────────────
//  2. Approve before submit
// ─────────────────────────────────────────────────────

#[test]
fn test_approve_before_submit() {
    let mut env = setup();
    let id = next_id();
    let s = create_funded_milestone(&mut env, id, STANDARD_AMOUNT);

    let err = send(
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
    .unwrap_err();

    assert!(err.contains("0x1771"), "Expected InvalidStatus, got: {err}");
}

// ─────────────────────────────────────────────────────
//  3. Submit before funding
// ─────────────────────────────────────────────────────

#[test]
fn test_submit_before_funding() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_and_assign(&mut env, &gig);
    let milestone = create_milestone_for(&mut env, &gig, 0, STANDARD_AMOUNT);

    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &gig, &milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap_err();

    assert!(err.contains("0x1771"), "Expected InvalidStatus, got: {err}");
}

// ─────────────────────────────────────────────────────
//  4. Submit twice
// ─────────────────────────────────────────────────────

#[test]
fn test_submit_twice() {
    let mut env = setup();
    let id = next_id();
    let s = create_funded_milestone(&mut env, id, STANDARD_AMOUNT);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &s.gig, &s.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    env.svm.expire_blockhash();
    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &s.gig, &s.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap_err();

    assert!(err.contains("0x1775"), "Expected MilestoneAlreadySubmitted, got: {err}");
}

// ─────────────────────────────────────────────────────
//  5. Approve twice
// ─────────────────────────────────────────────────────

#[test]
fn test_approve_twice() {
    let mut env = setup();
    let id = next_id();
    let s = completed(&mut env, id);

    // Milestone is Completed, approve requires Submitted
    env.svm.expire_blockhash();
    let err = send(
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
    .unwrap_err();

    assert!(err.contains("0x1771"), "Expected InvalidStatus, got: {err}");
}

// ─────────────────────────────────────────────────────
//  6. Fund twice
// ─────────────────────────────────────────────────────

#[test]
fn test_fund_twice() {
    let mut env = setup();
    let id = next_id();
    let s = create_funded_milestone(&mut env, id, STANDARD_AMOUNT);

    let extra_token = create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.client.pubkey());
    mint_to(
        &mut env.svm,
        &env.payer,
        &env.mint.pubkey(),
        &env.payer,
        &extra_token,
        STANDARD_AMOUNT,
    );

    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_fund_milestone(&FundAccounts {
            client: env.client.pubkey(),
            gig: s.gig,
            milestone: s.milestone,
            vault: s.vault,
            vault_token_account: s.vault_token_account,
            client_token_account: extra_token,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    )
    .unwrap_err();

    assert!(err.contains("0x1772"), "Expected AlreadyFunded, got: {err}");
}

// ─────────────────────────────────────────────────────
//  7. Cancel after funding
// ─────────────────────────────────────────────────────

#[test]
fn test_cancel_after_funding() {
    let mut env = setup();
    let id = next_id();
    let s = create_funded_milestone(&mut env, id, STANDARD_AMOUNT);

    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_cancel_before_funding(&env.client.pubkey(), &s.gig, &s.milestone)],
        &[&env.payer, &env.client],
    )
    .unwrap_err();

    assert!(err.contains("0x1772"), "Expected AlreadyFunded, got: {err}");
}

// ─────────────────────────────────────────────────────
//  8. Cancel after submit
// ─────────────────────────────────────────────────────

#[test]
fn test_cancel_after_submit() {
    let mut env = setup();
    let id = next_id();
    let s = submitted(&mut env, id);

    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_cancel_before_funding(&env.client.pubkey(), &s.gig, &s.milestone)],
        &[&env.payer, &env.client],
    )
    .unwrap_err();

    assert!(err.contains("0x1772"), "Expected AlreadyFunded, got: {err}");
}

// ─────────────────────────────────────────────────────
//  9. Partial timeout before submit
// ─────────────────────────────────────────────────────

#[test]
fn test_partial_timeout_before_submit() {
    let mut env = setup();
    let id = next_id();
    let s = create_funded_milestone(&mut env, id, STANDARD_AMOUNT);

    warp_seconds(&mut env.svm, 73 * 3_600);

    let err = send(
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
    .unwrap_err();

    assert!(err.contains("0x1771"), "Expected InvalidStatus, got: {err}");
}

// ─────────────────────────────────────────────────────
//  10. Full timeout without partial
// ─────────────────────────────────────────────────────

/// The 7-day guarantee stands alone: it must not require the 72h partial to
/// have been executed first. Requiring `PartialReleased` made the freelancer's
/// protection conditional on a prior permissionless call succeeding, which is
/// not always possible (see `test_full_timeout_releases_dust_milestone`).
#[test]
fn test_full_timeout_without_partial() {
    let mut env = setup();
    let id = next_id();
    let s = submitted(&mut env, id);

    warp_seconds(&mut env.svm, 8 * 86_400);

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

    let ms = read_milestone(&env.svm, &s.milestone);
    assert_eq!(ms.status, MilestoneStatus::Completed);
    assert_eq!(ms.released, ms.amount, "full timeout must release the entire milestone");
}

// ─────────────────────────────────────────────────────
//  11. Full timeout before deadline after partial
// ─────────────────────────────────────────────────────

#[test]
fn test_full_timeout_before_deadline_after_partial() {
    let mut env = setup();
    let id = next_id();
    let s = submitted(&mut env, id);

    // warp past partial deadline (72h) and execute partial
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

    // warp to 6 days from submission (still before 7-day full deadline)
    let six_days = 6 * 86_400;
    let already_elapsed: i64 = 73 * 3_600;
    warp_seconds(&mut env.svm, six_days - already_elapsed);

    let err = send(
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
    .unwrap_err();

    assert!(err.contains("0x1776"), "Expected TimeoutNotReached, got: {err}");
}

// ─────────────────────────────────────────────────────
//  12. Partial timeout before deadline
// ─────────────────────────────────────────────────────

#[test]
fn test_partial_timeout_before_deadline() {
    let mut env = setup();
    let id = next_id();
    let s = submitted(&mut env, id);

    // warp only 71h (partial needs 72h)
    warp_seconds(&mut env.svm, 71 * 3_600);

    let err = send(
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
    .unwrap_err();

    assert!(err.contains("0x1776"), "Expected TimeoutNotReached, got: {err}");
}

// ─────────────────────────────────────────────────────
//  13. Create milestone after gig completed
// ─────────────────────────────────────────────────────

#[test]
fn test_create_milestone_after_gig_completed() {
    let mut env = setup();
    let id = next_id();
    let s = completed(&mut env, id);

    // gig is Completed, create_milestone requires Active
    let (new_milestone, _) = milestone_pda(&s.gig, 1);
    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_create_milestone(&env.client.pubkey(), &s.gig, &new_milestone, STANDARD_AMOUNT)],
        &[&env.payer, &env.client],
    )
    .unwrap_err();

    assert!(err.contains("0x177a"), "Expected GigNotFundable, got: {err}");
}

// ─────────────────────────────────────────────────────
//  14. Cancel twice
// ─────────────────────────────────────────────────────

#[test]
fn test_cancel_twice() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_and_assign(&mut env, &gig);
    let milestone = create_milestone_for(&mut env, &gig, 0, STANDARD_AMOUNT);

    let cancel_ix = ix_cancel_before_funding(&env.client.pubkey(), &gig, &milestone);
    send(&mut env.svm, &env.payer, &[cancel_ix.clone()], &[&env.payer, &env.client]).unwrap();

    // milestone account was closed; second cancel fails
    let err = send(&mut env.svm, &env.payer, &[cancel_ix], &[&env.payer, &env.client]).unwrap_err();
    assert!(!err.is_empty(), "Expected error when cancelling a closed milestone");
}

// ─────────────────────────────────────────────────────
//  15. Fund zero-amount milestone
// ─────────────────────────────────────────────────────

#[test]
fn test_fund_zero_amount_milestone() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_and_assign(&mut env, &gig);

    let (milestone, _) = milestone_pda(&gig, 0);
    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_create_milestone(&env.client.pubkey(), &gig, &milestone, 0)],
        &[&env.payer, &env.client],
    )
    .unwrap_err();

    assert!(err.contains("0x1779"), "Expected InvalidAmount, got: {err}");
}

// ─────────────────────────────────────────────────────
//  16. Submit on completed milestone
// ─────────────────────────────────────────────────────

#[test]
fn test_submit_on_completed_milestone() {
    let mut env = setup();
    let id = next_id();
    let s = completed(&mut env, id);

    env.svm.expire_blockhash();
    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &s.gig, &s.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap_err();

    assert!(err.contains("0x1771"), "Expected InvalidStatus, got: {err}");
}

// ─────────────────────────────────────────────────────
//  17. Partial timeout on completed milestone
// ─────────────────────────────────────────────────────

#[test]
fn test_partial_timeout_on_completed_milestone() {
    let mut env = setup();
    let id = next_id();
    let s = completed(&mut env, id);

    warp_seconds(&mut env.svm, 73 * 3_600);

    let err = send(
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
    .unwrap_err();

    assert!(err.contains("0x1771"), "Expected InvalidStatus, got: {err}");
}

// ─────────────────────────────────────────────────────
//  18. Full timeout on completed milestone
// ─────────────────────────────────────────────────────

#[test]
fn test_full_timeout_on_completed_milestone() {
    let mut env = setup();
    let id = next_id();
    let s = completed(&mut env, id);

    warp_seconds(&mut env.svm, 8 * 86_400);

    let err = send(
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
    .unwrap_err();

    assert!(err.contains("0x1771"), "Expected InvalidStatus, got: {err}");
}

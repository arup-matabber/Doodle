mod common;

use common::*;
use std::sync::atomic::{AtomicU64, Ordering};
use solana_signer::Signer;

static NEXT_ID: AtomicU64 = AtomicU64::new(200);
fn next_id() -> u64 { NEXT_ID.fetch_add(1, Ordering::Relaxed) }

const PARTIAL_TIMEOUT: i64 = 72 * 3600;
const FULL_TIMEOUT: i64 = 7 * 86_400;
const PARTIAL_PCT: u64 = STANDARD_AMOUNT / 5;

fn advance(env: &mut Env, secs: i64) {
    env.svm.expire_blockhash();
    warp_seconds(&mut env.svm, secs);
}

fn partial_accts(a: &SetupAccounts, mint: solana_pubkey::Pubkey) -> TimeoutAccounts {
    TimeoutAccounts {
        gig: a.gig,
        milestone: a.milestone,
        vault: a.vault,
        vault_token_account: a.vault_token_account,
        freelancer_token_account: a.freelancer_token_account,
        mint,
    }
}

// ── 1. 71h59m — one second before partial timeout ──

#[test]
fn test_partial_71h59m_fails() {
    let mut env = setup();
    let a = create_funded_milestone(&mut env, next_id(), STANDARD_AMOUNT);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &a.gig, &a.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    advance(&mut env, PARTIAL_TIMEOUT - 1);

    let mint = env.mint.pubkey();
    let accts = partial_accts(&a, mint);
    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_partial_timeout_release(&accts)],
        &[&env.payer],
    )
    .unwrap_err();
    assert!(err.contains("0x1776"), "expected TimeoutNotReached, got: {err}");
}

// ── 2. Exactly 72h — partial timeout matures ──

#[test]
fn test_partial_72h_succeeds() {
    let mut env = setup();
    let a = create_funded_milestone(&mut env, next_id(), STANDARD_AMOUNT);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &a.gig, &a.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    advance(&mut env, PARTIAL_TIMEOUT);

    let mint = env.mint.pubkey();
    let accts = partial_accts(&a, mint);
    send(
        &mut env.svm,
        &env.payer,
        &[ix_partial_timeout_release(&accts)],
        &[&env.payer],
    )
    .unwrap();

    assert_eq!(token_balance(&env.svm, &a.freelancer_token_account), PARTIAL_PCT);
}

// ── 3. 72h + 1s ──

#[test]
fn test_partial_72h_plus_1s() {
    let mut env = setup();
    let a = create_funded_milestone(&mut env, next_id(), STANDARD_AMOUNT);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &a.gig, &a.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    advance(&mut env, PARTIAL_TIMEOUT + 1);

    let mint = env.mint.pubkey();
    let accts = partial_accts(&a, mint);
    send(
        &mut env.svm,
        &env.payer,
        &[ix_partial_timeout_release(&accts)],
        &[&env.payer],
    )
    .unwrap();

    assert_eq!(token_balance(&env.svm, &a.freelancer_token_account), PARTIAL_PCT);
}

// ── 4. Full timeout fails 1s before 7d (after partial) ──

#[test]
fn test_full_6d23h59m_fails_after_partial() {
    let mut env = setup();
    let a = create_funded_milestone(&mut env, next_id(), STANDARD_AMOUNT);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &a.gig, &a.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    // Warp past partial timeout and execute partial
    advance(&mut env, PARTIAL_TIMEOUT + 1);
    let mint = env.mint.pubkey();
    let accts = partial_accts(&a, mint);
    send(
        &mut env.svm,
        &env.payer,
        &[ix_partial_timeout_release(&accts)],
        &[&env.payer],
    )
    .unwrap();

    // Warp to 1s before full timeout from submission
    // current clock = submission + PARTIAL_TIMEOUT + 1 = 259201
    // target = submission + FULL_TIMEOUT - 1 = 604799
    // remaining = 604799 - 259201 = 345598
    advance(&mut env, FULL_TIMEOUT - PARTIAL_TIMEOUT - 2);

    let mint = env.mint.pubkey();
    let accts = partial_accts(&a, mint);
    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_full_timeout_release(&accts)],
        &[&env.payer],
    )
    .unwrap_err();
    assert!(err.contains("0x1776"), "expected TimeoutNotReached, got: {err}");
}

// ── 5. Full timeout succeeds at 7d + 1s ──

#[test]
fn test_full_7d_succeeds() {
    let mut env = setup();
    let a = create_funded_milestone(&mut env, next_id(), STANDARD_AMOUNT);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &a.gig, &a.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    advance(&mut env, PARTIAL_TIMEOUT + 1);
    let mint = env.mint.pubkey();
    let accts = partial_accts(&a, mint);
    send(
        &mut env.svm,
        &env.payer,
        &[ix_partial_timeout_release(&accts)],
        &[&env.payer],
    )
    .unwrap();

    // Warp remaining to hit 7d + 1s from submission
    // current clock = submission + PARTIAL_TIMEOUT + 1 = 259201
    // target = submission + FULL_TIMEOUT + 1 = 604801
    // remaining = 604801 - 259201 = 345600 = FULL_TIMEOUT - PARTIAL_TIMEOUT
    advance(&mut env, FULL_TIMEOUT - PARTIAL_TIMEOUT);

    let mint = env.mint.pubkey();
    let accts = partial_accts(&a, mint);
    send(
        &mut env.svm,
        &env.payer,
        &[ix_full_timeout_release(&accts)],
        &[&env.payer],
    )
    .unwrap();

    assert_eq!(token_balance(&env.svm, &a.freelancer_token_account), STANDARD_AMOUNT);
    assert_eq!(token_balance(&env.svm, &a.vault_token_account), 0);
}

// ── 6. Full without partial → allowed straight from Submitted ──

/// The full release accepts `Submitted` directly. It used to require
/// `PartialReleased`, which made the 7-day guarantee depend on the 72h partial
/// having succeeded first -- impossible for milestones whose 20% truncates to
/// zero, stranding those funds for good.
#[test]
fn test_full_7d_without_partial_releases_everything() {
    let mut env = setup();
    let a = create_funded_milestone(&mut env, next_id(), STANDARD_AMOUNT);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &a.gig, &a.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    advance(&mut env, 8 * 86_400);

    let mint = env.mint.pubkey();
    let accts = partial_accts(&a, mint);
    send(
        &mut env.svm,
        &env.payer,
        &[ix_full_timeout_release(&accts)],
        &[&env.payer],
    )
    .unwrap();

    assert_eq!(
        token_balance(&env.svm, &a.freelancer_token_account),
        STANDARD_AMOUNT,
        "a 7-day release straight from Submitted must pay out the whole milestone",
    );
}

// ── 7. Complete flow: fund → submit → partial (20%) → full (80%) ──

#[test]
fn test_timeout_sequence_complete_flow() {
    let mut env = setup();
    let a = create_funded_milestone(&mut env, next_id(), STANDARD_AMOUNT);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &a.gig, &a.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    // Partial timeout at exactly 72h
    advance(&mut env, PARTIAL_TIMEOUT);
    let mint = env.mint.pubkey();
    let accts = partial_accts(&a, mint);
    send(
        &mut env.svm,
        &env.payer,
        &[ix_partial_timeout_release(&accts)],
        &[&env.payer],
    )
    .unwrap();

    assert_eq!(
        token_balance(&env.svm, &a.freelancer_token_account),
        PARTIAL_PCT,
        "partial should release 20%",
    );
    assert_eq!(
        token_balance(&env.svm, &a.vault_token_account),
        STANDARD_AMOUNT - PARTIAL_PCT,
        "vault should hold 80% after partial",
    );

    // Full timeout after additional 5 days
    advance(&mut env, 5 * 86_400);
    let mint = env.mint.pubkey();
    let accts = partial_accts(&a, mint);
    send(
        &mut env.svm,
        &env.payer,
        &[ix_full_timeout_release(&accts)],
        &[&env.payer],
    )
    .unwrap();

    assert_eq!(
        token_balance(&env.svm, &a.freelancer_token_account),
        STANDARD_AMOUNT,
        "freelancer should have 100% after full",
    );
    assert_eq!(
        token_balance(&env.svm, &a.vault_token_account),
        0,
        "vault should be empty after full",
    );

    // Verify milestone and vault state
    let m = read_milestone(&env.svm, &a.milestone);
    assert_eq!(m.released, STANDARD_AMOUNT);

    let v = read_vault(&env.svm, &a.vault);
    assert_eq!(v.total_locked, v.total_released);
}

// ── 8. Exact 72h timing — released amount is exactly 20% ──

#[test]
fn test_partial_exact_timing() {
    let mut env = setup();
    let a = create_funded_milestone(&mut env, next_id(), STANDARD_AMOUNT);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &a.gig, &a.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    advance(&mut env, PARTIAL_TIMEOUT);
    let mint = env.mint.pubkey();
    let accts = partial_accts(&a, mint);
    send(
        &mut env.svm,
        &env.payer,
        &[ix_partial_timeout_release(&accts)],
        &[&env.payer],
    )
    .unwrap();

    let m = read_milestone(&env.svm, &a.milestone);
    assert_eq!(
        m.released, PARTIAL_PCT,
        "exactly 20% of {} should be released, got {}",
        STANDARD_AMOUNT, m.released,
    );
    assert_eq!(
        m.status as u8,
        MilestoneStatus::PartialReleased as u8,
        "status should be PartialReleased",
    );
}

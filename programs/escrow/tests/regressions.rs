//! Regression tests for the escrow logic bugs found in the pre-deployment audit.
//!
//! Each test here pins the *fixed* behaviour of one defect. They are kept in a
//! separate file from `state_transitions.rs` so the failure mode each one
//! guards against stays readable next to the assertion.

mod common;

use common::*;
use solana_signer::Signer;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(9_000);
fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

fn submitted(env: &mut Env, id: u64, amount: u64) -> SetupAccounts {
    let s = create_funded_milestone(env, id, amount);
    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &s.gig, &s.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();
    s
}

fn timeout_accounts(env: &Env, s: &SetupAccounts) -> TimeoutAccounts {
    TimeoutAccounts {
        gig: s.gig,
        milestone: s.milestone,
        vault: s.vault,
        vault_token_account: s.vault_token_account,
        freelancer_token_account: s.freelancer_token_account,
        mint: env.mint.pubkey(),
    }
}

fn release_accounts(env: &Env, s: &SetupAccounts) -> ReleaseAccounts {
    ReleaseAccounts {
        client: env.client.pubkey(),
        gig: s.gig,
        milestone: s.milestone,
        vault: s.vault,
        vault_token_account: s.vault_token_account,
        freelancer: env.freelancer.pubkey(),
        freelancer_token_account: s.freelancer_token_account,
        mint: env.mint.pubkey(),
    }
}

// ─────────────────────────────────────────────────────
//  Fix 1: client can still approve after a partial release
// ─────────────────────────────────────────────────────

/// `approve_milestone` used to require `MilestoneStatus::Submitted`. Because
/// the permissionless 72h partial moves the milestone to `PartialReleased`,
/// a single partial call permanently removed the client's only way to pay --
/// the remaining 80% could then only move at the 7-day full timeout, even
/// when both parties wanted to settle immediately.
#[test]
fn approve_after_partial_release_succeeds() {
    let mut env = setup();
    let s = submitted(&mut env, next_id(), STANDARD_AMOUNT);

    warp_seconds(&mut env.svm, 73 * 3_600);
    let ta = timeout_accounts(&env, &s);
    send(
        &mut env.svm,
        &env.payer,
        &[ix_partial_timeout_release(&ta)],
        &[&env.payer],
    )
    .unwrap();

    let ms = read_milestone(&env.svm, &s.milestone);
    assert_eq!(ms.status, MilestoneStatus::PartialReleased);
    let after_partial = token_balance(&env.svm, &s.freelancer_token_account);
    assert_eq!(after_partial, STANDARD_AMOUNT / 5, "partial must release 20%");

    // The client approves well before the 7-day full timeout.
    let ra = release_accounts(&env, &s);
    send(
        &mut env.svm,
        &env.payer,
        &[ix_approve_milestone(&ra)],
        &[&env.payer, &env.client],
    )
    .unwrap();

    let ms = read_milestone(&env.svm, &s.milestone);
    assert_eq!(ms.status, MilestoneStatus::Completed);
    assert_eq!(ms.released, ms.amount, "approval must top the milestone up to its full amount");
    assert_eq!(
        token_balance(&env.svm, &s.freelancer_token_account),
        STANDARD_AMOUNT,
        "freelancer must end up with exactly the milestone amount, not a double payout"
    );
    verify_vault_invariant(&env.svm, &s.vault, &s.vault_token_account);
}

// ─────────────────────────────────────────────────────
//  Fix 2: dust milestones are not permanently locked
// ─────────────────────────────────────────────────────

/// For a milestone small enough that 20% rounds down to zero,
/// `partial_timeout_release` rejects with `InsufficientFunds`. While
/// `full_timeout_release` required `PartialReleased`, that combination left
/// the funds unreachable by every path once the client went silent.
#[test]
fn full_timeout_releases_dust_milestone() {
    let mut env = setup();
    // 20% of 4 base units truncates to 0.
    let s = submitted(&mut env, next_id(), 4);

    warp_seconds(&mut env.svm, 73 * 3_600);
    let ta = timeout_accounts(&env, &s);
    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_partial_timeout_release(&ta)],
        &[&env.payer],
    )
    .unwrap_err();
    assert!(
        err.contains("0x1773"),
        "partial release of a dust milestone must still reject with InsufficientFunds, got: {err}"
    );

    warp_seconds(&mut env.svm, 8 * 86_400);
    let ta = timeout_accounts(&env, &s);
    send(
        &mut env.svm,
        &env.payer,
        &[ix_full_timeout_release(&ta)],
        &[&env.payer],
    )
    .unwrap();

    let ms = read_milestone(&env.svm, &s.milestone);
    assert_eq!(ms.status, MilestoneStatus::Completed);
    assert_eq!(token_balance(&env.svm, &s.freelancer_token_account), 4);
}

// ─────────────────────────────────────────────────────
//  Fix 3: cancelling an unfunded milestone spares a live gig
// ─────────────────────────────────────────────────────

/// `cancel_before_funding` used to CPI `mark_cancelled_by_escrow`
/// unconditionally, so closing one never-funded milestone tore down a gig
/// whose other milestones were funded and in flight.
#[test]
fn cancelling_unfunded_milestone_keeps_funded_gig_alive() {
    let mut env = setup();
    let s = create_funded_milestone(&mut env, next_id(), STANDARD_AMOUNT);

    // A second milestone that is created but never funded.
    let extra = create_milestone_for(&mut env, &s.gig, 1, STANDARD_AMOUNT);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_cancel_before_funding(&env.client.pubkey(), &s.gig, &extra)],
        &[&env.payer, &env.client],
    )
    .unwrap();

    let gig = read_gig(&env.svm, &s.gig);
    assert_eq!(
        gig.status,
        GigStatus::InProgress,
        "a gig holding funded milestones must survive the cancellation of an unfunded one"
    );

    let vault = read_vault(&env.svm, &s.vault);
    assert_eq!(vault.cancelled_milestones, 1);
    assert_eq!(vault.milestone_count, 2, "milestone_count allocates PDA indices and must stay monotonic");
}

/// With nothing ever locked, cancelling the only milestone should still take
/// the gig down -- that path is unchanged.
#[test]
fn cancelling_only_unfunded_milestone_still_cancels_gig() {
    let mut env = setup();
    let gig = init_gig(&mut env, next_id());
    publish_and_assign(&mut env, &gig);
    let milestone = create_milestone_for(&mut env, &gig, 0, STANDARD_AMOUNT);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_cancel_before_funding(&env.client.pubkey(), &gig, &milestone)],
        &[&env.payer, &env.client],
    )
    .unwrap();

    assert_eq!(read_gig(&env.svm, &gig).status, GigStatus::Cancelled);
}

// ─────────────────────────────────────────────────────
//  Fix 4: settlement stays reachable after a cancellation
// ─────────────────────────────────────────────────────

/// `settle_reputation` required `active_milestone >= milestone_count`. Since
/// a cancelled milestone never increments `active_milestone`, one cancellation
/// made that constraint permanently unsatisfiable -- silently blocking both
/// reputation settlement and `rate_freelancer` for the whole gig.
#[test]
fn settlement_reachable_after_cancelled_milestone() {
    let mut env = setup();
    let s = create_funded_milestone(&mut env, next_id(), STANDARD_AMOUNT);
    let extra = create_milestone_for(&mut env, &s.gig, 1, STANDARD_AMOUNT);

    let freelancer = env.freelancer.insecure_clone();
    let profile = init_reputation_profile(&mut env, &freelancer);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_cancel_before_funding(&env.client.pubkey(), &s.gig, &extra)],
        &[&env.payer, &env.client],
    )
    .unwrap();

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&freelancer.pubkey(), &s.gig, &s.milestone)],
        &[&env.payer, &freelancer],
    )
    .unwrap();
    let ra = release_accounts(&env, &s);
    send(
        &mut env.svm,
        &env.payer,
        &[ix_approve_milestone(&ra)],
        &[&env.payer, &env.client],
    )
    .unwrap();

    send(
        &mut env.svm,
        &env.payer,
        &[ix_settle_reputation(&s.gig, &s.vault, &profile)],
        &[&env.payer],
    )
    .unwrap();

    let p = read_reputation_profile(&env.svm, &profile);
    assert_eq!(p.completed_jobs, 1);
    assert_eq!(p.successful_jobs, 1, "the gig reached Completed, so it settles as successful");
    assert_eq!(p.cancelled_jobs, 0);
    assert_eq!(p.total_earnings, STANDARD_AMOUNT);
}

// ─────────────────────────────────────────────────────
//  Fix 5: cancelled gigs settle as unsuccessful
// ─────────────────────────────────────────────────────

/// `settle_reputation` hardcoded `successful = true`, which made
/// `cancelled_jobs` unreachable from every code path. That pinned
/// `success_rate` at 100 for every profile and turned the cancellation
/// penalty in `compute_reputation_score` into dead code.
#[test]
fn cancelled_gig_settles_as_unsuccessful() {
    let mut env = setup();
    let gig = init_gig(&mut env, next_id());
    publish_and_assign(&mut env, &gig);
    let milestone = create_milestone_for(&mut env, &gig, 0, STANDARD_AMOUNT);

    let freelancer = env.freelancer.insecure_clone();
    let profile = init_reputation_profile(&mut env, &freelancer);
    let (vault, _) = vault_pda(&gig);

    // Nothing was ever funded, so this cancels the gig as well as the milestone.
    send(
        &mut env.svm,
        &env.payer,
        &[ix_cancel_before_funding(&env.client.pubkey(), &gig, &milestone)],
        &[&env.payer, &env.client],
    )
    .unwrap();
    assert_eq!(read_gig(&env.svm, &gig).status, GigStatus::Cancelled);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_settle_reputation(&gig, &vault, &profile)],
        &[&env.payer],
    )
    .unwrap();

    let p = read_reputation_profile(&env.svm, &profile);
    assert_eq!(p.completed_jobs, 1);
    assert_eq!(p.successful_jobs, 0);
    assert_eq!(p.cancelled_jobs, 1, "a cancelled gig must count against the freelancer");
    assert_eq!(p.total_earnings, 0);
    assert!(
        p.reputation_score < 100,
        "success_rate must not be pinned at 100 for a profile with no successful jobs"
    );
}

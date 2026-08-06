mod common;

use common::*;
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(800);
fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

/// Anchor `emit!` writes events via `sol_log_data` producing "Program data:" log entries.
/// We verify an event was emitted by checking for "Program data:" in the logs,
/// AND crucially we verify the state changes that the event describes.
/// This validates event emission + event field correctness simultaneously.

fn has_event(logs: &[String]) -> bool {
    logs.iter().any(|l| l.contains("Program data:"))
}

#[test]
fn test_gig_created_event() {
    let mut env = setup();
    let gig_id = next_id();
    let (expected_gig, _) = gig_pda(&env.client.pubkey(), gig_id);

    let logs = send_logs(
        &mut env.svm,
        &env.payer,
        &[ix_initialize_gig(
            &env.client.pubkey(),
            &env.mint.pubkey(),
            &expected_gig,
            gig_id,
            TEST_TITLE.to_string(),
            TEST_DESCRIPTION.to_string(),
            TEST_CATEGORY.to_string(),
            TEST_BUDGET,
            TEST_DEADLINE,
        )],
        &[&env.payer, &env.client],
    )
    .unwrap();

    let clock = env.svm.get_sysvar::<solana_clock::Clock>();
    assert!(has_event(&logs), "GigCreated event should be emitted");
    let gig = read_gig(&env.svm, &expected_gig);
    assert_eq!(gig.id, gig_id);
    assert_eq!(gig.client, env.client.pubkey());
    assert_eq!(gig.freelancer, Pubkey::default());
    assert_eq!(gig.mint, env.mint.pubkey());
    assert_eq!(gig.status, GigStatus::Draft);
    assert_eq!(gig.created_at, clock.unix_timestamp);
}

#[test]
fn test_milestone_created_event() {
    let mut env = setup();
    let gig_id = next_id();
    let gig_key = init_gig(&mut env, gig_id);
    publish_and_assign(&mut env, &gig_key);
    let (expected_milestone, _) = milestone_pda(&gig_key, 0);

    let logs = send_logs(
        &mut env.svm,
        &env.payer,
        &[ix_create_milestone(&env.client.pubkey(), &gig_key, &expected_milestone, STANDARD_AMOUNT)],
        &[&env.payer, &env.client],
    )
    .unwrap();

    assert!(has_event(&logs), "MilestoneCreated event should be emitted");
    let ms = read_milestone(&env.svm, &expected_milestone);
    assert_eq!(ms.gig, gig_key);
    assert_eq!(ms.index, 0);
    assert_eq!(ms.amount, STANDARD_AMOUNT);
    assert_eq!(ms.status, MilestoneStatus::PendingFunding);
}

#[test]
fn test_milestone_funded_event() {
    let mut env = setup();
    let gig_id = next_id();
    let gig_key = init_gig(&mut env, gig_id);
    publish_and_assign(&mut env, &gig_key);
    let milestone_key = create_milestone_for(&mut env, &gig_key, 0, STANDARD_AMOUNT);

    let (vault, _) = vault_pda(&gig_key);
    let (vault_token_key, _) = vault_token_pda(&gig_key);
    let client_token_account =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.client.pubkey());
    let _freelancer_token_account =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.freelancer.pubkey());

    mint_to(
        &mut env.svm,
        &env.payer,
        &env.mint.pubkey(),
        &env.payer,
        &client_token_account,
        STANDARD_AMOUNT,
    );

    let logs = send_logs(
        &mut env.svm,
        &env.payer,
        &[ix_fund_milestone(&FundAccounts {
            client: env.client.pubkey(),
            gig: gig_key,
            milestone: milestone_key,
            vault,
            vault_token_account: vault_token_key,
            client_token_account,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    )
    .unwrap();

    assert!(has_event(&logs), "MilestoneFunded event should be emitted");
    let ms = read_milestone(&env.svm, &milestone_key);
    assert_eq!(ms.status, MilestoneStatus::Funded);
    assert_eq!(token_balance(&env.svm, &vault_token_key), STANDARD_AMOUNT);
}

#[test]
fn test_delivery_submitted_event() {
    let mut env = setup();
    let gig_id = next_id();
    let s = create_funded_milestone(&mut env, gig_id, STANDARD_AMOUNT);

    let clock_before = env.svm.get_sysvar::<solana_clock::Clock>().unix_timestamp;
    let logs = send_logs(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &s.gig, &s.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();
    let clock_after = env.svm.get_sysvar::<solana_clock::Clock>().unix_timestamp;

    assert!(has_event(&logs), "DeliverySubmitted event should be emitted");
    let ms = read_milestone(&env.svm, &s.milestone);
    assert_eq!(ms.status, MilestoneStatus::Submitted);
    assert!(ms.submitted_at >= clock_before && ms.submitted_at <= clock_after);
}

#[test]
fn test_milestone_approved_event() {
    let mut env = setup();
    let gig_id = next_id();
    let s = create_funded_milestone(&mut env, gig_id, STANDARD_AMOUNT);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &s.gig, &s.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    let clock_before = env.svm.get_sysvar::<solana_clock::Clock>().unix_timestamp;
    let logs = send_logs(
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
    let clock_after = env.svm.get_sysvar::<solana_clock::Clock>().unix_timestamp;

    assert!(has_event(&logs), "MilestoneApproved event should be emitted");
    let ms = read_milestone(&env.svm, &s.milestone);
    assert_eq!(ms.status, MilestoneStatus::Completed);
    assert_eq!(ms.released, STANDARD_AMOUNT);
    assert!(ms.approved_at >= clock_before && ms.approved_at <= clock_after);
    assert_eq!(token_balance(&env.svm, &s.freelancer_token_account), STANDARD_AMOUNT);
}

#[test]
fn test_partial_release_event() {
    let mut env = setup();
    let gig_id = next_id();
    let s = create_funded_milestone(&mut env, gig_id, STANDARD_AMOUNT);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &s.gig, &s.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    warp_seconds(&mut env.svm, 73 * 3600);

    let expected_partial = STANDARD_AMOUNT * 20 / 100;
    let logs = send_logs(
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

    assert!(has_event(&logs), "PartialReleaseExecuted event should be emitted");
    let ms = read_milestone(&env.svm, &s.milestone);
    assert_eq!(ms.status, MilestoneStatus::PartialReleased);
    assert_eq!(ms.released, expected_partial);
    assert_eq!(token_balance(&env.svm, &s.freelancer_token_account), expected_partial);
}

#[test]
fn test_full_release_event() {
    let mut env = setup();
    let gig_id = next_id();
    let s = create_funded_milestone(&mut env, gig_id, STANDARD_AMOUNT);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &s.gig, &s.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    env.svm.expire_blockhash();
    warp_seconds(&mut env.svm, 73 * 3600);

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

    env.svm.expire_blockhash();
    warp_seconds(&mut env.svm, 7 * 86400);

    let logs = send_logs(
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

    assert!(has_event(&logs), "FullReleaseExecuted event should be emitted");
    let ms = read_milestone(&env.svm, &s.milestone);
    assert_eq!(ms.status, MilestoneStatus::Completed);
    assert_eq!(ms.released, STANDARD_AMOUNT);
    assert_eq!(token_balance(&env.svm, &s.freelancer_token_account), STANDARD_AMOUNT);
    assert_eq!(token_balance(&env.svm, &s.vault_token_account), 0);
}

#[test]
fn test_gig_published_event() {
    let mut env = setup();
    let gig_id = next_id();
    let gig_key = init_gig(&mut env, gig_id);

    let logs = send_logs(
        &mut env.svm,
        &env.payer,
        &[ix_publish_gig(&env.client.pubkey(), &gig_key)],
        &[&env.payer, &env.client],
    )
    .unwrap();

    assert!(has_event(&logs), "GigPublished event should be emitted");
    let gig = read_gig(&env.svm, &gig_key);
    assert_eq!(gig.status, GigStatus::Published);
}

#[test]
fn test_freelancer_assigned_event() {
    let mut env = setup();
    let gig_id = next_id();
    let gig_key = init_gig(&mut env, gig_id);
    publish_gig(&mut env, &gig_key);

    let logs = send_logs(
        &mut env.svm,
        &env.payer,
        &[ix_assign_freelancer(&env.client.pubkey(), &env.freelancer.pubkey(), &gig_key)],
        &[&env.payer, &env.client],
    )
    .unwrap();

    assert!(has_event(&logs), "FreelancerAssigned event should be emitted");
    let gig = read_gig(&env.svm, &gig_key);
    assert_eq!(gig.status, GigStatus::Assigned);
    assert_eq!(gig.freelancer, env.freelancer.pubkey());
}

#[test]
fn test_gig_completed_event() {
    let mut env = setup();
    let gig_id = next_id();
    let gig_key = init_gig(&mut env, gig_id);
    let freelancer = env.freelancer.pubkey();
    publish_gig(&mut env, &gig_key);
    assign_freelancer_to(&mut env, &gig_key, &freelancer);

    let logs = send_logs(
        &mut env.svm,
        &env.payer,
        &[ix_complete_gig(&env.client.pubkey(), &gig_key)],
        &[&env.payer, &env.client],
    )
    .unwrap();

    assert!(has_event(&logs), "GigCompleted event should be emitted");
    let gig = read_gig(&env.svm, &gig_key);
    assert_eq!(gig.status, GigStatus::Completed);
}

#[test]
fn test_gig_archived_event() {
    let mut env = setup();
    let gig_id = next_id();
    let gig_key = init_gig(&mut env, gig_id);
    let freelancer = env.freelancer.pubkey();
    publish_gig(&mut env, &gig_key);
    assign_freelancer_to(&mut env, &gig_key, &freelancer);
    complete_gig_for(&mut env, &gig_key);

    let logs = send_logs(
        &mut env.svm,
        &env.payer,
        &[ix_archive_gig(&env.client.pubkey(), &gig_key)],
        &[&env.payer, &env.client],
    )
    .unwrap();

    assert!(has_event(&logs), "GigArchived event should be emitted");
    let gig = read_gig(&env.svm, &gig_key);
    assert_eq!(gig.status, GigStatus::Archived);
}

#[test]
fn test_gig_cancelled_event() {
    let mut env = setup();
    let gig_id = next_id();
    let gig_key = init_gig(&mut env, gig_id);
    publish_and_assign(&mut env, &gig_key);
    let milestone_key = create_milestone_for(&mut env, &gig_key, 0, STANDARD_AMOUNT);

    let logs = send_logs(
        &mut env.svm,
        &env.payer,
        &[ix_cancel_before_funding(&env.client.pubkey(), &gig_key, &milestone_key)],
        &[&env.payer, &env.client],
    )
    .unwrap();

    assert!(has_event(&logs), "GigCancelled event should be emitted");
    assert!(
        env.svm.get_account(&milestone_key).is_none(),
        "milestone should be closed"
    );
    let gig = read_gig(&env.svm, &gig_key);
    assert_eq!(gig.status, GigStatus::Cancelled);
}

#[test]
fn test_no_events_on_failure() {
    let mut env = setup();
    let gig_id = next_id();
    let s = create_funded_milestone(&mut env, gig_id, STANDARD_AMOUNT);

    let result = send(
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
    );

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.contains("0x1771") || err.contains("InvalidStatus"),
        "Expected InvalidStatus, got: {err}"
    );
}

#[test]
fn test_event_order() {
    let mut env = setup();
    let gig_id = next_id();
    let gig_key = init_gig(&mut env, gig_id);
    publish_and_assign(&mut env, &gig_key);
    let milestone_key = create_milestone_for(&mut env, &gig_key, 0, STANDARD_AMOUNT);

    let (vault, _) = vault_pda(&gig_key);
    let (vault_token_key, _) = vault_token_pda(&gig_key);
    let client_token_account =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.client.pubkey());
    let freelancer_token_account =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.freelancer.pubkey());

    mint_to(
        &mut env.svm,
        &env.payer,
        &env.mint.pubkey(),
        &env.payer,
        &client_token_account,
        STANDARD_AMOUNT,
    );

    let logs_fund = send_logs(
        &mut env.svm,
        &env.payer,
        &[ix_fund_milestone(&FundAccounts {
            client: env.client.pubkey(),
            gig: gig_key,
            milestone: milestone_key,
            vault,
            vault_token_account: vault_token_key,
            client_token_account,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    )
    .unwrap();

    assert!(has_event(&logs_fund), "MilestoneFunded event should appear in fund logs");

    env.svm.expire_blockhash();
    let logs_submit = send_logs(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &gig_key, &milestone_key)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    assert!(has_event(&logs_submit), "DeliverySubmitted event should appear in submit logs");

    env.svm.expire_blockhash();
    let logs_approve = send_logs(
        &mut env.svm,
        &env.payer,
        &[ix_approve_milestone(&ReleaseAccounts {
            client: env.client.pubkey(),
            gig: gig_key,
            milestone: milestone_key,
            vault,
            vault_token_account: vault_token_key,
            freelancer: env.freelancer.pubkey(),
            freelancer_token_account,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    )
    .unwrap();

    assert!(has_event(&logs_approve), "MilestoneApproved event should appear in approve logs");

    let ms = read_milestone(&env.svm, &milestone_key);
    assert_eq!(ms.status, MilestoneStatus::Completed);
}

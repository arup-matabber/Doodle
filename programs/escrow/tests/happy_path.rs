mod common;

use common::*;
use solana_clock::Clock;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

#[test]
fn test_initialize_gig() {
    let mut env = setup();
    let gig_id = next_id();

    let clock_before = env.svm.get_sysvar::<Clock>().unix_timestamp;
    let gig_key = init_gig(&mut env, gig_id);
    let clock_after = env.svm.get_sysvar::<Clock>().unix_timestamp;

    let (expected_pda, expected_bump) = gig_pda(&env.client.pubkey(), gig_id);
    assert_eq!(gig_key, expected_pda);

    let gig = read_gig(&env.svm, &gig_key);

    assert_eq!(gig.id, gig_id);
    assert_eq!(gig.client, env.client.pubkey());
    assert_eq!(gig.freelancer, Pubkey::default());
    assert_eq!(gig.mint, env.mint.pubkey());
    assert_eq!(gig.status, GigStatus::Draft);
    assert!(gig.created_at >= clock_before && gig.created_at <= clock_after);
    assert_eq!(gig.bump, expected_bump);
}

#[test]
fn test_create_milestone() {
    let mut env = setup();
    let gig_id = next_id();
    let gig_key = init_gig(&mut env, gig_id);
    publish_and_assign(&mut env, &gig_key);

    let (expected_pda, expected_bump) = milestone_pda(&gig_key, 0);
    let milestone_key = create_milestone_for(&mut env, &gig_key, 0, STANDARD_AMOUNT);
    assert_eq!(milestone_key, expected_pda);

    let milestone = read_milestone(&env.svm, &milestone_key);

    assert_eq!(milestone.gig, gig_key);
    assert_eq!(milestone.index, 0);
    assert_eq!(milestone.amount, STANDARD_AMOUNT);
    assert_eq!(milestone.released, 0);
    assert_eq!(milestone.status, MilestoneStatus::PendingFunding);
    assert_eq!(milestone.submitted_at, 0);
    assert_eq!(milestone.approved_at, 0);
    assert_eq!(milestone.bump, expected_bump);

    let vault_state = read_vault(&env.svm, &vault_pda(&gig_key).0);
    assert_eq!(vault_state.milestone_count, 1);
}

#[test]
fn test_fund_milestone() {
    let mut env = setup();
    let gig_id = next_id();
    let gig_key = init_gig(&mut env, gig_id);
    publish_and_assign(&mut env, &gig_key);
    let milestone_key = create_milestone_for(&mut env, &gig_key, 0, STANDARD_AMOUNT);

    let (vault, vault_bump) = vault_pda(&gig_key);
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

    let client_balance_before = token_balance(&env.svm, &client_token_account);
    assert_eq!(client_balance_before, STANDARD_AMOUNT);

    send(
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

    let milestone = read_milestone(&env.svm, &milestone_key);
    assert_eq!(milestone.status, MilestoneStatus::Funded);

    let vault_state = read_vault(&env.svm, &vault);
    assert_eq!(vault_state.total_locked, STANDARD_AMOUNT);
    assert_eq!(vault_state.total_released, 0);
    assert_eq!(vault_state.mint, env.mint.pubkey());
    assert_eq!(vault_state.gig, gig_key);
    assert_eq!(vault_state.token_account, vault_token_key);
    assert_eq!(vault_state.bump, vault_bump);

    assert_eq!(token_balance(&env.svm, &client_token_account), 0);
    assert_eq!(token_balance(&env.svm, &vault_token_key), STANDARD_AMOUNT);

    verify_vault_invariant(&env.svm, &vault, &vault_token_key);
}

#[test]
fn test_submit_delivery() {
    let mut env = setup();
    let gig_id = next_id();
    let s = create_funded_milestone(&mut env, gig_id, STANDARD_AMOUNT);

    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.client.pubkey(), &s.gig, &s.milestone)],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());

    let clock_before = env.svm.get_sysvar::<Clock>().unix_timestamp;
    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &s.gig, &s.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();
    let clock_after = env.svm.get_sysvar::<Clock>().unix_timestamp;

    let milestone = read_milestone(&env.svm, &s.milestone);
    assert_eq!(milestone.status, MilestoneStatus::Submitted);
    assert!(milestone.submitted_at >= clock_before && milestone.submitted_at <= clock_after);
}

#[test]
fn test_approve_milestone() {
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

    let clock_before = env.svm.get_sysvar::<Clock>().unix_timestamp;
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
    let clock_after = env.svm.get_sysvar::<Clock>().unix_timestamp;

    let milestone = read_milestone(&env.svm, &s.milestone);
    assert_eq!(milestone.status, MilestoneStatus::Completed);
    assert_eq!(milestone.released, STANDARD_AMOUNT);
    assert!(milestone.approved_at >= clock_before && milestone.approved_at <= clock_after);

    let vault_state = read_vault(&env.svm, &s.vault);
    assert_eq!(vault_state.total_released, STANDARD_AMOUNT);

    assert_eq!(token_balance(&env.svm, &s.vault_token_account), 0);
    assert_eq!(
        token_balance(&env.svm, &s.freelancer_token_account),
        STANDARD_AMOUNT
    );

    let gig = read_gig(&env.svm, &s.gig);
    assert_eq!(gig.status, GigStatus::Completed);

    verify_vault_invariant(&env.svm, &s.vault, &s.vault_token_account);
}

#[test]
fn test_partial_timeout_release() {
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
    assert_eq!(milestone.status, MilestoneStatus::PartialReleased);
    assert_eq!(milestone.released, expected_partial);

    let vault_state = read_vault(&env.svm, &s.vault);
    assert_eq!(vault_state.total_released, expected_partial);

    assert_eq!(
        token_balance(&env.svm, &s.freelancer_token_account),
        expected_partial
    );
    assert_eq!(
        token_balance(&env.svm, &s.vault_token_account),
        STANDARD_AMOUNT - expected_partial
    );

    verify_vault_invariant(&env.svm, &s.vault, &s.vault_token_account);
}

#[test]
fn test_full_timeout_release() {
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

    warp_seconds(&mut env.svm, 7 * 86400);

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
    assert_eq!(milestone.status, MilestoneStatus::Completed);
    assert_eq!(milestone.released, STANDARD_AMOUNT);

    let vault_state = read_vault(&env.svm, &s.vault);
    assert_eq!(vault_state.total_released, STANDARD_AMOUNT);

    assert_eq!(token_balance(&env.svm, &s.vault_token_account), 0);
    assert_eq!(
        token_balance(&env.svm, &s.freelancer_token_account),
        STANDARD_AMOUNT
    );

    let gig = read_gig(&env.svm, &s.gig);
    assert_eq!(gig.status, GigStatus::Completed);

    verify_vault_invariant(&env.svm, &s.vault, &s.vault_token_account);
}

#[test]
fn test_cancel_before_funding() {
    let mut env = setup();
    let gig_id = next_id();
    let gig_key = init_gig(&mut env, gig_id);
    publish_and_assign(&mut env, &gig_key);
    let milestone_key = create_milestone_for(&mut env, &gig_key, 0, STANDARD_AMOUNT);

    assert!(env.svm.get_account(&milestone_key).is_some());

    send(
        &mut env.svm,
        &env.payer,
        &[ix_cancel_before_funding(&env.client.pubkey(), &gig_key, &milestone_key)],
        &[&env.payer, &env.client],
    )
    .unwrap();

    assert!(env.svm.get_account(&milestone_key).is_none());

    let gig = read_gig(&env.svm, &gig_key);
    assert_eq!(gig.status, GigStatus::Cancelled);
    let vault_state = read_vault(&env.svm, &vault_pda(&gig_key).0);
    assert_eq!(vault_state.milestone_count, 1);
}

#[test]
fn test_multiple_milestones_full_approve() {
    let mut env = setup();
    let gig_id = next_id();
    let gig_key = init_gig(&mut env, gig_id);
    publish_and_assign(&mut env, &gig_key);

    let amounts = [100_000u64, 200_000u64, 300_000u64];
    let mut milestones: Vec<Pubkey> = Vec::new();
    for (i, &amount) in amounts.iter().enumerate() {
        let m = create_milestone_for(&mut env, &gig_key, i as u32, amount);
        milestones.push(m);
    }

    let (vault, _) = vault_pda(&gig_key);
    let (vault_token_key, _) = vault_token_pda(&gig_key);
    let client_token_account =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.client.pubkey());
    let freelancer_token_account =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.freelancer.pubkey());

    let total: u64 = amounts.iter().sum();
    mint_to(
        &mut env.svm,
        &env.payer,
        &env.mint.pubkey(),
        &env.payer,
        &client_token_account,
        total,
    );

    let mut total_locked: u64 = 0;
    let mut total_released: u64 = 0;

    for (i, (&amount, milestone)) in amounts.iter().zip(&milestones).enumerate() {
        send(
            &mut env.svm,
            &env.payer,
            &[ix_fund_milestone(&FundAccounts {
                client: env.client.pubkey(),
                gig: gig_key,
                milestone: *milestone,
                vault,
                vault_token_account: vault_token_key,
                client_token_account,
                mint: env.mint.pubkey(),
            })],
            &[&env.payer, &env.client],
        )
        .unwrap();
        total_locked += amount;

        let vault_state = read_vault(&env.svm, &vault);
        assert_eq!(vault_state.total_locked, total_locked);
        verify_vault_invariant(&env.svm, &vault, &vault_token_key);

        send(
            &mut env.svm,
            &env.payer,
            &[ix_submit_delivery(&env.freelancer.pubkey(), &gig_key, milestone)],
            &[&env.payer, &env.freelancer],
        )
        .unwrap();

        let ms = read_milestone(&env.svm, milestone);
        assert_eq!(ms.status, MilestoneStatus::Submitted);

        send(
            &mut env.svm,
            &env.payer,
            &[ix_approve_milestone(&ReleaseAccounts {
                client: env.client.pubkey(),
                gig: gig_key,
                milestone: *milestone,
                vault,
                vault_token_account: vault_token_key,
                freelancer: env.freelancer.pubkey(),
                freelancer_token_account,
                mint: env.mint.pubkey(),
            })],
            &[&env.payer, &env.client],
        )
        .unwrap();
        total_released += amount;

        let ms = read_milestone(&env.svm, milestone);
        assert_eq!(ms.status, MilestoneStatus::Completed);
        assert_eq!(ms.released, amount);

        let vault_state = read_vault(&env.svm, &vault);
        let vault_balance = token_balance(&env.svm, &vault_token_key);
        assert_eq!(vault_state.total_released, total_released);
        assert_eq!(vault_balance, total_locked - total_released);
        assert_eq!(
            token_balance(&env.svm, &freelancer_token_account),
            total_released
        );

        verify_vault_invariant(&env.svm, &vault, &vault_token_key);

        let g = read_gig(&env.svm, &gig_key);
        let vault_state = read_vault(&env.svm, &vault);
        if i == amounts.len() - 1 {
            assert_eq!(g.status, GigStatus::Completed);
        } else {
            // First funding CPIs the gig from Assigned -> InProgress; it stays InProgress
            // until the last milestone is approved.
            assert_eq!(g.status, GigStatus::InProgress);
            assert_eq!(vault_state.active_milestone, (i + 1) as u32);
        }
    }

    assert_eq!(token_balance(&env.svm, &vault_token_key), 0);
    assert_eq!(
        token_balance(&env.svm, &freelancer_token_account),
        total
    );
}

#[test]
fn test_gig_completes_on_last_milestone() {
    let mut env = setup();
    let gig_id = next_id();
    let gig_key = init_gig(&mut env, gig_id);
    publish_and_assign(&mut env, &gig_key);

    let m1 = create_milestone_for(&mut env, &gig_key, 0, STANDARD_AMOUNT);
    let m2 = create_milestone_for(&mut env, &gig_key, 1, STANDARD_AMOUNT);

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
        2 * STANDARD_AMOUNT,
    );

    for milestone in [&m1, &m2] {
        send(
            &mut env.svm,
            &env.payer,
            &[ix_fund_milestone(&FundAccounts {
                client: env.client.pubkey(),
                gig: gig_key,
                milestone: *milestone,
                vault,
                vault_token_account: vault_token_key,
                client_token_account,
                mint: env.mint.pubkey(),
            })],
            &[&env.payer, &env.client],
        )
        .unwrap();

        send(
            &mut env.svm,
            &env.payer,
            &[ix_submit_delivery(&env.freelancer.pubkey(), &gig_key, milestone)],
            &[&env.payer, &env.freelancer],
        )
        .unwrap();
    }

    send(
        &mut env.svm,
        &env.payer,
        &[ix_approve_milestone(&ReleaseAccounts {
            client: env.client.pubkey(),
            gig: gig_key,
            milestone: m1,
            vault,
            vault_token_account: vault_token_key,
            freelancer: env.freelancer.pubkey(),
            freelancer_token_account,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    )
    .unwrap();

    let gig_state = read_gig(&env.svm, &gig_key);
    assert_eq!(gig_state.status, GigStatus::InProgress);
    assert_eq!(read_vault(&env.svm, &vault).active_milestone, 1);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_approve_milestone(&ReleaseAccounts {
            client: env.client.pubkey(),
            gig: gig_key,
            milestone: m2,
            vault,
            vault_token_account: vault_token_key,
            freelancer: env.freelancer.pubkey(),
            freelancer_token_account,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    )
    .unwrap();

    let gig_state = read_gig(&env.svm, &gig_key);
    assert_eq!(gig_state.status, GigStatus::Completed);
    assert_eq!(read_vault(&env.svm, &vault).active_milestone, 2);

    verify_vault_invariant(&env.svm, &vault, &vault_token_key);
}

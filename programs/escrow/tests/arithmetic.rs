mod common;

use common::*;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(400);
fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

#[test]
fn test_milestone_amount_zero() {
    let mut env = setup();
    let gig_id = next_id();
    let gig = init_gig(&mut env, gig_id);

    let (milestone, _) = milestone_pda(&gig, 0);
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_create_milestone(&env.client.pubkey(), &gig, &milestone, 0)],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err(), "create milestone with amount 0 must fail");
}

#[test]
fn test_milestone_amount_one() {
    let mut env = setup();
    let gig_id = next_id();
    let gig = init_gig(&mut env, gig_id);
    publish_and_assign(&mut env, &gig);
    let milestone = create_milestone_for(&mut env, &gig, 0, 1);

    let (vault, _) = vault_pda(&gig);
    let (vault_token_key, _) = vault_token_pda(&gig);
    let client_token =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.client.pubkey());
    let freelancer_token =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.freelancer.pubkey());

    mint_to(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.payer, &client_token, 1);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_fund_milestone(&FundAccounts {
            client: env.client.pubkey(),
            gig,
            milestone,
            vault,
            vault_token_account: vault_token_key,
            client_token_account: client_token,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    )
    .unwrap();

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &gig, &milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    send(
        &mut env.svm,
        &env.payer,
        &[ix_approve_milestone(&ReleaseAccounts {
            client: env.client.pubkey(),
            gig,
            milestone,
            vault,
            vault_token_account: vault_token_key,
            freelancer: env.freelancer.pubkey(),
            freelancer_token_account: freelancer_token,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    )
    .unwrap();

    assert_eq!(token_balance(&env.svm, &freelancer_token), 1);
    assert_eq!(token_balance(&env.svm, &vault_token_key), 0);
    verify_vault_invariant(&env.svm, &vault, &vault_token_key);
}

#[test]
fn test_milestone_large_amount() {
    let mut env = setup();
    let gig_id = next_id();
    let gig = init_gig(&mut env, gig_id);
    publish_and_assign(&mut env, &gig);
    let amount = 10_000_000_000_000u64;
    let milestone = create_milestone_for(&mut env, &gig, 0, amount);

    let (vault, _) = vault_pda(&gig);
    let (vault_token_key, _) = vault_token_pda(&gig);
    let client_token =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.client.pubkey());
    let freelancer_token =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.freelancer.pubkey());

    mint_to(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.payer, &client_token, amount);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_fund_milestone(&FundAccounts {
            client: env.client.pubkey(),
            gig,
            milestone,
            vault,
            vault_token_account: vault_token_key,
            client_token_account: client_token,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    )
    .unwrap();

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &gig, &milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    send(
        &mut env.svm,
        &env.payer,
        &[ix_approve_milestone(&ReleaseAccounts {
            client: env.client.pubkey(),
            gig,
            milestone,
            vault,
            vault_token_account: vault_token_key,
            freelancer: env.freelancer.pubkey(),
            freelancer_token_account: freelancer_token,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    )
    .unwrap();

    assert_eq!(token_balance(&env.svm, &freelancer_token), amount);
    assert_eq!(token_balance(&env.svm, &vault_token_key), 0);
    verify_vault_invariant(&env.svm, &vault, &vault_token_key);
}

#[test]
fn test_milestone_max_u64() {
    let mut env = setup();
    let gig_id = next_id();
    let gig = init_gig(&mut env, gig_id);
    publish_and_assign(&mut env, &gig);

    let (milestone, _) = milestone_pda(&gig, 0);
    send(
        &mut env.svm,
        &env.payer,
        &[ix_create_milestone(&env.client.pubkey(), &gig, &milestone, u64::MAX)],
        &[&env.payer, &env.client],
    )
    .unwrap();

    let ms = read_milestone(&env.svm, &milestone);
    assert_eq!(ms.amount, u64::MAX);
}

#[test]
fn test_multiple_milestones_large_total() {
    let mut env = setup();
    let gig_id = next_id();
    let gig = init_gig(&mut env, gig_id);
    publish_and_assign(&mut env, &gig);

    let amt = u64::MAX / 3 + 1;
    let m0 = create_milestone_for(&mut env, &gig, 0, amt);
    let m1 = create_milestone_for(&mut env, &gig, 1, amt);

    let (vault, _) = vault_pda(&gig);
    let (vault_token_key, _) = vault_token_pda(&gig);
    let client_token =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.client.pubkey());
    let freelancer_token =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.freelancer.pubkey());

    mint_to(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.payer, &client_token, amt);
    env.svm.expire_blockhash();
    send(
        &mut env.svm,
        &env.payer,
        &[ix_fund_milestone(&FundAccounts {
            client: env.client.pubkey(),
            gig,
            milestone: m0,
            vault,
            vault_token_account: vault_token_key,
            client_token_account: client_token,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    )
    .unwrap();

    env.svm.expire_blockhash();
    mint_to(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.payer, &client_token, amt);
    send(
        &mut env.svm,
        &env.payer,
        &[ix_fund_milestone(&FundAccounts {
            client: env.client.pubkey(),
            gig,
            milestone: m1,
            vault,
            vault_token_account: vault_token_key,
            client_token_account: client_token,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    )
    .unwrap();

    let total = amt + amt;
    let vault_state = read_vault(&env.svm, &vault);
    assert_eq!(vault_state.total_locked, total);
    verify_vault_invariant(&env.svm, &vault, &vault_token_key);

    for milestone in [&m0, &m1] {
        send(
            &mut env.svm,
            &env.payer,
            &[ix_submit_delivery(&env.freelancer.pubkey(), &gig, milestone)],
            &[&env.payer, &env.freelancer],
        )
        .unwrap();

        send(
            &mut env.svm,
            &env.payer,
            &[ix_approve_milestone(&ReleaseAccounts {
                client: env.client.pubkey(),
                gig,
                milestone: *milestone,
                vault,
                vault_token_account: vault_token_key,
                freelancer: env.freelancer.pubkey(),
                freelancer_token_account: freelancer_token,
                mint: env.mint.pubkey(),
            })],
            &[&env.payer, &env.client],
        )
        .unwrap();
    }

    assert_eq!(token_balance(&env.svm, &vault_token_key), 0);
    assert_eq!(token_balance(&env.svm, &freelancer_token), total);
    verify_vault_invariant(&env.svm, &vault, &vault_token_key);
}

#[test]
fn test_partial_timeout_percent_of_max() {
    let mut env = setup();
    let gig_id = next_id();
    let gig = init_gig(&mut env, gig_id);
    publish_and_assign(&mut env, &gig);
    let amount = u64::MAX / 1000;
    let milestone = create_milestone_for(&mut env, &gig, 0, amount);

    let (vault, _) = vault_pda(&gig);
    let (vault_token_key, _) = vault_token_pda(&gig);
    let client_token =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.client.pubkey());
    let freelancer_token =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.freelancer.pubkey());

    mint_to(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.payer, &client_token, amount);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_fund_milestone(&FundAccounts {
            client: env.client.pubkey(),
            gig,
            milestone,
            vault,
            vault_token_account: vault_token_key,
            client_token_account: client_token,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    )
    .unwrap();

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &gig, &milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    warp_seconds(&mut env.svm, 73 * 3600);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_partial_timeout_release(&TimeoutAccounts {
            gig,
            milestone,
            vault,
            vault_token_account: vault_token_key,
            freelancer_token_account: freelancer_token,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer],
    )
    .unwrap();

    let expected = amount * 20 / 100;
    let ms = read_milestone(&env.svm, &milestone);
    assert_eq!(ms.released, expected);
    assert_eq!(token_balance(&env.svm, &freelancer_token), expected);
    assert_eq!(
        token_balance(&env.svm, &vault_token_key),
        amount - expected
    );
    verify_vault_invariant(&env.svm, &vault, &vault_token_key);
}

#[test]
fn test_exact_accounting_after_many_operations() {
    let mut env = setup();
    let gig_id = next_id();
    let gig = init_gig(&mut env, gig_id);
    publish_and_assign(&mut env, &gig);

    let amounts = [100u64, 200, 300, 400, 500];
    let mut milestones = Vec::new();
    for (i, &amount) in amounts.iter().enumerate() {
        let m = create_milestone_for(&mut env, &gig, i as u32, amount);
        milestones.push(m);
    }

    let (vault, _) = vault_pda(&gig);
    let (vault_token_key, _) = vault_token_pda(&gig);
    let client_token =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.client.pubkey());
    let freelancer_token =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.freelancer.pubkey());

    let mut total_locked = 0u64;
    let mut total_released = 0u64;

    for (&amount, milestone) in amounts.iter().zip(&milestones) {
        mint_to(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.payer, &client_token, amount);

        send(
            &mut env.svm,
            &env.payer,
            &[ix_fund_milestone(&FundAccounts {
                client: env.client.pubkey(),
                gig,
                milestone: *milestone,
                vault,
                vault_token_account: vault_token_key,
                client_token_account: client_token,
                mint: env.mint.pubkey(),
            })],
            &[&env.payer, &env.client],
        )
        .unwrap();
        total_locked += amount;

        let vs = read_vault(&env.svm, &vault);
        assert_eq!(vs.total_locked, total_locked);
        verify_vault_invariant(&env.svm, &vault, &vault_token_key);

        send(
            &mut env.svm,
            &env.payer,
            &[ix_submit_delivery(&env.freelancer.pubkey(), &gig, milestone)],
            &[&env.payer, &env.freelancer],
        )
        .unwrap();

        send(
            &mut env.svm,
            &env.payer,
            &[ix_approve_milestone(&ReleaseAccounts {
                client: env.client.pubkey(),
                gig,
                milestone: *milestone,
                vault,
                vault_token_account: vault_token_key,
                freelancer: env.freelancer.pubkey(),
                freelancer_token_account: freelancer_token,
                mint: env.mint.pubkey(),
            })],
            &[&env.payer, &env.client],
        )
        .unwrap();
        total_released += amount;

        let vs = read_vault(&env.svm, &vault);
        assert_eq!(vs.total_released, total_released);
        assert_eq!(vs.total_locked, total_locked);
        assert_eq!(
            token_balance(&env.svm, &vault_token_key),
            total_locked - total_released
        );
        verify_vault_invariant(&env.svm, &vault, &vault_token_key);
    }

    let total = total_locked;
    assert_eq!(total_released, total);
    assert_eq!(token_balance(&env.svm, &vault_token_key), 0);
    assert_eq!(token_balance(&env.svm, &freelancer_token), total);

    let gig_state = read_gig(&env.svm, &gig);
    assert_eq!(gig_state.status, GigStatus::Completed);
}

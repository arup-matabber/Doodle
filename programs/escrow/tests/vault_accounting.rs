mod common;

use common::*;
use solana_signer::Signer;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(500);
fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

#[test]
fn test_invariant_after_fund() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_and_assign(&mut env, &gig);
    let milestone = create_milestone_for(&mut env, &gig, 0, STANDARD_AMOUNT);

    let (vault, _) = vault_pda(&gig);
    let (vault_token_key, _) = vault_token_pda(&gig);
    let client_token = create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.client.pubkey());
    create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.freelancer.pubkey());

    mint_to(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.payer, &client_token, STANDARD_AMOUNT);

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

    let v = read_vault(&env.svm, &vault);
    assert_eq!(v.total_locked, STANDARD_AMOUNT);
    assert_eq!(v.total_released, 0);
    assert_eq!(token_balance(&env.svm, &vault_token_key), STANDARD_AMOUNT);

    verify_vault_invariant(&env.svm, &vault, &vault_token_key);
}

#[test]
fn test_invariant_after_approve() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_and_assign(&mut env, &gig);
    let milestone = create_milestone_for(&mut env, &gig, 0, STANDARD_AMOUNT);

    let (vault, _) = vault_pda(&gig);
    let (vault_token_key, _) = vault_token_pda(&gig);
    let client_token = create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.client.pubkey());
    let freelancer_token =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.freelancer.pubkey());

    mint_to(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.payer, &client_token, STANDARD_AMOUNT);

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

    let v = read_vault(&env.svm, &vault);
    assert_eq!(v.total_locked, STANDARD_AMOUNT);
    assert_eq!(v.total_released, STANDARD_AMOUNT);
    assert_eq!(token_balance(&env.svm, &vault_token_key), 0);

    verify_vault_invariant(&env.svm, &vault, &vault_token_key);
}

#[test]
fn test_invariant_after_partial_timeout() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_and_assign(&mut env, &gig);
    let milestone = create_milestone_for(&mut env, &gig, 0, STANDARD_AMOUNT);

    let (vault, _) = vault_pda(&gig);
    let (vault_token_key, _) = vault_token_pda(&gig);
    let client_token = create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.client.pubkey());
    let freelancer_token =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.freelancer.pubkey());

    mint_to(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.payer, &client_token, STANDARD_AMOUNT);

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

    let expected_partial = STANDARD_AMOUNT * 20 / 100;
    let v = read_vault(&env.svm, &vault);
    assert_eq!(v.total_locked, STANDARD_AMOUNT);
    assert_eq!(v.total_released, expected_partial);
    assert_eq!(
        token_balance(&env.svm, &vault_token_key),
        STANDARD_AMOUNT - expected_partial
    );

    verify_vault_invariant(&env.svm, &vault, &vault_token_key);
}

#[test]
fn test_invariant_after_full_timeout() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_and_assign(&mut env, &gig);
    let milestone = create_milestone_for(&mut env, &gig, 0, STANDARD_AMOUNT);

    let (vault, _) = vault_pda(&gig);
    let (vault_token_key, _) = vault_token_pda(&gig);
    let client_token = create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.client.pubkey());
    let freelancer_token =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.freelancer.pubkey());

    mint_to(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.payer, &client_token, STANDARD_AMOUNT);

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

    warp_seconds(&mut env.svm, 7 * 86400);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_full_timeout_release(&TimeoutAccounts {
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

    let v = read_vault(&env.svm, &vault);
    assert_eq!(v.total_locked, STANDARD_AMOUNT);
    assert_eq!(v.total_released, STANDARD_AMOUNT);
    assert_eq!(token_balance(&env.svm, &vault_token_key), 0);

    verify_vault_invariant(&env.svm, &vault, &vault_token_key);
}

#[test]
fn test_invariant_multiple_milestones() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_and_assign(&mut env, &gig);

    let amount_a: u64 = 300_000;
    let amount_b: u64 = 700_000;

    let ms_a = create_milestone_for(&mut env, &gig, 0, amount_a);
    let ms_b = create_milestone_for(&mut env, &gig, 1, amount_b);

    let (vault, _) = vault_pda(&gig);
    let (vault_token_key, _) = vault_token_pda(&gig);
    let client_token = create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.client.pubkey());
    let freelancer_token =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.freelancer.pubkey());

    let total = amount_a + amount_b;
    mint_to(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.payer, &client_token, total);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_fund_milestone(&FundAccounts {
            client: env.client.pubkey(),
            gig,
            milestone: ms_a,
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
        &[ix_fund_milestone(&FundAccounts {
            client: env.client.pubkey(),
            gig,
            milestone: ms_b,
            vault,
            vault_token_account: vault_token_key,
            client_token_account: client_token,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    )
    .unwrap();

    let v = read_vault(&env.svm, &vault);
    assert_eq!(v.total_locked, total);
    assert_eq!(v.total_released, 0);
    assert_eq!(token_balance(&env.svm, &vault_token_key), total);
    verify_vault_invariant(&env.svm, &vault, &vault_token_key);

    // Submit + approve milestone A
    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &gig, &ms_a)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    send(
        &mut env.svm,
        &env.payer,
        &[ix_approve_milestone(&ReleaseAccounts {
            client: env.client.pubkey(),
            gig,
            milestone: ms_a,
            vault,
            vault_token_account: vault_token_key,
            freelancer: env.freelancer.pubkey(),
            freelancer_token_account: freelancer_token,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    )
    .unwrap();

    let v = read_vault(&env.svm, &vault);
    assert_eq!(v.total_released, amount_a);
    assert_eq!(token_balance(&env.svm, &vault_token_key), amount_b);
    verify_vault_invariant(&env.svm, &vault, &vault_token_key);

    // Submit + approve milestone B
    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &gig, &ms_b)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    send(
        &mut env.svm,
        &env.payer,
        &[ix_approve_milestone(&ReleaseAccounts {
            client: env.client.pubkey(),
            gig,
            milestone: ms_b,
            vault,
            vault_token_account: vault_token_key,
            freelancer: env.freelancer.pubkey(),
            freelancer_token_account: freelancer_token,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    )
    .unwrap();

    let v = read_vault(&env.svm, &vault);
    assert_eq!(v.total_released, total);
    assert_eq!(token_balance(&env.svm, &vault_token_key), 0);
    verify_vault_invariant(&env.svm, &vault, &vault_token_key);
}

#[test]
fn test_no_token_leakage() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_and_assign(&mut env, &gig);
    let milestone = create_milestone_for(&mut env, &gig, 0, STANDARD_AMOUNT);

    let (vault, _) = vault_pda(&gig);
    let (vault_token_key, _) = vault_token_pda(&gig);
    let client_token = create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.client.pubkey());
    let freelancer_token =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.freelancer.pubkey());

    mint_to(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.payer, &client_token, STANDARD_AMOUNT);

    let client_initial = token_balance(&env.svm, &client_token);
    let freelancer_initial = token_balance(&env.svm, &freelancer_token);
    assert_eq!(client_initial, STANDARD_AMOUNT);
    assert_eq!(freelancer_initial, 0);

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

    assert_eq!(token_balance(&env.svm, &client_token), 0);
    assert_eq!(token_balance(&env.svm, &vault_token_key), STANDARD_AMOUNT);
    assert_eq!(token_balance(&env.svm, &freelancer_token), 0);
    assert_eq!(
        token_balance(&env.svm, &client_token) + token_balance(&env.svm, &vault_token_key)
            + token_balance(&env.svm, &freelancer_token),
        STANDARD_AMOUNT,
        "tokens not conserved after fund"
    );

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &gig, &milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    assert_eq!(token_balance(&env.svm, &client_token), 0);
    assert_eq!(token_balance(&env.svm, &vault_token_key), STANDARD_AMOUNT);
    assert_eq!(token_balance(&env.svm, &freelancer_token), 0);

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

    assert_eq!(token_balance(&env.svm, &client_token), 0);
    assert_eq!(token_balance(&env.svm, &vault_token_key), 0);
    assert_eq!(token_balance(&env.svm, &freelancer_token), STANDARD_AMOUNT);

    let total_end = token_balance(&env.svm, &client_token)
        + token_balance(&env.svm, &vault_token_key)
        + token_balance(&env.svm, &freelancer_token);
    assert_eq!(
        total_end, STANDARD_AMOUNT,
        "tokens not conserved: client_spent = client_initial, freelancer_gained = freelancer_final"
    );
    assert_eq!(
        client_initial - token_balance(&env.svm, &client_token),
        token_balance(&env.svm, &freelancer_token) - freelancer_initial,
        "client spent != freelancer received"
    );
}

#[test]
fn test_vault_balances_after_cancel() {
    let mut env = setup();
    let id = next_id();
    let s = create_funded_milestone(&mut env, id, STANDARD_AMOUNT);

    verify_vault_invariant(&env.svm, &s.vault, &s.vault_token_account);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &s.gig, &s.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    verify_vault_invariant(&env.svm, &s.vault, &s.vault_token_account);

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

    let v = read_vault(&env.svm, &s.vault);
    assert_eq!(v.total_locked, v.total_released, "locked should equal released after full approve");
    assert_eq!(
        token_balance(&env.svm, &s.vault_token_account),
        0,
        "vault token balance should be zero after full release"
    );

    verify_vault_invariant(&env.svm, &s.vault, &s.vault_token_account);
}

#[test]
fn test_invariant_holds_through_complex_flow() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_and_assign(&mut env, &gig);

    let amount_a: u64 = 1000;
    let amount_b: u64 = 2000;

    let ms_a = create_milestone_for(&mut env, &gig, 0, amount_a);
    let ms_b = create_milestone_for(&mut env, &gig, 1, amount_b);

    let (vault, _) = vault_pda(&gig);
    let (vault_token_key, _) = vault_token_pda(&gig);
    let client_token = create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.client.pubkey());
    let freelancer_token =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.freelancer.pubkey());

    let total = amount_a + amount_b;
    mint_to(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.payer, &client_token, total);

    // Fund milestone A
    send(
        &mut env.svm,
        &env.payer,
        &[ix_fund_milestone(&FundAccounts {
            client: env.client.pubkey(),
            gig,
            milestone: ms_a,
            vault,
            vault_token_account: vault_token_key,
            client_token_account: client_token,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    )
    .unwrap();

    let v = read_vault(&env.svm, &vault);
    assert_eq!(v.total_locked, amount_a);
    assert_eq!(token_balance(&env.svm, &vault_token_key), amount_a);
    verify_vault_invariant(&env.svm, &vault, &vault_token_key);

    // Fund milestone B
    send(
        &mut env.svm,
        &env.payer,
        &[ix_fund_milestone(&FundAccounts {
            client: env.client.pubkey(),
            gig,
            milestone: ms_b,
            vault,
            vault_token_account: vault_token_key,
            client_token_account: client_token,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    )
    .unwrap();

    let v = read_vault(&env.svm, &vault);
    assert_eq!(v.total_locked, total);
    assert_eq!(v.total_released, 0);
    assert_eq!(token_balance(&env.svm, &vault_token_key), total);
    verify_vault_invariant(&env.svm, &vault, &vault_token_key);

    // Submit + approve milestone A
    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &gig, &ms_a)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    send(
        &mut env.svm,
        &env.payer,
        &[ix_approve_milestone(&ReleaseAccounts {
            client: env.client.pubkey(),
            gig,
            milestone: ms_a,
            vault,
            vault_token_account: vault_token_key,
            freelancer: env.freelancer.pubkey(),
            freelancer_token_account: freelancer_token,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    )
    .unwrap();

    let v = read_vault(&env.svm, &vault);
    assert_eq!(v.total_locked, total);
    assert_eq!(v.total_released, amount_a);
    assert_eq!(token_balance(&env.svm, &vault_token_key), amount_b);
    verify_vault_invariant(&env.svm, &vault, &vault_token_key);

    // Submit milestone B, warp 73h, partial timeout B (20% of 2000 = 400)
    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &gig, &ms_b)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    warp_seconds(&mut env.svm, 73 * 3600);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_partial_timeout_release(&TimeoutAccounts {
            gig,
            milestone: ms_b,
            vault,
            vault_token_account: vault_token_key,
            freelancer_token_account: freelancer_token,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer],
    )
    .unwrap();

    let partial_b = amount_b * 20 / 100;
    let v = read_vault(&env.svm, &vault);
    assert_eq!(v.total_locked, total);
    assert_eq!(v.total_released, amount_a + partial_b);
    assert_eq!(token_balance(&env.svm, &vault_token_key), total - (amount_a + partial_b));
    verify_vault_invariant(&env.svm, &vault, &vault_token_key);

    // Warp 7d from B submission, full timeout B releases remaining
    let elapsed_since_submit_b = 73 * 3600;
    let full_deadline = 7 * 86_400;
    warp_seconds(&mut env.svm, full_deadline - elapsed_since_submit_b + 1);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_full_timeout_release(&TimeoutAccounts {
            gig,
            milestone: ms_b,
            vault,
            vault_token_account: vault_token_key,
            freelancer_token_account: freelancer_token,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer],
    )
    .unwrap();

    let v = read_vault(&env.svm, &vault);
    assert_eq!(v.total_locked, total);
    assert_eq!(v.total_released, total);
    assert_eq!(token_balance(&env.svm, &vault_token_key), 0);
    verify_vault_invariant(&env.svm, &vault, &vault_token_key);
}

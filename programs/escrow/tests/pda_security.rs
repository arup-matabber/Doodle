mod common;

use common::*;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(600);
fn next_id() -> u64 { NEXT_ID.fetch_add(1, Ordering::Relaxed) }

#[test]
fn test_wrong_gig_pda() {
    let mut env = setup();
    let _real_gig = init_gig(&mut env, next_id());

    let (wrong_gig, _) = gig_pda(&env.client.pubkey(), next_id());
    let (milestone, _) = milestone_pda(&wrong_gig, 0);

    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_create_milestone(&env.client.pubkey(), &wrong_gig, &milestone, STANDARD_AMOUNT)],
        &[&env.payer, &env.client],
    )
    .unwrap_err();

    assert!(err.contains("0xbc4"), "Expected AccountNotInitialized, got: {err}");
}

#[test]
fn test_wrong_milestone_pda() {
    let mut env = setup();
    let gig = init_gig(&mut env, next_id());
    publish_and_assign(&mut env, &gig);
    let _real_milestone = create_milestone_for(&mut env, &gig, 0, STANDARD_AMOUNT);

    let (wrong_milestone, _) = milestone_pda(&gig, 999);
    let (vault, _) = vault_pda(&gig);
    let (vault_token, _) = vault_token_pda(&gig);
    let client_token = create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.client.pubkey());
    mint_to(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.payer, &client_token, STANDARD_AMOUNT);

    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_fund_milestone(&FundAccounts {
            client: env.client.pubkey(),
            gig,
            milestone: wrong_milestone,
            vault,
            vault_token_account: vault_token,
            client_token_account: client_token,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    )
    .unwrap_err();

    assert!(err.contains("0xbc4"), "Expected AccountNotInitialized, got: {err}");
}

#[test]
fn test_wrong_vault_pda() {
    let mut env = setup();
    let gig = init_gig(&mut env, next_id());
    publish_and_assign(&mut env, &gig);
    let milestone = create_milestone_for(&mut env, &gig, 0, STANDARD_AMOUNT);

    let wrong_key = Pubkey::new_unique();
    let (wrong_vault, _) = vault_pda(&wrong_key);
    let (wrong_vault_token, _) = vault_token_pda(&wrong_key);
    let client_token = create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.client.pubkey());
    mint_to(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.payer, &client_token, STANDARD_AMOUNT);

    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_fund_milestone(&FundAccounts {
            client: env.client.pubkey(),
            gig,
            milestone,
            vault: wrong_vault,
            vault_token_account: wrong_vault_token,
            client_token_account: client_token,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    )
    .unwrap_err();

    // The bogus vault PDA was never created, so Anchor rejects it as uninitialized
    // before it even gets a chance to compare seeds against a real vault.
    assert!(err.contains("0xbc4"), "Expected AccountNotInitialized, got: {err}");
}

#[test]
fn test_wrong_bump() {
    let mut env = setup();
    let s = create_funded_milestone(&mut env, next_id(), STANDARD_AMOUNT);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &s.gig, &s.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    let s_other = create_funded_milestone(&mut env, next_id(), STANDARD_AMOUNT);

    let ft = create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.freelancer.pubkey());

    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_approve_milestone(&ReleaseAccounts {
            client: env.client.pubkey(),
            gig: s.gig,
            milestone: s.milestone,
            vault: s_other.vault,
            vault_token_account: s_other.vault_token_account,
            freelancer: env.freelancer.pubkey(),
            freelancer_token_account: ft,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    )
    .unwrap_err();

    assert!(err.contains("0x7d6"), "Expected ConstraintSeeds, got: {err}");
}

#[test]
fn test_milestone_from_different_gig() {
    let mut env = setup();
    let gig_a = init_gig(&mut env, next_id());
    publish_and_assign(&mut env, &gig_a);
    let _milestone_a = create_milestone_for(&mut env, &gig_a, 0, STANDARD_AMOUNT);

    let gig_b = init_gig(&mut env, next_id());
    publish_and_assign(&mut env, &gig_b);
    let milestone_b = create_milestone_for(&mut env, &gig_b, 0, STANDARD_AMOUNT);

    let (vault_for_a, _) = vault_pda(&gig_a);
    let (vault_token_for_a, _) = vault_token_pda(&gig_a);
    let client_token = create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.client.pubkey());
    mint_to(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.payer, &client_token, STANDARD_AMOUNT);

    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_fund_milestone(&FundAccounts {
            client: env.client.pubkey(),
            gig: gig_a,
            milestone: milestone_b,
            vault: vault_for_a,
            vault_token_account: vault_token_for_a,
            client_token_account: client_token,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    )
    .unwrap_err();

    assert!(err.contains("0x1770"), "Expected Unauthorized (milestone.gig != gig.key()), got: {err}");
}

#[test]
fn test_vault_token_pda_mismatch() {
    let mut env = setup();
    let s = create_funded_milestone(&mut env, next_id(), STANDARD_AMOUNT);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &s.gig, &s.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    let wrong_token = create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &s.vault);
    let ft = create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.freelancer.pubkey());

    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_approve_milestone(&ReleaseAccounts {
            client: env.client.pubkey(),
            gig: s.gig,
            milestone: s.milestone,
            vault: s.vault,
            vault_token_account: wrong_token,
            freelancer: env.freelancer.pubkey(),
            freelancer_token_account: ft,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    )
    .unwrap_err();

    assert!(err.contains("0x1770"), "Expected Unauthorized (address != vault.token_account), got: {err}");
}

#[test]
fn test_vault_from_different_gig_in_approve() {
    let mut env = setup();
    let s_a = create_funded_milestone(&mut env, next_id(), STANDARD_AMOUNT);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &s_a.gig, &s_a.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    let s_b = create_funded_milestone(&mut env, next_id(), STANDARD_AMOUNT);

    let ft = create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.freelancer.pubkey());

    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_approve_milestone(&ReleaseAccounts {
            client: env.client.pubkey(),
            gig: s_a.gig,
            milestone: s_a.milestone,
            vault: s_b.vault,
            vault_token_account: s_b.vault_token_account,
            freelancer: env.freelancer.pubkey(),
            freelancer_token_account: ft,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    )
    .unwrap_err();

    assert!(err.contains("0x7d6"), "Expected ConstraintSeeds, got: {err}");
}

#[test]
fn test_wrong_gig_pda_on_publish() {
    let mut env = setup();
    let real_gig = init_gig(&mut env, next_id());
    let (fake_gig, _) = gig_pda(&env.client.pubkey(), next_id());

    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_publish_gig(&env.client.pubkey(), &fake_gig)],
        &[&env.payer, &env.client],
    )
    .unwrap_err();

    assert!(err.contains("0xbc4"), "Expected AccountNotInitialized, got: {err}");
}

#[test]
fn test_wrong_gig_pda_on_assign() {
    let mut env = setup();
    let real_gig = init_gig(&mut env, next_id());
    publish_gig(&mut env, &real_gig);
    let (fake_gig, _) = gig_pda(&env.client.pubkey(), next_id());

    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_assign_freelancer(&env.client.pubkey(), &env.freelancer.pubkey(), &fake_gig)],
        &[&env.payer, &env.client],
    )
    .unwrap_err();

    assert!(err.contains("0xbc4"), "Expected AccountNotInitialized, got: {err}");
}

#[test]
fn test_wrong_gig_pda_on_complete() {
    let mut env = setup();
    let real_gig = init_gig(&mut env, next_id());
    publish_and_assign(&mut env, &real_gig);
    let (fake_gig, _) = gig_pda(&env.client.pubkey(), next_id());

    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_complete_gig(&env.client.pubkey(), &fake_gig)],
        &[&env.payer, &env.client],
    )
    .unwrap_err();

    assert!(err.contains("0xbc4"), "Expected AccountNotInitialized, got: {err}");
}

#[test]
fn test_wrong_gig_pda_on_archive() {
    let mut env = setup();
    let real_gig = init_gig(&mut env, next_id());
    let freelancer = env.freelancer.pubkey();
    publish_gig(&mut env, &real_gig);
    assign_freelancer_to(&mut env, &real_gig, &freelancer);
    complete_gig_for(&mut env, &real_gig);
    let (fake_gig, _) = gig_pda(&env.client.pubkey(), next_id());

    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_archive_gig(&env.client.pubkey(), &fake_gig)],
        &[&env.payer, &env.client],
    )
    .unwrap_err();

    assert!(err.contains("0xbc4"), "Expected AccountNotInitialized, got: {err}");
}

#[test]
fn test_wrong_gig_pda_on_cancel() {
    let mut env = setup();
    let real_gig = init_gig(&mut env, next_id());
    let (fake_gig, _) = gig_pda(&env.client.pubkey(), next_id());

    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_cancel_gig(&env.client.pubkey(), &fake_gig)],
        &[&env.payer, &env.client],
    )
    .unwrap_err();

    assert!(err.contains("0xbc4"), "Expected AccountNotInitialized, got: {err}");
}

#[test]
fn test_spoofed_pda_not_initialized() {
    let mut env = setup();
    let gig = init_gig(&mut env, next_id());
    publish_and_assign(&mut env, &gig);
    let _real_milestone = create_milestone_for(&mut env, &gig, 0, STANDARD_AMOUNT);

    let (spoofed_milestone, _) = milestone_pda(&gig, 999);

    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &gig, &spoofed_milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap_err();

    assert!(err.contains("0xbc4"), "Expected AccountNotInitialized, got: {err}");
}

mod common;

use common::*;
use solana_keypair::Keypair;
use solana_signer::Signer;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
fn next_id() -> u64 { NEXT_ID.fetch_add(1, Ordering::Relaxed) }

#[test]
fn test_create_milestone_unauthorized() {
    let mut env = setup();
    let gig = init_gig(&mut env, next_id());

    let random = Keypair::new();
    env.svm.airdrop(&random.pubkey(), 1_000_000_000).unwrap();

    let (milestone, _) = milestone_pda(&gig, 0);
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_create_milestone(&random.pubkey(), &gig, &milestone, STANDARD_AMOUNT)],
        &[&env.payer, &random],
    );
    assert!(result.is_err());
}

#[test]
fn test_fund_milestone_by_random() {
    let mut env = setup();
    let gig = init_gig(&mut env, next_id());
    publish_and_assign(&mut env, &gig);
    let milestone = create_milestone_for(&mut env, &gig, 0, STANDARD_AMOUNT);

    let random = Keypair::new();
    env.svm.airdrop(&random.pubkey(), 1_000_000_000).unwrap();

    let (vault, _) = vault_pda(&gig);
    let (vault_token_account, _) = vault_token_pda(&gig);
    let client_token_account =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &random.pubkey());
    mint_to(
        &mut env.svm,
        &env.payer,
        &env.mint.pubkey(),
        &env.payer,
        &client_token_account,
        STANDARD_AMOUNT,
    );

    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_fund_milestone(&FundAccounts {
            client: random.pubkey(),
            gig,
            milestone,
            vault,
            vault_token_account,
            client_token_account,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &random],
    );
    assert!(result.is_err());
}

#[test]
fn test_submit_delivery_by_client() {
    let mut env = setup();
    let setup = create_funded_milestone(&mut env, next_id(), STANDARD_AMOUNT);

    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.client.pubkey(), &setup.gig, &setup.milestone)],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

#[test]
fn test_submit_delivery_by_random() {
    let mut env = setup();
    let setup = create_funded_milestone(&mut env, next_id(), STANDARD_AMOUNT);

    let random = Keypair::new();
    env.svm.airdrop(&random.pubkey(), 1_000_000_000).unwrap();

    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&random.pubkey(), &setup.gig, &setup.milestone)],
        &[&env.payer, &random],
    );
    assert!(result.is_err());
}

#[test]
fn test_approve_by_freelancer() {
    let mut env = setup();
    let setup = create_funded_milestone(&mut env, next_id(), STANDARD_AMOUNT);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &setup.gig, &setup.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_approve_milestone(&ReleaseAccounts {
            client: env.freelancer.pubkey(),
            gig: setup.gig,
            milestone: setup.milestone,
            vault: setup.vault,
            vault_token_account: setup.vault_token_account,
            freelancer: env.freelancer.pubkey(),
            freelancer_token_account: setup.freelancer_token_account,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.freelancer],
    );
    assert!(result.is_err());
}

#[test]
fn test_approve_by_random() {
    let mut env = setup();
    let setup = create_funded_milestone(&mut env, next_id(), STANDARD_AMOUNT);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &setup.gig, &setup.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    let random = Keypair::new();
    env.svm.airdrop(&random.pubkey(), 1_000_000_000).unwrap();

    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_approve_milestone(&ReleaseAccounts {
            client: random.pubkey(),
            gig: setup.gig,
            milestone: setup.milestone,
            vault: setup.vault,
            vault_token_account: setup.vault_token_account,
            freelancer: env.freelancer.pubkey(),
            freelancer_token_account: setup.freelancer_token_account,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &random],
    );
    assert!(result.is_err());
}

#[test]
fn test_cancel_by_random() {
    let mut env = setup();
    let gig = init_gig(&mut env, next_id());
    publish_and_assign(&mut env, &gig);
    let milestone = create_milestone_for(&mut env, &gig, 0, STANDARD_AMOUNT);

    let random = Keypair::new();
    env.svm.airdrop(&random.pubkey(), 1_000_000_000).unwrap();

    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_cancel_before_funding(&random.pubkey(), &gig, &milestone)],
        &[&env.payer, &random],
    );
    assert!(result.is_err());
}

#[test]
fn test_cancel_by_freelancer() {
    let mut env = setup();
    let gig = init_gig(&mut env, next_id());
    publish_and_assign(&mut env, &gig);
    let milestone = create_milestone_for(&mut env, &gig, 0, STANDARD_AMOUNT);

    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_cancel_before_funding(&env.freelancer.pubkey(), &gig, &milestone)],
        &[&env.payer, &env.freelancer],
    );
    assert!(result.is_err());
}

#[test]
fn test_assign_freelancer_client_eq_freelancer() {
    let mut env = setup();
    let gig = init_gig(&mut env, next_id());
    publish_gig(&mut env, &gig);

    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_assign_freelancer(&env.client.pubkey(), &env.client.pubkey(), &gig)],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

#[test]
fn test_publish_gig_unauthorized() {
    let mut env = setup();
    let gig = init_gig(&mut env, next_id());
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_publish_gig(&env.freelancer.pubkey(), &gig)],
        &[&env.payer, &env.freelancer],
    );
    assert!(result.is_err());
}

#[test]
fn test_assign_freelancer_unauthorized() {
    let mut env = setup();
    let gig = init_gig(&mut env, next_id());
    publish_gig(&mut env, &gig);
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_assign_freelancer(&env.freelancer.pubkey(), &env.freelancer.pubkey(), &gig)],
        &[&env.payer, &env.freelancer],
    );
    assert!(result.is_err());
}

#[test]
fn test_complete_gig_unauthorized() {
    let mut env = setup();
    let gig = init_gig(&mut env, next_id());
    let freelancer = env.freelancer.pubkey();
    publish_gig(&mut env, &gig);
    assign_freelancer_to(&mut env, &gig, &freelancer);
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_complete_gig(&freelancer, &gig)],
        &[&env.payer, &env.freelancer],
    );
    assert!(result.is_err());
}

#[test]
fn test_archive_gig_unauthorized() {
    let mut env = setup();
    let gig = init_gig(&mut env, next_id());
    let freelancer = env.freelancer.pubkey();
    publish_gig(&mut env, &gig);
    assign_freelancer_to(&mut env, &gig, &freelancer);
    complete_gig_for(&mut env, &gig);
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_archive_gig(&freelancer, &gig)],
        &[&env.payer, &env.freelancer],
    );
    assert!(result.is_err());
}

#[test]
fn test_cancel_gig_unauthorized() {
    let mut env = setup();
    let gig = init_gig(&mut env, next_id());
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_cancel_gig(&env.freelancer.pubkey(), &gig)],
        &[&env.payer, &env.freelancer],
    );
    assert!(result.is_err());
}

#[test]
fn test_partial_timeout_is_permissionless() {
    let mut env = setup();
    let setup = create_funded_milestone(&mut env, next_id(), STANDARD_AMOUNT);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &setup.gig, &setup.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    warp_seconds(&mut env.svm, 73 * 3_600);

    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_partial_timeout_release(&TimeoutAccounts {
            gig: setup.gig,
            milestone: setup.milestone,
            vault: setup.vault,
            vault_token_account: setup.vault_token_account,
            freelancer_token_account: setup.freelancer_token_account,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer],
    );
    assert!(result.is_ok());
}

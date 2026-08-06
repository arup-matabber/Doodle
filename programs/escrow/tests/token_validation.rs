mod common;

use common::*;
use solana_keypair::Keypair;
use solana_signer::Signer;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(700);
fn next_id() -> u64 { NEXT_ID.fetch_add(1, Ordering::Relaxed) }

#[test]
fn test_fund_with_wrong_mint() {
    let mut env = setup();
    let gig = init_gig(&mut env, next_id());
    publish_and_assign(&mut env, &gig);
    let milestone = create_milestone_for(&mut env, &gig, 0, STANDARD_AMOUNT);

    let wrong_mint = Keypair::new();
    create_mint(&mut env.svm, &env.payer, &wrong_mint, USDC_DECIMALS);

    let (vault, _) = vault_pda(&gig);
    let (vault_token_account, _) = vault_token_pda(&gig);
    let client_token_account =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.client.pubkey());
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
            client: env.client.pubkey(),
            gig,
            milestone,
            vault,
            vault_token_account,
            client_token_account,
            mint: wrong_mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

#[test]
fn test_fund_with_wrong_client_token_mint() {
    let mut env = setup();
    let gig = init_gig(&mut env, next_id());
    publish_and_assign(&mut env, &gig);
    let milestone = create_milestone_for(&mut env, &gig, 0, STANDARD_AMOUNT);

    let wrong_mint = Keypair::new();
    create_mint(&mut env.svm, &env.payer, &wrong_mint, USDC_DECIMALS);

    let (vault, _) = vault_pda(&gig);
    let (vault_token_account, _) = vault_token_pda(&gig);
    let client_token_account =
        create_token_account(&mut env.svm, &env.payer, &wrong_mint.pubkey(), &env.client.pubkey());

    let result = send(
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
    );
    assert!(result.is_err());
}

#[test]
fn test_fund_with_wrong_client_token_owner() {
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

    let result = send(
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
    );
    assert!(result.is_err());
}

#[test]
fn test_fund_with_empty_token_account() {
    let mut env = setup();
    let gig = init_gig(&mut env, next_id());
    publish_and_assign(&mut env, &gig);
    let milestone = create_milestone_for(&mut env, &gig, 0, STANDARD_AMOUNT);

    let (vault, _) = vault_pda(&gig);
    let (vault_token_account, _) = vault_token_pda(&gig);
    let client_token_account =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.client.pubkey());

    let result = send(
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
    );
    assert!(result.is_err());
}

#[test]
fn test_fund_with_insufficient_balance() {
    let mut env = setup();
    let gig = init_gig(&mut env, next_id());
    publish_and_assign(&mut env, &gig);
    let milestone = create_milestone_for(&mut env, &gig, 0, STANDARD_AMOUNT);

    let (vault, _) = vault_pda(&gig);
    let (vault_token_account, _) = vault_token_pda(&gig);
    let client_token_account =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.client.pubkey());
    mint_to(
        &mut env.svm,
        &env.payer,
        &env.mint.pubkey(),
        &env.payer,
        &client_token_account,
        STANDARD_AMOUNT / 2,
    );

    let result = send(
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
    );
    assert!(result.is_err());
}

#[test]
fn test_approve_with_wrong_freelancer_token_mint() {
    let mut env = setup();
    let s = create_funded_milestone(&mut env, next_id(), STANDARD_AMOUNT);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &s.gig, &s.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    let wrong_mint = Keypair::new();
    create_mint(&mut env.svm, &env.payer, &wrong_mint, USDC_DECIMALS);
    let wrong_freelancer_token =
        create_token_account(&mut env.svm, &env.payer, &wrong_mint.pubkey(), &env.freelancer.pubkey());

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
            freelancer_token_account: wrong_freelancer_token,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

#[test]
fn test_approve_with_wrong_freelancer_token_owner() {
    let mut env = setup();
    let s = create_funded_milestone(&mut env, next_id(), STANDARD_AMOUNT);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &s.gig, &s.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    let random = Keypair::new();
    env.svm.airdrop(&random.pubkey(), 1_000_000_000).unwrap();
    let wrong_freelancer_token =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &random.pubkey());

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
            freelancer_token_account: wrong_freelancer_token,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

#[test]
fn test_approve_wrong_vault_token_account() {
    let mut env = setup();
    let s = create_funded_milestone(&mut env, next_id(), STANDARD_AMOUNT);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &s.gig, &s.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    let random_vault_token =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &s.vault);

    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_approve_milestone(&ReleaseAccounts {
            client: env.client.pubkey(),
            gig: s.gig,
            milestone: s.milestone,
            vault: s.vault,
            vault_token_account: random_vault_token,
            freelancer: env.freelancer.pubkey(),
            freelancer_token_account: s.freelancer_token_account,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

#[test]
fn test_fund_wrong_vault_token_account() {
    let mut env = setup();
    let gig = init_gig(&mut env, next_id());
    publish_and_assign(&mut env, &gig);
    let milestone = create_milestone_for(&mut env, &gig, 0, STANDARD_AMOUNT);

    let (vault, _) = vault_pda(&gig);
    let wrong_vault_token =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &vault);
    let client_token_account =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.client.pubkey());
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
            client: env.client.pubkey(),
            gig,
            milestone,
            vault,
            vault_token_account: wrong_vault_token,
            client_token_account,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

#[test]
fn test_wrong_mint_in_gig_initialization() {
    let mut env = setup();
    let gig = init_gig(&mut env, next_id());
    publish_and_assign(&mut env, &gig);
    let milestone = create_milestone_for(&mut env, &gig, 0, STANDARD_AMOUNT);

    let wrong_mint = Keypair::new();
    create_mint(&mut env.svm, &env.payer, &wrong_mint, USDC_DECIMALS);

    let (vault, _) = vault_pda(&gig);
    let (vault_token_account, _) = vault_token_pda(&gig);
    let client_token_account =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.client.pubkey());
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
            client: env.client.pubkey(),
            gig,
            milestone,
            vault,
            vault_token_account,
            client_token_account,
            mint: wrong_mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

#[test]
fn test_approve_freelancer_rejects_wrong_destination() {
    let mut env = setup();
    let s = create_funded_milestone(&mut env, next_id(), STANDARD_AMOUNT);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &s.gig, &s.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    let client_owned_token =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.client.pubkey());

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
            freelancer_token_account: client_owned_token,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

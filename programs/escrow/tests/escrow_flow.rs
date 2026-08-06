mod common;

use common::*;
use solana_keypair::Keypair;
use solana_signer::Signer;

const AMOUNT: u64 = 1_000_000; // 1 USDC at 6 decimals

struct Funded {
    gig: solana_pubkey::Pubkey,
    milestone: solana_pubkey::Pubkey,
    vault: solana_pubkey::Pubkey,
    vault_token_account: solana_pubkey::Pubkey,
    freelancer_token_account: solana_pubkey::Pubkey,
}

fn init_create_fund(env: &mut Env, gig_id: u64) -> Funded {
    let gig = init_gig(env, gig_id);
    publish_and_assign(env, &gig);

    let (milestone, _) = milestone_pda(&gig, 0);
    send(
        &mut env.svm,
        &env.payer,
        &[ix_create_milestone(&env.client.pubkey(), &gig, &milestone, AMOUNT)],
        &[&env.payer, &env.client],
    )
    .unwrap();

    let (vault, _) = vault_pda(&gig);
    let (vault_token_account, _) = vault_token_pda(&gig);
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
        AMOUNT,
    );

    send(
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
    )
    .unwrap();

    Funded {
        gig,
        milestone,
        vault,
        vault_token_account,
        freelancer_token_account,
    }
}

#[test]
fn full_happy_path_initialize_create_fund_submit_approve() {
    let mut env = setup();
    let f = init_create_fund(&mut env, 1);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &f.gig, &f.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    send(
        &mut env.svm,
        &env.payer,
        &[ix_approve_milestone(&ReleaseAccounts {
            client: env.client.pubkey(),
            gig: f.gig,
            milestone: f.milestone,
            vault: f.vault,
            vault_token_account: f.vault_token_account,
            freelancer: env.freelancer.pubkey(),
            freelancer_token_account: f.freelancer_token_account,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    )
    .unwrap();

    assert_eq!(token_balance(&env.svm, &f.freelancer_token_account), AMOUNT);
    assert_eq!(token_balance(&env.svm, &f.vault_token_account), 0);
}

#[test]
fn double_approval_fails() {
    let mut env = setup();
    let f = init_create_fund(&mut env, 2);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &f.gig, &f.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    let approve_ix = ix_approve_milestone(&ReleaseAccounts {
        client: env.client.pubkey(),
        gig: f.gig,
        milestone: f.milestone,
        vault: f.vault,
        vault_token_account: f.vault_token_account,
        freelancer: env.freelancer.pubkey(),
        freelancer_token_account: f.freelancer_token_account,
        mint: env.mint.pubkey(),
    });

    send(&mut env.svm, &env.payer, &[approve_ix.clone()], &[&env.payer, &env.client]).unwrap();
    let result = send(&mut env.svm, &env.payer, &[approve_ix], &[&env.payer, &env.client]);
    assert!(result.is_err());
}

#[test]
fn double_funding_fails() {
    let mut env = setup();
    let f = init_create_fund(&mut env, 3);

    let extra_client_token_account =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.client.pubkey());
    mint_to(
        &mut env.svm,
        &env.payer,
        &env.mint.pubkey(),
        &env.payer,
        &extra_client_token_account,
        AMOUNT,
    );

    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_fund_milestone(&FundAccounts {
            client: env.client.pubkey(),
            gig: f.gig,
            milestone: f.milestone,
            vault: f.vault,
            vault_token_account: f.vault_token_account,
            client_token_account: extra_client_token_account,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

#[test]
fn unauthorized_submit_delivery_fails() {
    let mut env = setup();
    let f = init_create_fund(&mut env, 4);

    let stranger = Keypair::new();
    env.svm.airdrop(&stranger.pubkey(), 1_000_000_000).unwrap();

    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&stranger.pubkey(), &f.gig, &f.milestone)],
        &[&env.payer, &stranger],
    );
    assert!(result.is_err());
}

#[test]
fn unauthorized_approve_by_freelancer_fails() {
    let mut env = setup();
    let f = init_create_fund(&mut env, 5);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &f.gig, &f.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    // freelancer attempts to approve their own milestone instead of the client
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_approve_milestone(&ReleaseAccounts {
            client: env.freelancer.pubkey(),
            gig: f.gig,
            milestone: f.milestone,
            vault: f.vault,
            vault_token_account: f.vault_token_account,
            freelancer: env.freelancer.pubkey(),
            freelancer_token_account: f.freelancer_token_account,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.freelancer],
    );
    assert!(result.is_err());
}

#[test]
fn approve_before_submit_fails() {
    let mut env = setup();
    let f = init_create_fund(&mut env, 6);

    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_approve_milestone(&ReleaseAccounts {
            client: env.client.pubkey(),
            gig: f.gig,
            milestone: f.milestone,
            vault: f.vault,
            vault_token_account: f.vault_token_account,
            freelancer: env.freelancer.pubkey(),
            freelancer_token_account: f.freelancer_token_account,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

#[test]
fn fund_with_wrong_mint_fails() {
    let mut env = setup();
    let gig = init_gig(&mut env, 7);
    publish_and_assign(&mut env, &gig);

    let (milestone, _) = milestone_pda(&gig, 0);
    send(
        &mut env.svm,
        &env.payer,
        &[ix_create_milestone(&env.client.pubkey(), &gig, &milestone, AMOUNT)],
        &[&env.payer, &env.client],
    )
    .unwrap();

    let (vault, _) = vault_pda(&gig);
    let (vault_token_account, _) = vault_token_pda(&gig);

    let wrong_mint = Keypair::new();
    create_mint(&mut env.svm, &env.payer, &wrong_mint, USDC_DECIMALS);
    let wrong_client_token_account =
        create_token_account(&mut env.svm, &env.payer, &wrong_mint.pubkey(), &env.client.pubkey());
    mint_to(
        &mut env.svm,
        &env.payer,
        &wrong_mint.pubkey(),
        &env.payer,
        &wrong_client_token_account,
        AMOUNT,
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
            client_token_account: wrong_client_token_account,
            mint: wrong_mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

#[test]
fn cancel_before_funding_succeeds_and_double_cancel_fails() {
    let mut env = setup();
    let gig = init_gig(&mut env, 8);
    publish_and_assign(&mut env, &gig);

    let (milestone, _) = milestone_pda(&gig, 0);
    send(
        &mut env.svm,
        &env.payer,
        &[ix_create_milestone(&env.client.pubkey(), &gig, &milestone, AMOUNT)],
        &[&env.payer, &env.client],
    )
    .unwrap();

    let cancel_ix = ix_cancel_before_funding(&env.client.pubkey(), &gig, &milestone);
    send(&mut env.svm, &env.payer, &[cancel_ix], &[&env.payer, &env.client]).unwrap();

    assert!(env.svm.get_account(&milestone).is_none());
}

#[test]
fn cancel_after_funding_fails() {
    let mut env = setup();
    let f = init_create_fund(&mut env, 9);

    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_cancel_before_funding(&env.client.pubkey(), &f.gig, &f.milestone)],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

#[test]
fn partial_timeout_release_before_deadline_fails() {
    let mut env = setup();
    let f = init_create_fund(&mut env, 10);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &f.gig, &f.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_partial_timeout_release(&TimeoutAccounts {
            gig: f.gig,
            milestone: f.milestone,
            vault: f.vault,
            vault_token_account: f.vault_token_account,
            freelancer_token_account: f.freelancer_token_account,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer],
    );
    assert!(result.is_err());
}

#[test]
fn partial_then_full_timeout_release_flow() {
    let mut env = setup();
    let f = init_create_fund(&mut env, 11);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &f.gig, &f.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    // Anyone (here: the payer, unrelated to the gig) can trigger the timeout releases.
    warp_seconds(&mut env.svm, 73 * 3_600);
    send(
        &mut env.svm,
        &env.payer,
        &[ix_partial_timeout_release(&TimeoutAccounts {
            gig: f.gig,
            milestone: f.milestone,
            vault: f.vault,
            vault_token_account: f.vault_token_account,
            freelancer_token_account: f.freelancer_token_account,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer],
    )
    .unwrap();
    assert_eq!(token_balance(&env.svm, &f.freelancer_token_account), AMOUNT / 5);

    // full_timeout_release still too early relative to submission
    let too_early = send(
        &mut env.svm,
        &env.payer,
        &[ix_full_timeout_release(&TimeoutAccounts {
            gig: f.gig,
            milestone: f.milestone,
            vault: f.vault,
            vault_token_account: f.vault_token_account,
            freelancer_token_account: f.freelancer_token_account,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer],
    );
    assert!(too_early.is_err());

    env.svm.expire_blockhash();
    warp_seconds(&mut env.svm, 7 * 86_400);
    send(
        &mut env.svm,
        &env.payer,
        &[ix_full_timeout_release(&TimeoutAccounts {
            gig: f.gig,
            milestone: f.milestone,
            vault: f.vault,
            vault_token_account: f.vault_token_account,
            freelancer_token_account: f.freelancer_token_account,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer],
    )
    .unwrap();

    assert_eq!(token_balance(&env.svm, &f.freelancer_token_account), AMOUNT);
    assert_eq!(token_balance(&env.svm, &f.vault_token_account), 0);
}

/// The 7-day full release does not require the 72h partial to have run first.
/// See `regressions.rs::full_timeout_releases_dust_milestone` for why the old
/// `PartialReleased`-only gate could strand funds permanently.
#[test]
fn full_timeout_release_without_prior_partial_succeeds() {
    let mut env = setup();
    let f = init_create_fund(&mut env, 12);

    send(
        &mut env.svm,
        &env.payer,
        &[ix_submit_delivery(&env.freelancer.pubkey(), &f.gig, &f.milestone)],
        &[&env.payer, &env.freelancer],
    )
    .unwrap();

    warp_seconds(&mut env.svm, 8 * 86_400);
    send(
        &mut env.svm,
        &env.payer,
        &[ix_full_timeout_release(&TimeoutAccounts {
            gig: f.gig,
            milestone: f.milestone,
            vault: f.vault,
            vault_token_account: f.vault_token_account,
            freelancer_token_account: f.freelancer_token_account,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer],
    )
    .unwrap();

    assert_eq!(token_balance(&env.svm, &f.freelancer_token_account), AMOUNT);
    assert_eq!(token_balance(&env.svm, &f.vault_token_account), 0);
}

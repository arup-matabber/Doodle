mod common;

use common::*;
use solana_signer::Signer;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(600);
fn next_id() -> u64 { NEXT_ID.fetch_add(1, Ordering::Relaxed) }

#[test]
fn test_wrong_gig_pda_on_publish() {
    let mut env = setup();
    let _real_gig = init_gig(&mut env, next_id());
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
    let _real_gig = init_gig(&mut env, next_id());
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

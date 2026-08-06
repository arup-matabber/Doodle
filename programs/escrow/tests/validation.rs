mod common;

use common::*;
use solana_signer::Signer;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(1000);
fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

#[test]
fn test_create_milestone_fails_in_draft() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    let (m, _) = milestone_pda(&gig, 0);
    let r = send(
        &mut env.svm, &env.payer,
        &[ix_create_milestone(&env.client.pubkey(), &gig, &m, STANDARD_AMOUNT)],
        &[&env.payer, &env.client],
    );
    assert!(r.is_err());
}

#[test]
fn test_create_milestone_fails_in_published() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_gig(&mut env, &gig);
    let (m, _) = milestone_pda(&gig, 0);
    let r = send(
        &mut env.svm, &env.payer,
        &[ix_create_milestone(&env.client.pubkey(), &gig, &m, STANDARD_AMOUNT)],
        &[&env.payer, &env.client],
    );
    assert!(r.is_err());
}

#[test]
fn test_create_milestone_succeeds_in_assigned() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_and_assign(&mut env, &gig);
    create_milestone_for(&mut env, &gig, 0, STANDARD_AMOUNT);
    let ms = read_milestone(&env.svm, &milestone_pda(&gig, 0).0);
    assert_eq!(ms.amount, STANDARD_AMOUNT);
}

/// create_milestone must also succeed once the gig has moved InProgress (i.e. once a
/// prior milestone has already been funded) -- not just in the initial Assigned status.
#[test]
fn test_create_milestone_succeeds_in_in_progress() {
    let mut env = setup();
    let id = next_id();
    let s = create_funded_milestone(&mut env, id, STANDARD_AMOUNT);
    assert_eq!(read_gig(&env.svm, &s.gig).status, GigStatus::InProgress);

    create_milestone_for(&mut env, &s.gig, 1, STANDARD_AMOUNT);
    let ms = read_milestone(&env.svm, &milestone_pda(&s.gig, 1).0);
    assert_eq!(ms.amount, STANDARD_AMOUNT);
}

#[test]
fn test_create_milestone_fails_after_complete() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_and_assign(&mut env, &gig);
    complete_gig_for(&mut env, &gig);
    let (m, _) = milestone_pda(&gig, 0);
    let r = send(
        &mut env.svm, &env.payer,
        &[ix_create_milestone(&env.client.pubkey(), &gig, &m, STANDARD_AMOUNT)],
        &[&env.payer, &env.client],
    );
    assert!(r.is_err());
}

#[test]
fn test_create_milestone_fails_after_cancel() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_and_assign(&mut env, &gig);
    cancel_gig_for(&mut env, &gig);
    let (m, _) = milestone_pda(&gig, 0);
    let r = send(
        &mut env.svm, &env.payer,
        &[ix_create_milestone(&env.client.pubkey(), &gig, &m, STANDARD_AMOUNT)],
        &[&env.payer, &env.client],
    );
    assert!(r.is_err());
}

#[test]
fn test_create_milestone_zero_amount_fails() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_and_assign(&mut env, &gig);
    let (m, _) = milestone_pda(&gig, 0);
    let r = send(
        &mut env.svm, &env.payer,
        &[ix_create_milestone(&env.client.pubkey(), &gig, &m, 0)],
        &[&env.payer, &env.client],
    );
    assert!(r.is_err());
}

mod common;

use common::*;
use solana_signer::Signer;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
fn next_id() -> u64 { NEXT_ID.fetch_add(1, Ordering::Relaxed) }

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

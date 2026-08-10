mod common;

use common::*;
use solana_signer::Signer;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(700);
fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

#[test]
fn test_publish_gig_from_draft_succeeds() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_gig(&mut env, &gig);
    let g = read_gig(&env.svm, &gig);
    assert_eq!(g.status, GigStatus::Published);
}

#[test]
fn test_publish_gig_twice_fails() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_gig(&mut env, &gig);
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_publish_gig(&env.client.pubkey(), &gig)],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

#[test]
fn test_publish_gig_from_assigned_fails() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_and_assign(&mut env, &gig);
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_publish_gig(&env.client.pubkey(), &gig)],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

#[test]
fn test_publish_gig_unauthorized_fails() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_publish_gig(&env.freelancer.pubkey(), &gig)],
        &[&env.payer, &env.freelancer],
    );
    assert!(result.is_err());
}

#[test]
fn test_publish_gig_sets_updated_at() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    let before = env.svm.get_sysvar::<solana_clock::Clock>().unix_timestamp;
    publish_gig(&mut env, &gig);
    let after = env.svm.get_sysvar::<solana_clock::Clock>().unix_timestamp;
    let g = read_gig(&env.svm, &gig);
    assert!(g.updated_at >= before && g.updated_at <= after);
}

#[test]
fn test_publish_gig_preserves_metadata() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_gig(&mut env, &gig);
    let g = read_gig(&env.svm, &gig);
    assert_eq!(g.title, TEST_TITLE);
    assert_eq!(g.description, TEST_DESCRIPTION);
    assert_eq!(g.category, TEST_CATEGORY);
    assert_eq!(g.budget, TEST_BUDGET);
}

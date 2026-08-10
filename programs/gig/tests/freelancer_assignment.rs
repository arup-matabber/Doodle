mod common;

use common::*;
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(800);
fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

#[test]
fn test_assign_freelancer_succeeds() {
    let mut env = setup();
    let id = next_id();
    let freelancer = env.freelancer.pubkey();
    let gig = init_gig(&mut env, id);
    publish_gig(&mut env, &gig);
    assign_freelancer_to(&mut env, &gig, &freelancer);
    let g = read_gig(&env.svm, &gig);
    assert_eq!(g.status, GigStatus::Assigned);
    assert_eq!(g.freelancer, freelancer);
}

#[test]
fn test_assign_freelancer_twice_fails() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_and_assign(&mut env, &gig);
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_assign_freelancer(&env.client.pubkey(), &env.freelancer.pubkey(), &gig)],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

#[test]
fn test_assign_freelancer_from_draft_fails() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_assign_freelancer(&env.client.pubkey(), &env.freelancer.pubkey(), &gig)],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

#[test]
fn test_assign_freelancer_self_assign_fails() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
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
fn test_assign_freelancer_unauthorized_fails() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
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
fn test_assign_freelancer_sets_updated_at() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_gig(&mut env, &gig);
    let before = env.svm.get_sysvar::<solana_clock::Clock>().unix_timestamp;
    let freelancer = env.freelancer.pubkey();
    assign_freelancer_to(&mut env, &gig, &freelancer);
    let after = env.svm.get_sysvar::<solana_clock::Clock>().unix_timestamp;
    let g = read_gig(&env.svm, &gig);
    assert!(g.updated_at >= before && g.updated_at <= after);
}

#[test]
fn test_assign_freelancer_preserves_draft_metadata() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    let freelancer = env.freelancer.pubkey();
    publish_gig(&mut env, &gig);
    assign_freelancer_to(&mut env, &gig, &freelancer);
    let g = read_gig(&env.svm, &gig);
    assert_eq!(g.title, TEST_TITLE);
    assert_eq!(g.budget, TEST_BUDGET);
}

#[test]
fn test_assign_freelancer_to_different_pubkey() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_gig(&mut env, &gig);
    let other = Keypair::new();
    assign_freelancer_to(&mut env, &gig, &other.pubkey());
    let g = read_gig(&env.svm, &gig);
    assert_eq!(g.freelancer, other.pubkey());
}

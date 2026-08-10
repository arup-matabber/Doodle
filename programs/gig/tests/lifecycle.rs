mod common;

use common::*;
use solana_signer::Signer;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(900);
fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

#[test]
fn test_full_lifecycle_draft_to_archived() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    assert_eq!(read_gig(&env.svm, &gig).status, GigStatus::Draft);
    publish_gig(&mut env, &gig);
    assert_eq!(read_gig(&env.svm, &gig).status, GigStatus::Published);
    let freelancer = env.freelancer.pubkey();
    assign_freelancer_to(&mut env, &gig, &freelancer);
    assert_eq!(read_gig(&env.svm, &gig).status, GigStatus::Assigned);
    complete_gig_for(&mut env, &gig);
    assert_eq!(read_gig(&env.svm, &gig).status, GigStatus::Completed);
    archive_gig_for(&mut env, &gig);
    assert_eq!(read_gig(&env.svm, &gig).status, GigStatus::Archived);
}

#[test]
fn test_cancel_from_draft() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    cancel_gig_for(&mut env, &gig);
    assert_eq!(read_gig(&env.svm, &gig).status, GigStatus::Cancelled);
}

#[test]
fn test_cancel_from_published() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_gig(&mut env, &gig);
    cancel_gig_for(&mut env, &gig);
    assert_eq!(read_gig(&env.svm, &gig).status, GigStatus::Cancelled);
}

#[test]
fn test_cancel_from_assigned() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    let freelancer = env.freelancer.pubkey();
    publish_gig(&mut env, &gig);
    assign_freelancer_to(&mut env, &gig, &freelancer);
    cancel_gig_for(&mut env, &gig);
    assert_eq!(read_gig(&env.svm, &gig).status, GigStatus::Cancelled);
}

#[test]
fn test_cancel_from_completed_fails() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    let freelancer = env.freelancer.pubkey();
    publish_gig(&mut env, &gig);
    assign_freelancer_to(&mut env, &gig, &freelancer);
    complete_gig_for(&mut env, &gig);
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_cancel_gig(&env.client.pubkey(), &gig)],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

#[test]
fn test_cancel_from_archived_fails() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    let freelancer = env.freelancer.pubkey();
    publish_gig(&mut env, &gig);
    assign_freelancer_to(&mut env, &gig, &freelancer);
    complete_gig_for(&mut env, &gig);
    archive_gig_for(&mut env, &gig);
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_cancel_gig(&env.client.pubkey(), &gig)],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

#[test]
fn test_cancel_twice_fails() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    cancel_gig_for(&mut env, &gig);
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_cancel_gig(&env.client.pubkey(), &gig)],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

#[test]
fn test_complete_from_assigned_succeeds() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    let freelancer = env.freelancer.pubkey();
    publish_gig(&mut env, &gig);
    assign_freelancer_to(&mut env, &gig, &freelancer);
    complete_gig_for(&mut env, &gig);
    assert_eq!(read_gig(&env.svm, &gig).status, GigStatus::Completed);
}

#[test]
fn test_complete_from_draft_fails() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_complete_gig(&env.client.pubkey(), &gig)],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

#[test]
fn test_archive_from_completed_succeeds() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    let freelancer = env.freelancer.pubkey();
    publish_gig(&mut env, &gig);
    assign_freelancer_to(&mut env, &gig, &freelancer);
    complete_gig_for(&mut env, &gig);
    archive_gig_for(&mut env, &gig);
    assert_eq!(read_gig(&env.svm, &gig).status, GigStatus::Archived);
}

#[test]
fn test_archive_from_draft_fails() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_archive_gig(&env.client.pubkey(), &gig)],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

#[test]
fn test_archive_twice_fails() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    let freelancer = env.freelancer.pubkey();
    publish_gig(&mut env, &gig);
    assign_freelancer_to(&mut env, &gig, &freelancer);
    complete_gig_for(&mut env, &gig);
    archive_gig_for(&mut env, &gig);
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_archive_gig(&env.client.pubkey(), &gig)],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

#[test]
fn test_draft_to_published_to_cancelled_sequence() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_gig(&mut env, &gig);
    cancel_gig_for(&mut env, &gig);
    assert_eq!(read_gig(&env.svm, &gig).status, GigStatus::Cancelled);
}

#[test]
fn test_archive_fails_from_cancelled() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    cancel_gig_for(&mut env, &gig);
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_archive_gig(&env.client.pubkey(), &gig)],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}


mod common;

use common::*;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(500);
fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

#[test]
fn test_gig_created_with_draft_status() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    let g = read_gig(&env.svm, &gig);
    assert_eq!(g.status, GigStatus::Draft);
    assert_eq!(g.freelancer, Pubkey::default());
}

#[test]
fn test_gig_stores_all_metadata() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    let g = read_gig(&env.svm, &gig);
    assert_eq!(g.title, TEST_TITLE);
    assert_eq!(g.description, TEST_DESCRIPTION);
    assert_eq!(g.category, TEST_CATEGORY);
    assert_eq!(g.budget, TEST_BUDGET);
    assert_eq!(g.deadline, TEST_DEADLINE);
    assert!(g.created_at > 0);
    assert_eq!(g.updated_at, g.created_at);
}

#[test]
fn test_gig_created_twice_fails() {
    let mut env = setup();
    let id = next_id();
    init_gig(&mut env, id);
    let (dup_gig, _) = gig_pda(&env.client.pubkey(), id);
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_initialize_gig(
            &env.client.pubkey(),
            &env.mint.pubkey(),
            &dup_gig,
            id,
            TEST_TITLE.to_string(),
            TEST_DESCRIPTION.to_string(),
            TEST_CATEGORY.to_string(),
            TEST_BUDGET,
            TEST_DEADLINE,
        )],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

#[test]
fn test_create_gig_with_empty_title_fails() {
    let mut env = setup();
    let id = next_id();
    let (gig, _) = gig_pda(&env.client.pubkey(), id);
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_initialize_gig(
            &env.client.pubkey(),
            &env.mint.pubkey(),
            &gig,
            id,
            String::new(),
            TEST_DESCRIPTION.to_string(),
            TEST_CATEGORY.to_string(),
            TEST_BUDGET,
            TEST_DEADLINE,
        )],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

#[test]
fn test_create_gig_with_long_title_fails() {
    let mut env = setup();
    let id = next_id();
    let (gig, _) = gig_pda(&env.client.pubkey(), id);
    let long = "x".repeat(101);
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_initialize_gig(
            &env.client.pubkey(),
            &env.mint.pubkey(),
            &gig,
            id,
            long,
            TEST_DESCRIPTION.to_string(),
            TEST_CATEGORY.to_string(),
            TEST_BUDGET,
            TEST_DEADLINE,
        )],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

#[test]
fn test_create_gig_with_max_title_fine() {
    let mut env = setup();
    let id = next_id();
    let (gig, _) = gig_pda(&env.client.pubkey(), id);
    let long = "x".repeat(100);
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_initialize_gig(
            &env.client.pubkey(),
            &env.mint.pubkey(),
            &gig,
            id,
            long,
            TEST_DESCRIPTION.to_string(),
            TEST_CATEGORY.to_string(),
            TEST_BUDGET,
            TEST_DEADLINE,
        )],
        &[&env.payer, &env.client],
    );
    assert!(result.is_ok(), "max title length should succeed");
}

#[test]
fn test_create_gig_with_long_description_fails() {
    let mut env = setup();
    let id = next_id();
    let (gig, _) = gig_pda(&env.client.pubkey(), id);
    let long = "x".repeat(501);
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_initialize_gig(
            &env.client.pubkey(),
            &env.mint.pubkey(),
            &gig,
            id,
            TEST_TITLE.to_string(),
            long,
            TEST_CATEGORY.to_string(),
            TEST_BUDGET,
            TEST_DEADLINE,
        )],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

#[test]
fn test_create_gig_with_long_category_fails() {
    let mut env = setup();
    let id = next_id();
    let (gig, _) = gig_pda(&env.client.pubkey(), id);
    let long = "x".repeat(51);
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_initialize_gig(
            &env.client.pubkey(),
            &env.mint.pubkey(),
            &gig,
            id,
            TEST_TITLE.to_string(),
            TEST_DESCRIPTION.to_string(),
            long,
            TEST_BUDGET,
            TEST_DEADLINE,
        )],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

#[test]
fn test_create_gig_with_zero_budget_fails() {
    let mut env = setup();
    let id = next_id();
    let (gig, _) = gig_pda(&env.client.pubkey(), id);
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_initialize_gig(
            &env.client.pubkey(),
            &env.mint.pubkey(),
            &gig,
            id,
            TEST_TITLE.to_string(),
            TEST_DESCRIPTION.to_string(),
            TEST_CATEGORY.to_string(),
            0,
            TEST_DEADLINE,
        )],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

#[test]
fn test_create_gig_with_deadline_in_past_fails() {
    let mut env = setup();
    let id = next_id();
    let (gig, _) = gig_pda(&env.client.pubkey(), id);
    let clock = env.svm.get_sysvar::<solana_clock::Clock>();
    let in_the_past = clock.unix_timestamp - 1;
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_initialize_gig(
            &env.client.pubkey(),
            &env.mint.pubkey(),
            &gig,
            id,
            TEST_TITLE.to_string(),
            TEST_DESCRIPTION.to_string(),
            TEST_CATEGORY.to_string(),
            TEST_BUDGET,
            in_the_past,
        )],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

#[test]
fn test_create_gig_with_deadline_too_soon_fails() {
    let mut env = setup();
    let id = next_id();
    let (gig, _) = gig_pda(&env.client.pubkey(), id);
    let clock = env.svm.get_sysvar::<solana_clock::Clock>();
    let too_soon = clock.unix_timestamp + 86_399; // one second less than MIN_DEADLINE_SECS
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_initialize_gig(
            &env.client.pubkey(),
            &env.mint.pubkey(),
            &gig,
            id,
            TEST_TITLE.to_string(),
            TEST_DESCRIPTION.to_string(),
            TEST_CATEGORY.to_string(),
            TEST_BUDGET,
            too_soon,
        )],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

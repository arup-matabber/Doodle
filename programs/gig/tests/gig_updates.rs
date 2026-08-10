mod common;

use common::*;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(600);
fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

#[test]
fn test_update_gig_title() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    let new_title = "Updated Gig Title".to_string();
    send(
        &mut env.svm,
        &env.payer,
        &[ix_update_gig(
            &env.client.pubkey(),
            &gig,
            new_title.clone(),
            TEST_DESCRIPTION.to_string(),
            String::new(),
            TEST_CATEGORY.to_string(),
            TEST_BUDGET,
            TEST_DEADLINE,
        )],
        &[&env.payer, &env.client],
    )
    .unwrap();
    let g = read_gig(&env.svm, &gig);
    assert_eq!(g.title, new_title);
}

#[test]
fn test_update_gig_description() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    let new_desc = "Updated description.".to_string();
    send(
        &mut env.svm,
        &env.payer,
        &[ix_update_gig(
            &env.client.pubkey(),
            &gig,
            TEST_TITLE.to_string(),
            new_desc.clone(),
            String::new(),
            TEST_CATEGORY.to_string(),
            TEST_BUDGET,
            TEST_DEADLINE,
        )],
        &[&env.payer, &env.client],
    )
    .unwrap();
    let g = read_gig(&env.svm, &gig);
    assert_eq!(g.description, new_desc);
}

#[test]
fn test_update_gig_skills() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    let skills = "Rust, Solana, DeFi".to_string();
    send(
        &mut env.svm,
        &env.payer,
        &[ix_update_gig(
            &env.client.pubkey(),
            &gig,
            TEST_TITLE.to_string(),
            TEST_DESCRIPTION.to_string(),
            skills.clone(),
            TEST_CATEGORY.to_string(),
            TEST_BUDGET,
            TEST_DEADLINE,
        )],
        &[&env.payer, &env.client],
    )
    .unwrap();
    let g = read_gig(&env.svm, &gig);
    assert_eq!(g.skills, skills);
}

#[test]
fn test_update_gig_category() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    let new_cat = "Design".to_string();
    send(
        &mut env.svm,
        &env.payer,
        &[ix_update_gig(
            &env.client.pubkey(),
            &gig,
            TEST_TITLE.to_string(),
            TEST_DESCRIPTION.to_string(),
            String::new(),
            new_cat.clone(),
            TEST_BUDGET,
            TEST_DEADLINE,
        )],
        &[&env.payer, &env.client],
    )
    .unwrap();
    let g = read_gig(&env.svm, &gig);
    assert_eq!(g.category, new_cat);
}

#[test]
fn test_update_gig_budget() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    let new_budget = 20_000_000u64;
    send(
        &mut env.svm,
        &env.payer,
        &[ix_update_gig(
            &env.client.pubkey(),
            &gig,
            TEST_TITLE.to_string(),
            TEST_DESCRIPTION.to_string(),
            String::new(),
            TEST_CATEGORY.to_string(),
            new_budget,
            TEST_DEADLINE,
        )],
        &[&env.payer, &env.client],
    )
    .unwrap();
    let g = read_gig(&env.svm, &gig);
    assert_eq!(g.budget, new_budget);
}

#[test]
fn test_update_gig_deadline() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    let new_deadline = TEST_DEADLINE + 86400;
    send(
        &mut env.svm,
        &env.payer,
        &[ix_update_gig(
            &env.client.pubkey(),
            &gig,
            TEST_TITLE.to_string(),
            TEST_DESCRIPTION.to_string(),
            String::new(),
            TEST_CATEGORY.to_string(),
            TEST_BUDGET,
            new_deadline,
        )],
        &[&env.payer, &env.client],
    )
    .unwrap();
    let g = read_gig(&env.svm, &gig);
    assert_eq!(g.deadline, new_deadline);
}

#[test]
fn test_update_gig_sets_updated_at() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    let before = env.svm.get_sysvar::<solana_clock::Clock>().unix_timestamp;
    send(
        &mut env.svm,
        &env.payer,
        &[ix_update_gig(
            &env.client.pubkey(),
            &gig,
            "New Title".to_string(),
            TEST_DESCRIPTION.to_string(),
            String::new(),
            TEST_CATEGORY.to_string(),
            TEST_BUDGET,
            TEST_DEADLINE,
        )],
        &[&env.payer, &env.client],
    )
    .unwrap();
    let after = env.svm.get_sysvar::<solana_clock::Clock>().unix_timestamp;
    let g = read_gig(&env.svm, &gig);
    assert!(g.updated_at >= before && g.updated_at <= after);
    assert!(g.updated_at > g.created_at || g.updated_at == g.created_at);
}

#[test]
fn test_update_gig_fails_after_published() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_gig(&mut env, &gig);
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_update_gig(
            &env.client.pubkey(),
            &gig,
            "Hacked".to_string(),
            TEST_DESCRIPTION.to_string(),
            String::new(),
            TEST_CATEGORY.to_string(),
            TEST_BUDGET,
            TEST_DEADLINE,
        )],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

#[test]
fn test_update_gig_long_title_fails() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_update_gig(
            &env.client.pubkey(),
            &gig,
            "x".repeat(101),
            TEST_DESCRIPTION.to_string(),
            String::new(),
            TEST_CATEGORY.to_string(),
            TEST_BUDGET,
            TEST_DEADLINE,
        )],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

#[test]
fn test_update_gig_long_skills_fails() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_update_gig(
            &env.client.pubkey(),
            &gig,
            TEST_TITLE.to_string(),
            TEST_DESCRIPTION.to_string(),
            "x".repeat(201),
            TEST_CATEGORY.to_string(),
            TEST_BUDGET,
            TEST_DEADLINE,
        )],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

#[test]
fn test_update_gig_zero_budget_fails() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_update_gig(
            &env.client.pubkey(),
            &gig,
            TEST_TITLE.to_string(),
            TEST_DESCRIPTION.to_string(),
            String::new(),
            TEST_CATEGORY.to_string(),
            0,
            TEST_DEADLINE,
        )],
        &[&env.payer, &env.client],
    );
    assert!(result.is_err());
}

#[test]
fn test_update_gig_unauthorized_fails() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    let result = send(
        &mut env.svm,
        &env.payer,
        &[ix_update_gig(
            &env.freelancer.pubkey(),
            &gig,
            "Hacked".to_string(),
            TEST_DESCRIPTION.to_string(),
            String::new(),
            TEST_CATEGORY.to_string(),
            TEST_BUDGET,
            TEST_DEADLINE,
        )],
        &[&env.payer, &env.freelancer],
    );
    assert!(result.is_err());
}

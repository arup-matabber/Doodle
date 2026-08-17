mod common;

use common::*;
use solana_clock::Clock;
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;

#[test]
fn test_create_profile() {
    let mut env = setup();
    let freelancer = env.freelancer.insecure_clone();
    let freelancer_pk = freelancer.pubkey();

    let clock_before = env.svm.get_sysvar::<Clock>().unix_timestamp;
    let profile_key = init_profile(&mut env, &freelancer);
    let clock_after = env.svm.get_sysvar::<Clock>().unix_timestamp;

    let (expected_pda, expected_bump) = profile_pda(&freelancer_pk);
    assert_eq!(profile_key, expected_pda);

    let profile = read_profile(&env.svm, &profile_key);
    assert_eq!(profile.authority, freelancer_pk);
    assert_eq!(profile.completed_jobs, 0);
    assert_eq!(profile.successful_jobs, 0);
    assert_eq!(profile.cancelled_jobs, 0);
    assert_eq!(profile.total_earnings, 0);
    assert_eq!(profile.rating_sum, 0);
    assert_eq!(profile.rating_count, 0);
    assert_eq!(profile.average_rating, 0);
    assert_eq!(profile.reputation_score, 0);
    assert_eq!(profile.badges_earned, 0);
    assert!(profile.created_at >= clock_before && profile.created_at <= clock_after);
    assert_eq!(profile.updated_at, profile.created_at);
    assert_eq!(profile.bump, expected_bump);

    assert_invariants(&profile);
}

#[test]
fn test_cannot_create_twice() {
    let mut env = setup();
    let freelancer = env.freelancer.insecure_clone();
    init_profile(&mut env, &freelancer);

    env.svm.expire_blockhash();
    let (profile, _) = profile_pda(&freelancer.pubkey());
    let result = send(
        &mut env.svm,
        &env.payer.insecure_clone(),
        &[ix_initialize_profile(&freelancer.pubkey(), &profile)],
        &[&env.payer.insecure_clone(), &freelancer],
    );
    assert!(result.is_err(), "duplicate profile should fail");
}

#[test]
fn test_correct_pda() {
    let mut env = setup();
    let freelancer = env.freelancer.insecure_clone();
    let freelancer_pk = freelancer.pubkey();
    let profile_key = init_profile(&mut env, &freelancer);

    let (expected_pda, _) = profile_pda(&freelancer_pk);
    assert_eq!(profile_key, expected_pda);

    let profile = read_profile(&env.svm, &profile_key);
    let (computed_pda, computed_bump) =
        Pubkey::find_program_address(&[b"profile", freelancer_pk.as_ref()], &reputation::ID);
    assert_eq!(profile_key, computed_pda);
    assert_eq!(profile.bump, computed_bump);
}

#[test]
fn test_correct_authority() {
    let mut env = setup();
    let freelancer = env.freelancer.insecure_clone();
    let freelancer_pk = freelancer.pubkey();
    let profile_key = init_profile(&mut env, &freelancer);

    let profile = read_profile(&env.svm, &profile_key);
    assert_eq!(profile.authority, freelancer_pk);
}

#[test]
fn test_correct_timestamps() {
    let mut env = setup();
    let freelancer = env.freelancer.insecure_clone();

    let clock_before = env.svm.get_sysvar::<Clock>().unix_timestamp;
    let profile_key = init_profile(&mut env, &freelancer);
    let clock_after = env.svm.get_sysvar::<Clock>().unix_timestamp;

    let profile = read_profile(&env.svm, &profile_key);
    assert!(profile.created_at >= clock_before);
    assert!(profile.created_at <= clock_after);
    assert_eq!(profile.updated_at, profile.created_at);
}

#[test]
fn test_correct_default_values() {
    let mut env = setup();
    let freelancer = env.freelancer.insecure_clone();
    let profile_key = init_profile(&mut env, &freelancer);

    let profile = read_profile(&env.svm, &profile_key);
    assert_eq!(profile.completed_jobs, 0);
    assert_eq!(profile.successful_jobs, 0);
    assert_eq!(profile.cancelled_jobs, 0);
    assert_eq!(profile.total_earnings, 0);
    assert_eq!(profile.rating_sum, 0);
    assert_eq!(profile.rating_count, 0);
    assert_eq!(profile.average_rating, 0);
    assert_eq!(profile.reputation_score, 0);
    assert_eq!(profile.badges_earned, 0);
}

#[test]
fn test_wrong_pda_rejected() {
    let mut env = setup();
    let freelancer = env.freelancer.insecure_clone();
    let freelancer_pk = freelancer.pubkey();

    let wrong_profile = Keypair::new();
    let result = send(
        &mut env.svm,
        &env.payer.insecure_clone(),
        &[ix_initialize_profile(&freelancer_pk, &wrong_profile.pubkey())],
        &[&env.payer.insecure_clone(), &freelancer],
    );
    expect_send_error(result.unwrap_err(), ERR_CONSTRAINT_SEEDS, "wrong profile PDA should fail");
}

#[test]
fn test_wrong_signer_creates_different_pda() {
    let mut env = setup();
    let client = env.client.insecure_clone();
    let client_pk = client.pubkey();
    let freelancer = env.freelancer.insecure_clone();
    let freelancer_pk = freelancer.pubkey();
    init_profile(&mut env, &client);

    let (client_profile_pda, _) = profile_pda(&client_pk);
    let (freelancer_profile_pda, _) = profile_pda(&freelancer_pk);
    assert_ne!(client_profile_pda, freelancer_profile_pda);

    // The client's profile exists
    assert!(account_exists(&env.svm, &client_profile_pda));
    // The freelancer's profile does not (yet)
    assert!(!account_exists(&env.svm, &freelancer_profile_pda));

    let profile = read_profile(&env.svm, &client_profile_pda);
    assert_eq!(profile.authority, client_pk);
}

#[test]
fn test_profile_bump_is_correct() {
    let mut env = setup();
    let freelancer = env.freelancer.insecure_clone();
    let profile_key = init_profile(&mut env, &freelancer);

    let (_, expected_bump) = profile_pda(&freelancer.pubkey());
    let profile = read_profile(&env.svm, &profile_key);
    assert_eq!(profile.bump, expected_bump);
}

#[test]
fn test_different_authority_profiles_are_independent() {
    let mut env = setup();
    let freelancer = env.freelancer.insecure_clone();
    let client = env.client.insecure_clone();

    let f_profile = init_profile(&mut env, &freelancer);
    let c_profile = init_profile(&mut env, &client);

    assert_ne!(f_profile, c_profile);

    let f_data = read_profile(&env.svm, &f_profile);
    let c_data = read_profile(&env.svm, &c_profile);
    assert_eq!(f_data.authority, freelancer.pubkey());
    assert_eq!(c_data.authority, client.pubkey());
}

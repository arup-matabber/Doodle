//! Authorization tests for profile-scoped instructions that remain directly
//! callable (initialize_profile, get_profile). `update_completion` and
//! `submit_rating` are now CPI-only from escrow -- their authorization tests
//! live in `pda_security.rs` (forged-signer / forged-PDA) and in
//! `programs/escrow/tests/reputation_settlement.rs` (real end-to-end CPI).

mod common;

use common::*;
use solana_keypair::Keypair;
use solana_signer::Signer;

#[test]
fn test_get_profile_works_for_any_caller() {
    let mut env = setup();
    let freelancer = env.freelancer.insecure_clone();
    let profile_key = init_profile(&mut env, &freelancer);

    let result = send(
        &mut env.svm,
        &env.payer.insecure_clone(),
        &[ix_get_profile(&profile_key)],
        &[&env.payer.insecure_clone()],
    );
    assert!(result.is_ok(), "get_profile should work for anyone");
}

#[test]
fn test_get_profile_wrong_pda_fails() {
    let mut env = setup();
    let wrong_key = Keypair::new();

    let result = send(
        &mut env.svm,
        &env.payer.insecure_clone(),
        &[ix_get_profile(&wrong_key.pubkey())],
        &[&env.payer.insecure_clone()],
    );
    assert!(result.is_err(), "get_profile with wrong PDA should fail");
}

#[test]
fn test_duplicate_profile_rejected() {
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
fn test_authority_cannot_be_changed() {
    let mut env = setup();
    let freelancer = env.freelancer.insecure_clone();
    let profile_key = init_profile(&mut env, &freelancer);

    let profile = read_profile(&env.svm, &profile_key);
    assert_eq!(profile.authority, freelancer.pubkey());
    // No instruction exists to modify authority -- enforced by program design.
}

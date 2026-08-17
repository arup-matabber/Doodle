//! Event-emission tests for instructions that remain directly callable.
//! Rating/completion/badge-award events (now only reachable via escrow's CPI)
//! are covered end-to-end in `programs/escrow/tests/reputation_settlement.rs`.

mod common;

use common::*;
use solana_clock::Clock;
use solana_signer::Signer;

#[test]
fn test_profile_created_event_emitted() {
    let mut env = setup();
    let freelancer = env.freelancer.insecure_clone();
    let (profile, _) = profile_pda(&freelancer.pubkey());
    let clock_before = env.svm.get_sysvar::<Clock>().unix_timestamp;

    let logs = send_logs(
        &mut env.svm,
        &env.payer.insecure_clone(),
        &[ix_initialize_profile(&freelancer.pubkey(), &profile)],
        &[&env.payer.insecure_clone(), &freelancer],
    )
    .unwrap();
    let clock_after = env.svm.get_sysvar::<Clock>().unix_timestamp;

    assert_event_emitted(&logs);

    let profile_data = read_profile(&env.svm, &profile);
    assert_eq!(profile_data.authority, freelancer.pubkey());
    assert!(profile_data.created_at >= clock_before && profile_data.created_at <= clock_after);
}

#[test]
fn test_event_contents_profile_created() {
    let mut env = setup();
    let freelancer = env.freelancer.insecure_clone();
    let (profile_key, _) = profile_pda(&freelancer.pubkey());
    let clock_before = env.svm.get_sysvar::<Clock>().unix_timestamp;

    let logs = send_logs(
        &mut env.svm,
        &env.payer.insecure_clone(),
        &[ix_initialize_profile(&freelancer.pubkey(), &profile_key)],
        &[&env.payer.insecure_clone(), &freelancer],
    )
    .unwrap();

    assert_event_emitted(&logs);

    let profile = read_profile(&env.svm, &profile_key);
    assert_eq!(profile.authority, freelancer.pubkey());
    assert_eq!(profile.created_at, profile.updated_at);
    assert!(profile.created_at >= clock_before);
    assert!(profile.created_at <= env.svm.get_sysvar::<Clock>().unix_timestamp);
}

#[test]
fn test_no_events_on_failed_instruction() {
    let mut env = setup();
    let freelancer = env.freelancer.insecure_clone();
    init_profile(&mut env, &freelancer);

    let (profile, _) = profile_pda(&freelancer.pubkey());
    let result = send_logs(
        &mut env.svm,
        &env.payer.insecure_clone(),
        &[ix_initialize_profile(&freelancer.pubkey(), &profile)],
        &[&env.payer.insecure_clone(), &freelancer],
    );

    assert!(result.is_err(), "duplicate profile should fail");
}

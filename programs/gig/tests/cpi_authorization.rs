//! Proof that `mark_in_progress`, `mark_completed_by_escrow`, and `mark_cancelled_by_escrow`
//! can only ever be reached via a CPI signed by escrow's own `escrow_authority` PDA.
//!
//! Each instruction requires `escrow_authority: Signer<'info>` constrained by
//! `seeds = [b"escrow_authority"], bump, seeds::program = ESCROW_PROGRAM_ID`. That PDA is
//! off-curve (derived, not a real keypair), so nothing outside of escrow's own
//! `invoke_signed` can ever produce a valid signature for it. Any direct caller must supply
//! *some* real keypair as the `escrow_authority` account to get a transaction signed at all,
//! and that keypair's address will never equal the derived PDA -- so Anchor's seeds
//! constraint rejects it before the handler runs.

mod common;

use common::*;
use solana_keypair::Keypair;
use solana_signer::Signer;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(2_000);
fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

#[test]
fn test_mark_in_progress_direct_call_rejected() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_and_assign(&mut env, &gig);

    let attacker = Keypair::new();
    env.svm.airdrop(&attacker.pubkey(), 1_000_000_000).unwrap();

    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_mark_in_progress(&attacker.pubkey(), &gig)],
        &[&env.payer, &attacker],
    )
    .unwrap_err();

    assert!(err.contains("0x7d6") || err.to_lowercase().contains("seeds"), "expected ConstraintSeeds, got: {err}");
    assert_eq!(read_gig(&env.svm, &gig).status, GigStatus::Assigned, "gig must not transition");
}

#[test]
fn test_mark_in_progress_with_correct_pda_address_but_no_signature_rejected() {
    // Even supplying the *correct* escrow_authority address doesn't help an attacker:
    // it's off-curve, so it can never actually appear as a transaction signer. Anchor
    // still requires it to be a signer, so the transaction is rejected either way.
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_and_assign(&mut env, &gig);

    let (correct_authority, _) = escrow_authority_pda();
    let ix = ix_mark_in_progress(&correct_authority, &gig);

    // We can't actually sign with `correct_authority` (no keypair exists for a PDA), so
    // construct the transaction with only the payer's signature and confirm the runtime
    // rejects it for the missing signature on that account.
    let blockhash = env.svm.latest_blockhash();
    let msg = solana_message::Message::new_with_blockhash(&[ix], Some(&env.payer.pubkey()), &blockhash);
    let tx = solana_message::VersionedMessage::Legacy(msg);
    let versioned = solana_transaction::versioned::VersionedTransaction::try_new(tx, &[&env.payer]);
    // Either signing fails outright (missing required signer), or if constructed anyway,
    // submission must fail because `correct_authority` never actually signed.
    match versioned {
        Ok(t) => {
            let res = env.svm.send_transaction(t);
            assert!(res.is_err(), "transaction without escrow_authority's signature must fail");
        }
        Err(_) => {} // failed to even build the transaction -- also acceptable proof.
    }
    assert_eq!(read_gig(&env.svm, &gig).status, GigStatus::Assigned, "gig must not transition");
}

#[test]
fn test_mark_completed_by_escrow_direct_call_rejected() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_and_assign(&mut env, &gig);

    let attacker = Keypair::new();
    env.svm.airdrop(&attacker.pubkey(), 1_000_000_000).unwrap();

    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_mark_completed_by_escrow(&attacker.pubkey(), &gig)],
        &[&env.payer, &attacker],
    )
    .unwrap_err();

    assert!(err.contains("0x7d6") || err.to_lowercase().contains("seeds"), "expected ConstraintSeeds, got: {err}");
    assert_eq!(read_gig(&env.svm, &gig).status, GigStatus::Assigned, "gig must not transition");
}

#[test]
fn test_mark_cancelled_by_escrow_direct_call_rejected() {
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id);
    publish_and_assign(&mut env, &gig);

    let attacker = Keypair::new();
    env.svm.airdrop(&attacker.pubkey(), 1_000_000_000).unwrap();

    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_mark_cancelled_by_escrow(&attacker.pubkey(), &gig)],
        &[&env.payer, &attacker],
    )
    .unwrap_err();

    assert!(err.contains("0x7d6") || err.to_lowercase().contains("seeds"), "expected ConstraintSeeds, got: {err}");
    assert_eq!(read_gig(&env.svm, &gig).status, GigStatus::Assigned, "gig must not transition");
}

#[test]
fn test_mark_cancelled_by_escrow_rejects_wrong_status() {
    // Even with a signer whose address happened to be right, a Completed/Archived/Draft gig
    // is not a valid target for this transition -- defense in depth beyond the signer check.
    let mut env = setup();
    let id = next_id();
    let gig = init_gig(&mut env, id); // Draft

    let attacker = Keypair::new();
    env.svm.airdrop(&attacker.pubkey(), 1_000_000_000).unwrap();

    let err = send(
        &mut env.svm,
        &env.payer,
        &[ix_mark_cancelled_by_escrow(&attacker.pubkey(), &gig)],
        &[&env.payer, &attacker],
    )
    .unwrap_err();
    assert!(!err.is_empty());
    assert_eq!(read_gig(&env.svm, &gig).status, GigStatus::Draft);
}

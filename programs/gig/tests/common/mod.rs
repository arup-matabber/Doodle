#![allow(dead_code)]

use anchor_lang::{
    solana_program::instruction::Instruction, AccountDeserialize, InstructionData, ToAccountMetas,
};
use litesvm::LiteSVM;
use solana_clock::Clock;
use solana_keypair::Keypair;
use solana_message::{Message, VersionedMessage};
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;

pub use gig::state::{Gig, GigStatus};

pub const TEST_TITLE: &str = "Test Gig";
pub const TEST_DESCRIPTION: &str = "A test gig for integration testing";
pub const TEST_SKILLS: &str = "Rust,Solana";
pub const TEST_CATEGORY: &str = "Development";
pub const TEST_BUDGET: u64 = 10_000_000;
pub const TEST_DEADLINE: i64 = 2_000_000_000; // far future, always > now + 86400

pub struct Env {
    pub svm: LiteSVM,
    pub payer: Keypair,
    pub client: Keypair,
    pub freelancer: Keypair,
    pub mint: Keypair,
}

pub fn setup() -> Env {
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../../target/deploy/gig.so");
    svm.add_program(gig::ID, bytes).unwrap();

    let payer = Keypair::new();
    let client = Keypair::new();
    let freelancer = Keypair::new();
    let mint = Keypair::new();

    for kp in [&payer, &client, &freelancer] {
        svm.airdrop(&kp.pubkey(), 10_000_000_000).unwrap();
    }

    create_mint(&mut svm, &payer, &mint, 6);

    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp = 1_700_000_000;
    svm.set_sysvar::<Clock>(&clock);

    Env { svm, payer, client, freelancer, mint }
}

pub fn send(svm: &mut LiteSVM, payer: &Keypair, ixs: &[Instruction], signers: &[&Keypair]) -> Result<(), String> {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(ixs, Some(&payer.pubkey()), &blockhash);
    let mut all_signers = vec![payer];
    for s in signers {
        if s.pubkey() != payer.pubkey() {
            all_signers.push(s);
        }
    }
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &all_signers).unwrap();
    svm.send_transaction(tx).map(|_| ()).map_err(|e| e.err.to_string())
}

pub fn send_logs(svm: &mut LiteSVM, payer: &Keypair, ixs: &[Instruction], signers: &[&Keypair]) -> Result<Vec<String>, String> {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(ixs, Some(&payer.pubkey()), &blockhash);
    let mut all_signers = vec![payer];
    for s in signers {
        if s.pubkey() != payer.pubkey() {
            all_signers.push(s);
        }
    }
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &all_signers).unwrap();
    let meta = svm.send_transaction(tx).map_err(|e| e.err.to_string())?;
    Ok(meta.logs)
}

pub fn create_mint(svm: &mut LiteSVM, payer: &Keypair, mint: &Keypair, decimals: u8) {
    use anchor_lang::solana_program::{program_pack::Pack, system_instruction};
    use spl_token_interface as spl_token;

    let rent = svm.minimum_balance_for_rent_exemption(spl_token::state::Mint::LEN);
    let create_ix = system_instruction::create_account(
        &payer.pubkey(),
        &mint.pubkey(),
        rent,
        spl_token::state::Mint::LEN as u64,
        &spl_token::ID,
    );
    let init_ix = spl_token::instruction::initialize_mint2(
        &spl_token::ID,
        &mint.pubkey(),
        &payer.pubkey(),
        None,
        decimals,
    )
    .unwrap();
    send(svm, payer, &[create_ix, init_ix], &[payer, mint]).unwrap();
}

pub fn warp_seconds(svm: &mut LiteSVM, seconds: i64) {
    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp = clock.unix_timestamp.saturating_add(seconds);
    svm.set_sysvar::<Clock>(&clock);
}

// ── Account deserialization ──

pub fn read_gig(svm: &LiteSVM, key: &Pubkey) -> Gig {
    let data = svm.get_account(key).unwrap().data;
    Gig::try_deserialize(&mut &data[..]).unwrap()
}

// ── PDA derivation ──

pub fn gig_pda(client: &Pubkey, id: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"gig", client.as_ref(), id.to_le_bytes().as_ref()],
        &gig::ID,
    )
}

/// The *correctly derived* escrow_authority PDA under gig's hardcoded ESCROW_PROGRAM_ID.
/// Only escrow's program itself can ever produce a valid signature for this address
/// (it is off-curve, so no keypair exists for it) -- it is included here purely so
/// tests can show what "correct" looks like when asserting that nothing else can reach it.
pub fn escrow_authority_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"escrow_authority"], &gig::ESCROW_PROGRAM_ID)
}

// ── Instruction builders ──

pub fn ix_initialize_gig(
    client: &Pubkey,
    mint: &Pubkey,
    gig: &Pubkey,
    id: u64,
    title: String,
    description: String,
    category: String,
    budget: u64,
    deadline: i64,
) -> Instruction {
    Instruction::new_with_bytes(
        gig::ID,
        &gig::instruction::InitializeGig { id, title, description, category, budget, deadline }.data(),
        gig::accounts::InitializeGig {
            client: *client,
            mint: *mint,
            gig: *gig,
            system_program: anchor_lang::solana_program::system_program::ID,
        }
        .to_account_metas(None),
    )
}

pub fn ix_update_gig(
    client: &Pubkey,
    gig: &Pubkey,
    title: String,
    description: String,
    skills: String,
    category: String,
    budget: u64,
    deadline: i64,
) -> Instruction {
    Instruction::new_with_bytes(
        gig::ID,
        &gig::instruction::UpdateGig { title, description, skills, category, budget, deadline }.data(),
        gig::accounts::UpdateGig { client: *client, gig: *gig }.to_account_metas(None),
    )
}

pub fn ix_publish_gig(client: &Pubkey, gig: &Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        gig::ID,
        &gig::instruction::PublishGig {}.data(),
        gig::accounts::PublishGig { client: *client, gig: *gig }.to_account_metas(None),
    )
}

pub fn ix_assign_freelancer(client: &Pubkey, freelancer: &Pubkey, gig: &Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        gig::ID,
        &gig::instruction::AssignFreelancer {}.data(),
        gig::accounts::AssignFreelancer { client: *client, freelancer: *freelancer, gig: *gig }.to_account_metas(None),
    )
}

pub fn ix_complete_gig(client: &Pubkey, gig: &Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        gig::ID,
        &gig::instruction::CompleteGig {}.data(),
        gig::accounts::CompleteGig { client: *client, gig: *gig }.to_account_metas(None),
    )
}

pub fn ix_archive_gig(client: &Pubkey, gig: &Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        gig::ID,
        &gig::instruction::ArchiveGig {}.data(),
        gig::accounts::ArchiveGig { client: *client, gig: *gig }.to_account_metas(None),
    )
}

pub fn ix_cancel_gig(client: &Pubkey, gig: &Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        gig::ID,
        &gig::instruction::CancelGig {}.data(),
        gig::accounts::CancelGig { client: *client, gig: *gig }.to_account_metas(None),
    )
}

/// Direct (non-CPI) call into the escrow-only instruction. `escrow_authority` is whatever
/// pubkey the caller supplies -- tests use this to prove that only the real PDA (which no
/// keypair anywhere can sign for outside of escrow's own CPI) is accepted.
pub fn ix_mark_in_progress(escrow_authority: &Pubkey, gig: &Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        gig::ID,
        &gig::instruction::MarkInProgress {}.data(),
        gig::accounts::MarkInProgress { escrow_authority: *escrow_authority, gig: *gig }.to_account_metas(None),
    )
}

pub fn ix_mark_completed_by_escrow(escrow_authority: &Pubkey, gig: &Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        gig::ID,
        &gig::instruction::MarkCompletedByEscrow {}.data(),
        gig::accounts::MarkCompletedByEscrow { escrow_authority: *escrow_authority, gig: *gig }.to_account_metas(None),
    )
}

pub fn ix_mark_cancelled_by_escrow(escrow_authority: &Pubkey, gig: &Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        gig::ID,
        &gig::instruction::MarkCancelledByEscrow {}.data(),
        gig::accounts::MarkCancelledByEscrow { escrow_authority: *escrow_authority, gig: *gig }.to_account_metas(None),
    )
}

// ── Test harness helpers ──

pub fn init_gig(env: &mut Env, gig_id: u64) -> Pubkey {
    let (gig, _) = gig_pda(&env.client.pubkey(), gig_id);
    send(
        &mut env.svm,
        &env.payer,
        &[ix_initialize_gig(
            &env.client.pubkey(),
            &env.mint.pubkey(),
            &gig,
            gig_id,
            TEST_TITLE.to_string(),
            TEST_DESCRIPTION.to_string(),
            TEST_CATEGORY.to_string(),
            TEST_BUDGET,
            TEST_DEADLINE,
        )],
        &[&env.payer, &env.client],
    )
    .unwrap();
    gig
}

pub fn publish_gig(env: &mut Env, gig: &Pubkey) {
    send(&mut env.svm, &env.payer, &[ix_publish_gig(&env.client.pubkey(), gig)], &[&env.payer, &env.client]).unwrap();
}

pub fn assign_freelancer_to(env: &mut Env, gig: &Pubkey, freelancer: &Pubkey) {
    send(
        &mut env.svm,
        &env.payer,
        &[ix_assign_freelancer(&env.client.pubkey(), freelancer, gig)],
        &[&env.payer, &env.client],
    )
    .unwrap();
}

pub fn publish_and_assign(env: &mut Env, gig: &Pubkey) {
    let freelancer = env.freelancer.pubkey();
    publish_gig(env, gig);
    assign_freelancer_to(env, gig, &freelancer);
}

pub fn complete_gig_for(env: &mut Env, gig: &Pubkey) {
    send(&mut env.svm, &env.payer, &[ix_complete_gig(&env.client.pubkey(), gig)], &[&env.payer, &env.client]).unwrap();
}

pub fn archive_gig_for(env: &mut Env, gig: &Pubkey) {
    send(&mut env.svm, &env.payer, &[ix_archive_gig(&env.client.pubkey(), gig)], &[&env.payer, &env.client]).unwrap();
}

pub fn cancel_gig_for(env: &mut Env, gig: &Pubkey) {
    send(&mut env.svm, &env.payer, &[ix_cancel_gig(&env.client.pubkey(), gig)], &[&env.payer, &env.client]).unwrap();
}

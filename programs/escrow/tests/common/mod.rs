#![allow(dead_code)]

use anchor_lang::{
    solana_program::{instruction::Instruction, program_pack::Pack, system_instruction},
    AccountDeserialize, InstructionData, ToAccountMetas,
};
use litesvm::LiteSVM;
use solana_clock::Clock;
use solana_keypair::Keypair;
use solana_message::{Message, VersionedMessage};
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;
use spl_token_interface as spl_token;

pub use escrow::state::{EscrowVault, Milestone, MilestoneStatus};
pub use gig::state::{Gig, GigStatus};
pub use reputation::state::UserProfile;

pub const USDC_DECIMALS: u8 = 6;
pub const STANDARD_AMOUNT: u64 = 1_000_000;

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
    let escrow_bytes = include_bytes!("../../../../target/deploy/escrow.so");
    svm.add_program(escrow::ID, escrow_bytes).unwrap();
    let gig_bytes = include_bytes!("../../../../target/deploy/gig.so");
    svm.add_program(gig::ID, gig_bytes).unwrap();
    let reputation_bytes = include_bytes!("../../../../target/deploy/reputation.so");
    svm.add_program(reputation::ID, reputation_bytes).unwrap();

    let payer = Keypair::new();
    let client = Keypair::new();
    let freelancer = Keypair::new();
    let mint = Keypair::new();

    for kp in [&payer, &client, &freelancer] {
        svm.airdrop(&kp.pubkey(), 10_000_000_000).unwrap();
    }

    create_mint(&mut svm, &payer, &mint, USDC_DECIMALS);

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

pub fn create_token_account(svm: &mut LiteSVM, payer: &Keypair, mint: &Pubkey, owner: &Pubkey) -> Pubkey {
    let account = Keypair::new();
    let rent = svm.minimum_balance_for_rent_exemption(spl_token::state::Account::LEN);
    let create_ix = system_instruction::create_account(
        &payer.pubkey(),
        &account.pubkey(),
        rent,
        spl_token::state::Account::LEN as u64,
        &spl_token::ID,
    );
    let init_ix = spl_token::instruction::initialize_account3(&spl_token::ID, &account.pubkey(), mint, owner)
        .unwrap();
    send(svm, payer, &[create_ix, init_ix], &[payer, &account]).unwrap();
    account.pubkey()
}

pub fn mint_to(svm: &mut LiteSVM, payer: &Keypair, mint: &Pubkey, mint_authority: &Keypair, dest: &Pubkey, amount: u64) {
    let ix = spl_token::instruction::mint_to(
        &spl_token::ID,
        mint,
        dest,
        &mint_authority.pubkey(),
        &[],
        amount,
    )
    .unwrap();
    send(svm, payer, &[ix], &[payer, mint_authority]).unwrap();
}

pub fn token_balance(svm: &LiteSVM, account: &Pubkey) -> u64 {
    let data = svm.get_account(account).unwrap().data;
    spl_token::state::Account::unpack(&data).unwrap().amount
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

pub fn read_milestone(svm: &LiteSVM, key: &Pubkey) -> Milestone {
    let data = svm.get_account(key).unwrap().data;
    Milestone::try_deserialize(&mut &data[..]).unwrap()
}

pub fn read_vault(svm: &LiteSVM, key: &Pubkey) -> EscrowVault {
    let data = svm.get_account(key).unwrap().data;
    EscrowVault::try_deserialize(&mut &data[..]).unwrap()
}

// ── Accounting invariants ──

pub fn verify_vault_invariant(svm: &LiteSVM, vault_key: &Pubkey, vault_token_key: &Pubkey) {
    let vault = read_vault(svm, vault_key);
    let vault_balance = token_balance(svm, vault_token_key);
    assert_eq!(
        vault.total_locked,
        vault.total_released + vault_balance,
        "vault accounting invariant violated: locked {} != released {} + balance {}",
        vault.total_locked,
        vault.total_released,
        vault_balance,
    );
}

// ── PDA derivation ──

pub fn gig_pda(client: &Pubkey, id: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"gig", client.as_ref(), id.to_le_bytes().as_ref()],
        &gig::ID,
    )
}

pub fn milestone_pda(gig: &Pubkey, index: u32) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"milestone", gig.as_ref(), index.to_le_bytes().as_ref()], &escrow::ID)
}

pub fn vault_pda(gig: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"vault", gig.as_ref()], &escrow::ID)
}

pub fn vault_token_pda(gig: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"vault", gig.as_ref(), b"token"], &escrow::ID)
}

/// Escrow's own CPI-signer PDA, used to authorize calls into the gig program.
pub fn escrow_authority_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"escrow_authority"], &escrow::ID)
}

pub fn reputation_profile_pda(authority: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"profile", authority.as_ref()], &reputation::ID)
}

pub fn reputation_rating_pda(client: &Pubkey, job_id: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"rating", client.as_ref(), job_id.to_le_bytes().as_ref()],
        &reputation::ID,
    )
}

pub fn read_reputation_profile(svm: &LiteSVM, key: &Pubkey) -> UserProfile {
    let data = svm.get_account(key).unwrap().data;
    UserProfile::try_deserialize(&mut &data[..]).unwrap()
}

pub fn ix_initialize_reputation_profile(authority: &Pubkey, profile: &Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        reputation::ID,
        &reputation::instruction::InitializeProfile {}.data(),
        reputation::accounts::InitializeProfile {
            authority: *authority,
            profile: *profile,
            system_program: anchor_lang::solana_program::system_program::ID,
        }
        .to_account_metas(None),
    )
}

pub fn init_reputation_profile(env: &mut Env, authority: &Keypair) -> Pubkey {
    let (profile, _) = reputation_profile_pda(&authority.pubkey());
    send(
        &mut env.svm,
        &env.payer,
        &[ix_initialize_reputation_profile(&authority.pubkey(), &profile)],
        &[&env.payer, authority],
    )
    .unwrap();
    profile
}

pub fn ix_settle_reputation(gig: &Pubkey, vault: &Pubkey, freelancer_profile: &Pubkey) -> Instruction {
    let (escrow_authority, _) = escrow_authority_pda();
    Instruction::new_with_bytes(
        escrow::ID,
        &escrow::instruction::SettleReputation {}.data(),
        escrow::accounts::SettleReputation {
            gig: *gig,
            vault: *vault,
            escrow_authority,
            freelancer_profile: *freelancer_profile,
            reputation_program: reputation::ID,
        }
        .to_account_metas(None),
    )
}

pub struct RateFreelancerAccounts {
    pub client: Pubkey,
    pub gig: Pubkey,
    pub vault: Pubkey,
    pub freelancer: Pubkey,
    pub freelancer_profile: Pubkey,
    pub rating: Pubkey,
}

pub fn ix_rate_freelancer(a: &RateFreelancerAccounts, score: u8, review_hash: [u8; 32]) -> Instruction {
    let (escrow_authority, _) = escrow_authority_pda();
    Instruction::new_with_bytes(
        escrow::ID,
        &escrow::instruction::RateFreelancer { score, review_hash }.data(),
        escrow::accounts::RateFreelancer {
            client: a.client,
            gig: a.gig,
            vault: a.vault,
            freelancer: a.freelancer,
            freelancer_profile: a.freelancer_profile,
            rating: a.rating,
            escrow_authority,
            reputation_program: reputation::ID,
            system_program: anchor_lang::solana_program::system_program::ID,
        }
        .to_account_metas(None),
    )
}

// ── Gig instruction builders (calls into the `gig` program) ──

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

// ── Escrow instruction builders ──

pub fn ix_create_milestone(client: &Pubkey, gig: &Pubkey, milestone: &Pubkey, amount: u64) -> Instruction {
    let (vault, _) = vault_pda(gig);
    Instruction::new_with_bytes(
        escrow::ID,
        &escrow::instruction::CreateMilestone { amount }.data(),
        escrow::accounts::CreateMilestone {
            client: *client,
            gig: *gig,
            vault,
            milestone: *milestone,
            system_program: anchor_lang::solana_program::system_program::ID,
        }
        .to_account_metas(None),
    )
}

pub struct FundAccounts {
    pub client: Pubkey,
    pub gig: Pubkey,
    pub milestone: Pubkey,
    pub vault: Pubkey,
    pub vault_token_account: Pubkey,
    pub client_token_account: Pubkey,
    pub mint: Pubkey,
}

pub fn ix_fund_milestone(a: &FundAccounts) -> Instruction {
    let (escrow_authority, _) = escrow_authority_pda();
    Instruction::new_with_bytes(
        escrow::ID,
        &escrow::instruction::FundMilestone {}.data(),
        escrow::accounts::FundMilestone {
            client: a.client,
            gig: a.gig,
            milestone: a.milestone,
            vault: a.vault,
            vault_token_account: a.vault_token_account,
            client_token_account: a.client_token_account,
            mint: a.mint,
            escrow_authority,
            gig_program: gig::ID,
            token_program: spl_token::ID,
            system_program: anchor_lang::solana_program::system_program::ID,
        }
        .to_account_metas(None),
    )
}

pub fn ix_submit_delivery(freelancer: &Pubkey, gig: &Pubkey, milestone: &Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        escrow::ID,
        &escrow::instruction::SubmitDelivery {}.data(),
        escrow::accounts::SubmitDelivery {
            freelancer: *freelancer,
            gig: *gig,
            milestone: *milestone,
        }
        .to_account_metas(None),
    )
}

pub struct ReleaseAccounts {
    pub client: Pubkey,
    pub gig: Pubkey,
    pub milestone: Pubkey,
    pub vault: Pubkey,
    pub vault_token_account: Pubkey,
    pub freelancer: Pubkey,
    pub freelancer_token_account: Pubkey,
    pub mint: Pubkey,
}

pub fn ix_approve_milestone(a: &ReleaseAccounts) -> Instruction {
    let (escrow_authority, _) = escrow_authority_pda();
    Instruction::new_with_bytes(
        escrow::ID,
        &escrow::instruction::ApproveMilestone {}.data(),
        escrow::accounts::ApproveMilestone {
            client: a.client,
            gig: a.gig,
            milestone: a.milestone,
            vault: a.vault,
            vault_token_account: a.vault_token_account,
            freelancer: a.freelancer,
            freelancer_token_account: a.freelancer_token_account,
            mint: a.mint,
            escrow_authority,
            gig_program: gig::ID,
            token_program: spl_token::ID,
        }
        .to_account_metas(None),
    )
}

pub struct TimeoutAccounts {
    pub gig: Pubkey,
    pub milestone: Pubkey,
    pub vault: Pubkey,
    pub vault_token_account: Pubkey,
    pub freelancer_token_account: Pubkey,
    pub mint: Pubkey,
}

pub fn ix_partial_timeout_release(a: &TimeoutAccounts) -> Instruction {
    Instruction::new_with_bytes(
        escrow::ID,
        &escrow::instruction::PartialTimeoutRelease {}.data(),
        escrow::accounts::PartialTimeoutRelease {
            gig: a.gig,
            milestone: a.milestone,
            vault: a.vault,
            vault_token_account: a.vault_token_account,
            freelancer_token_account: a.freelancer_token_account,
            mint: a.mint,
            token_program: spl_token::ID,
        }
        .to_account_metas(None),
    )
}

pub fn ix_full_timeout_release(a: &TimeoutAccounts) -> Instruction {
    let (escrow_authority, _) = escrow_authority_pda();
    Instruction::new_with_bytes(
        escrow::ID,
        &escrow::instruction::FullTimeoutRelease {}.data(),
        escrow::accounts::FullTimeoutRelease {
            gig: a.gig,
            milestone: a.milestone,
            vault: a.vault,
            vault_token_account: a.vault_token_account,
            freelancer_token_account: a.freelancer_token_account,
            mint: a.mint,
            escrow_authority,
            gig_program: gig::ID,
            token_program: spl_token::ID,
        }
        .to_account_metas(None),
    )
}

pub fn ix_cancel_before_funding(client: &Pubkey, gig: &Pubkey, milestone: &Pubkey) -> Instruction {
    let (escrow_authority, _) = escrow_authority_pda();
    let (vault, _) = vault_pda(gig);
    Instruction::new_with_bytes(
        escrow::ID,
        &escrow::instruction::CancelBeforeFunding {}.data(),
        escrow::accounts::CancelBeforeFunding {
            client: *client,
            gig: *gig,
            milestone: *milestone,
            vault,
            escrow_authority,
            gig_program: gig::ID,
        }
        .to_account_metas(None),
    )
}

// ── Test harness helpers ──

pub struct SetupAccounts {
    pub gig: Pubkey,
    pub milestone: Pubkey,
    pub vault: Pubkey,
    pub vault_token_account: Pubkey,
    pub client_token_account: Pubkey,
    pub freelancer_token_account: Pubkey,
}

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
    send(
        &mut env.svm,
        &env.payer,
        &[ix_publish_gig(&env.client.pubkey(), gig)],
        &[&env.payer, &env.client],
    )
    .unwrap();
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
    send(
        &mut env.svm,
        &env.payer,
        &[ix_complete_gig(&env.client.pubkey(), gig)],
        &[&env.payer, &env.client],
    )
    .unwrap();
}

pub fn archive_gig_for(env: &mut Env, gig: &Pubkey) {
    send(
        &mut env.svm,
        &env.payer,
        &[ix_archive_gig(&env.client.pubkey(), gig)],
        &[&env.payer, &env.client],
    )
    .unwrap();
}

pub fn cancel_gig_for(env: &mut Env, gig: &Pubkey) {
    send(
        &mut env.svm,
        &env.payer,
        &[ix_cancel_gig(&env.client.pubkey(), gig)],
        &[&env.payer, &env.client],
    )
    .unwrap();
}

pub fn create_milestone_for(env: &mut Env, gig: &Pubkey, index: u32, amount: u64) -> Pubkey {
    let (milestone, _) = milestone_pda(gig, index);
    send(
        &mut env.svm,
        &env.payer,
        &[ix_create_milestone(&env.client.pubkey(), gig, &milestone, amount)],
        &[&env.payer, &env.client],
    )
    .unwrap();
    milestone
}

/// Full harness: init gig (Draft) -> publish -> assign freelancer -> create milestone 0 -> fund.
pub fn create_funded_milestone(env: &mut Env, gig_id: u64, amount: u64) -> SetupAccounts {
    let gig = init_gig(env, gig_id);
    publish_and_assign(env, &gig);
    let milestone = create_milestone_for(env, &gig, 0, amount);

    let (vault, _) = vault_pda(&gig);
    let (vault_token_account, _) = vault_token_pda(&gig);
    let client_token_account = create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.client.pubkey());
    let freelancer_token_account =
        create_token_account(&mut env.svm, &env.payer, &env.mint.pubkey(), &env.freelancer.pubkey());

    mint_to(
        &mut env.svm,
        &env.payer,
        &env.mint.pubkey(),
        &env.payer,
        &client_token_account,
        amount,
    );

    send(
        &mut env.svm,
        &env.payer,
        &[ix_fund_milestone(&FundAccounts {
            client: env.client.pubkey(),
            gig,
            milestone,
            vault,
            vault_token_account,
            client_token_account,
            mint: env.mint.pubkey(),
        })],
        &[&env.payer, &env.client],
    )
    .unwrap();

    SetupAccounts {
        gig,
        milestone,
        vault,
        vault_token_account,
        client_token_account,
        freelancer_token_account,
    }
}

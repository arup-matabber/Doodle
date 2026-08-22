#![allow(dead_code)]

use anchor_lang::{
    solana_program::instruction::Instruction, AccountDeserialize, AccountSerialize,
    InstructionData, ToAccountMetas,
};
use litesvm::LiteSVM;
use solana_account::Account;
use solana_clock::Clock;
use solana_keypair::Keypair;
use solana_message::{Message, VersionedMessage};
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;

pub use achievement::state::{Achievement, AchievementConfig};
pub use reputation::{Badge, BadgeType, UserProfile};

pub struct Env {
    pub svm: LiteSVM,
    pub payer: Keypair,
    pub user: Keypair,
}

pub fn setup() -> Env {
    let mut svm = LiteSVM::new();
    let achievement_bytes = include_bytes!("../../../../target/deploy/achievement.so");
    svm.add_program(achievement::ID, achievement_bytes).unwrap();
    let reputation_bytes = include_bytes!("../../../../target/deploy/reputation.so");
    svm.add_program(reputation::ID, reputation_bytes).unwrap();

    let payer = Keypair::new();
    let user = Keypair::new();
    for kp in [&payer, &user] {
        svm.airdrop(&kp.pubkey(), 10_000_000_000).unwrap();
    }

    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp = 1_700_000_000;
    svm.set_sysvar::<Clock>(&clock);

    Env { svm, payer, user }
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
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &all_signers)
        .map_err(|e| e.to_string())?;
    svm.send_transaction(tx).map(|_| ()).map_err(|e| e.err.to_string())
}

// ── PDA derivation ──

pub fn reputation_profile_pda(authority: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"profile", authority.as_ref()], &reputation::ID)
}

pub fn reputation_badge_pda(authority: &Pubkey, badge_type: BadgeType) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"badge", authority.as_ref(), &[badge_type.as_seed()]], &reputation::ID)
}

pub fn config_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"config"], &achievement::ID)
}

pub fn achievement_pda(owner: &Pubkey, badge_type: BadgeType) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"achievement", owner.as_ref(), &[badge_type.as_seed()]], &achievement::ID)
}

// ── Reputation setup helpers (drives real reputation instructions) ──

pub fn init_reputation_profile(env: &mut Env, authority: &Keypair) -> Pubkey {
    let (profile, _) = reputation_profile_pda(&authority.pubkey());
    let ix = Instruction::new_with_bytes(
        reputation::ID,
        &reputation::instruction::InitializeProfile {}.data(),
        reputation::accounts::InitializeProfile {
            authority: authority.pubkey(),
            profile,
            system_program: anchor_lang::solana_program::system_program::ID,
        }
        .to_account_metas(None),
    );
    send(&mut env.svm, &env.payer, &[ix], &[&env.payer, authority]).unwrap();
    profile
}

/// Directly overwrites the profile's stats so a badge's eligibility
/// criteria are met, bypassing the escrow-only `update_completion` CPI path
/// (which requires a live escrow signer this test suite doesn't stand up).
pub fn set_completed_jobs(env: &mut Env, profile_key: &Pubkey, completed_jobs: u64) {
    let data = env.svm.get_account(profile_key).unwrap().data;
    let mut profile = UserProfile::try_deserialize(&mut &data[..]).unwrap();
    profile.completed_jobs = completed_jobs;
    profile.successful_jobs = completed_jobs;

    let mut bytes = Vec::new();
    profile.try_serialize(&mut bytes).unwrap();

    let mut account = env.svm.get_account(profile_key).unwrap();
    account.data = bytes;
    env.svm.set_account(*profile_key, account).unwrap();
}

pub fn award_badge(env: &mut Env, authority: &Pubkey, profile: &Pubkey, badge_type: BadgeType) -> Pubkey {
    let (badge, _) = reputation_badge_pda(authority, badge_type);
    let ix = Instruction::new_with_bytes(
        reputation::ID,
        &reputation::instruction::AwardBadge { badge_type, metadata: String::new() }.data(),
        reputation::accounts::AwardBadge {
            payer: env.payer.pubkey(),
            profile: *profile,
            badge,
            system_program: anchor_lang::solana_program::system_program::ID,
        }
        .to_account_metas(None),
    );
    send(&mut env.svm, &env.payer, &[ix], &[&env.payer]).unwrap();
    badge
}

pub fn read_badge(svm: &LiteSVM, key: &Pubkey) -> Badge {
    let data = svm.get_account(key).unwrap().data;
    Badge::try_deserialize(&mut &data[..]).unwrap()
}

pub fn read_achievement(svm: &LiteSVM, key: &Pubkey) -> Achievement {
    let data = svm.get_account(key).unwrap().data;
    Achievement::try_deserialize(&mut &data[..]).unwrap()
}

/// Writes a fully-formed `Achievement` account into the test SVM directly,
/// simulating "this badge was already claimed" without needing a live
/// Metaplex Core program to actually perform a first mint.
pub fn seed_claimed_achievement(env: &mut Env, owner: &Pubkey, badge_type: BadgeType) -> Pubkey {
    let (achievement_key, bump) = achievement_pda(owner, badge_type);
    let achievement = Achievement {
        owner: *owner,
        badge_type,
        mint: Pubkey::new_unique(),
        claimed: true,
        claimed_at: 1_700_000_000,
        bump,
    };
    let mut bytes = Vec::new();
    achievement.try_serialize(&mut bytes).unwrap();

    let rent = env.svm.minimum_balance_for_rent_exemption(bytes.len());
    let account = Account {
        lamports: rent,
        data: bytes,
        owner: achievement::ID,
        executable: false,
        rent_epoch: 0,
    };
    env.svm.set_account(achievement_key, account).unwrap();
    achievement_key
}

pub fn ix_init_collection(
    admin: &Pubkey,
    config: &Pubkey,
    collection: &Pubkey,
    mpl_core_program: &Pubkey,
    name: String,
    uri: String,
    base_uri: String,
) -> Instruction {
    Instruction::new_with_bytes(
        achievement::ID,
        &achievement::instruction::InitCollection { name, uri, base_uri }.data(),
        achievement::accounts::InitCollection {
            admin: *admin,
            config: *config,
            collection: *collection,
            mpl_core_program: *mpl_core_program,
            system_program: anchor_lang::solana_program::system_program::ID,
        }
        .to_account_metas(None),
    )
}

pub struct ClaimAccounts {
    pub claimer: Pubkey,
    pub profile: Pubkey,
    pub badge: Pubkey,
    pub config: Pubkey,
    pub achievement: Pubkey,
    pub asset: Pubkey,
    pub collection: Pubkey,
    pub mpl_core_program: Pubkey,
}

pub fn ix_claim_achievement(a: &ClaimAccounts, badge_type: BadgeType) -> Instruction {
    Instruction::new_with_bytes(
        achievement::ID,
        &achievement::instruction::ClaimAchievement { badge_type }.data(),
        achievement::accounts::ClaimAchievement {
            claimer: a.claimer,
            profile: a.profile,
            badge: a.badge,
            config: a.config,
            achievement: a.achievement,
            asset: a.asset,
            collection: a.collection,
            mpl_core_program: a.mpl_core_program,
            system_program: anchor_lang::solana_program::system_program::ID,
        }
        .to_account_metas(None),
    )
}

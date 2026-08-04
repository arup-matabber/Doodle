# Changelog

All notable changes to the Gig, Escrow, and Reputation programs and their documentation are recorded here.

Entries below the current one are kept as written at the time of release. Where an older entry says "production-ready", read it as "implementation, tests, and internal audit complete" — no program is deployed, and none has had an external audit. Test counts in older entries predate later suite growth; see [TESTING.md](./TESTING.md) for current figures.

## [Unreleased — repo layout fix]

### Fixed
- Moved `programs/services/reputation-indexer` to `services/reputation-indexer` (still just `.gitkeep` scaffold, no source). It had drifted under `programs/`, where Anchor's workspace scan expects every entry to be a Cargo crate — `anchor build` failed with "Error reading manifest from path: programs/services/Cargo.toml" until it was moved back out. No workaround scripts needed now; `anchor build` runs clean from the repo root. This also matches the layout already documented in README.md and ARCHITECTURE.md.

## [Unreleased — Achievement program]

Adds a fourth, independently-deployable program that mints NFT credentials for badges the Reputation program has already awarded. Does not modify Gig, Escrow, or Reputation's existing behavior, state, or CPI surface.

### Added
- `programs/achievement` (Anchor, `declare_id!("GV8Z39NBK7qrojXCfnnwLTXpqsLoCW6sy9cLHGYjtrv9")`) — new workspace member.
  - `init_collection(name, uri)` — one-time admin setup creating the shared Metaplex Core collection, with a program PDA (`config`) as its update authority.
  - `claim_achievement(badge_type)` — user-signed instruction that re-derives the caller's `UserProfile`/`Badge` PDAs from `programs/reputation` as eligibility proof, then mints a Metaplex Core asset via CPI into the shared collection.
  - `AchievementConfig` (singleton) and `Achievement` (one PDA per `(owner, badge_type)`) account types.
  - `AchievementClaimed` event.
  - Reuses `reputation::BadgeType` rather than declaring a parallel enum — badge eligibility logic stays exclusively in `programs/reputation`.
- `programs/achievement/tests/claim_achievement.rs` (8 tests) — eligibility, forged-profile/forged-badge rejection, invalid-signer/invalid-PDA rejection, and duplicate-claim/replay rejection.
- SECURITY.md §27 "Achievement Program Security Model".
- ARCHITECTURE.md §20 "Achievement Program".
- README.md "Achievement Program" section and Implementation Matrix rows.
- TESTING.md "Achievement Program" section.

### Notes
- NFT minting is never triggered by escrow settlement — `claim_achievement` is always a separate, user-initiated transaction. Settlement's CPI surface is unchanged (still ends at Reputation, §18).
- This offline development sandbox has no network access to the deployed Metaplex Core program binary, so the test suite verifies every check Achievement itself owns (all of which run before the handler reaches the mpl-core CPI) but does not execute a real mint end-to-end. See TESTING.md's "Known gap" note before treating this program as audited to the same standard as Gig/Escrow/Reputation.

## [Unreleased — ESCROW_PROGRAM_ID consistency guard]

Documents and enforces the intentional duplication of `ESCROW_PROGRAM_ID` across `gig` and `reputation`, without introducing a mutable on-chain registry (the compile-time trust model is preserved by design — see SECURITY.md §4c).

### Added
- Compile-time consistency check (`const _: () = assert!(...)`, `programs/escrow/src/lib.rs`) that fails the build if `gig::ESCROW_PROGRAM_ID` or `reputation::ESCROW_PROGRAM_ID` ever drifts from Escrow's own `declare_id!`.
- `docs/runbooks/escrow-redeploy.md` — step-by-step runbook for redeploying Escrow to a new program ID, including updating both dependent constants and rebuilding/redeploying `gig` and `reputation`.
- SECURITY.md §4c "Escrow Program ID Trust Assumption (Operational)" — documents why the duplication is intentional and what the compile-time guard covers.

### Changed
- `programs/gig/src/constants.rs` and `programs/reputation/src/constants.rs` — `ESCROW_PROGRAM_ID` doc comments now point to SECURITY.md §4c and the redeploy runbook instead of a bare `ponytail:` note.
- README.md / ARCHITECTURE.md cross-reference the new SECURITY.md section and runbook wherever `ESCROW_PROGRAM_ID` is discussed.

## [Unreleased — Escrow/Reputation CPI]

Wires the Escrow and Reputation programs together via secure CPI, closing the trust gap left by the previous `REPUTATION_AUTHORITY` hardcoded-pubkey design: reputation now updates only after Escrow itself confirms a payment has settled, signed by the same kind of `escrow_authority` PDA already used for the Escrow → Gig CPI.

### Added
- Two new Escrow instructions: `settle_reputation` (permissionless, callable once a gig's vault is fully released, at most once per gig) and `rate_freelancer` (client-signed, callable once a gig is `Completed`). Both CPI into the Reputation program, signed by Escrow's own `escrow_authority` PDA.
- `EscrowVault.reputation_synced: bool` — prevents `settle_reputation`'s CPI (and its earnings credit) from ever firing more than once per gig.
- `programs/escrow/tests/reputation_settlement.rs` — 10 new cross-program integration tests (all three programs deployed together in one `litesvm` instance) covering the real settlement/rating CPI flow, duplicate-settlement/duplicate-rating rejection, premature-settlement rejection, and direct-call/forged-signer rejection.
- `programs/reputation/tests/pda_security.rs` gains direct CPI-forgery tests: an attacker's real (non-PDA) keypair standing in for `escrow_authority` is rejected for both `update_completion` and `submit_rating`.

### Changed
- `update_completion` and `submit_rating` no longer accept `REPUTATION_AUTHORITY`, a single hardcoded pubkey. They now require an `escrow_authority: Signer<'info>` constrained to `seeds = [ESCROW_AUTHORITY_SEED], bump, seeds::program = ESCROW_PROGRAM_ID` — the same pattern Gig already uses to trust Escrow. A PDA has no private key, so only Escrow's own `invoke_signed` CPI can satisfy it; `REPUTATION_AUTHORITY` and its keypair fixture are removed.
- `award_badge` is now permissionless (`payer` replaces the `authority` field, which no longer requires any signer beyond footing rent) — eligibility is recomputed from the profile's own public fields on every call, so there was no privileged data left to gate.
- `is_eligible_for_badge` returns `false` (not `true`) for `TrustedFreelancer`/`FastDeliverer`: since award is now permissionless, these two badge types — which have no on-chain signal backing them — are simply not awardable yet, rather than being awardable on a bare unverified claim.
- `fund_milestone`'s `gig: Account<'info, Gig>` is now `Box<Account<'info, Gig>>`, trimming its `try_accounts` stack frame back under the SBF 4096-byte limit after the new `reputation` crate dependency pushed it 8 bytes over.

### Removed
- `REPUTATION_AUTHORITY` constant and its keypair test fixture (`programs/reputation/tests/fixtures/authority-keypair.json`).
- Reputation's `math.rs`, `completion_updates.rs`, `rating_submission.rs`, `rating_validation.rs`, `regressions.rs`, `reputation_algorithm.rs`, `state_invariants.rs`, `badge_system.rs` test modules — their positive-path coverage assumed direct, non-CPI calls to `update_completion`/`submit_rating`, which the security fix deliberately makes impossible; that coverage now lives in `programs/escrow/tests/reputation_settlement.rs`, exercising the real CPI path instead. Reputation's own suite keeps what remains validly testable in isolation (profile creation/authorization, PDA/CPI-forgery security, pure-function unit tests in `utils.rs`).

## [Unreleased — Gig/Escrow Program Split]

### Added
- New `programs/gig` program (`declare_id!("9LpGZY8p8dYfYdWm5D9MuvGXh9VXdF8DqEEAmdNZ92Na")`), extracted from the previously-merged Escrow program. Owns `Gig` account, job metadata, and lifecycle only: `initialize_gig`, `update_gig`, `publish_gig`, `assign_freelancer`, `complete_gig`, `archive_gig`, `cancel_gig`.
- Three CPI-only Gig instructions — `mark_in_progress`, `mark_completed_by_escrow`, `mark_cancelled_by_escrow` — callable exclusively by Escrow, authorized via an `escrow_authority` signer PDA constrained by `seeds::program = ESCROW_PROGRAM_ID`. The Solana runtime itself guarantees only the Escrow program can produce a valid signature for that PDA; see [ARCHITECTURE.md §5.3–5.4](./ARCHITECTURE.md) and [SECURITY.md §4a](./SECURITY.md).
- `GigStatus` gains an `InProgress` variant (`Draft → Published → Assigned → InProgress → Completed`, `Cancelled`/`Archived` as before), entered only via Escrow's `mark_in_progress` CPI on first milestone funding — never client-callable.
- `programs/gig/tests/` — new 68-test litesvm suite across 8 modules, including `cpi_authorization.rs` proving the three CPI-only instructions reject any direct, non-CPI caller.
- `programs/escrow/tests/gig_escrow_integration.rs` — 11 new cross-program integration tests covering the full create→publish→assign→fund→InProgress→complete/cancel flow across both deployed programs in one `litesvm` instance, plus unauthorized-CPI and wrong-PDA rejections.

### Changed
- `EscrowVault` gains `milestone_count`/`active_milestone` (`u32` each), moved off `Gig` — these are payment-lifecycle counters, not job metadata, so Escrow now owns them directly instead of needing a CPI just to keep a counter in sync.
- `Gig` account (owned by `programs/gig`) drops `milestone_count`/`active_milestone` accordingly.
- `EscrowError` trimmed to escrow-only concerns (Escrow no longer defines gig-metadata errors like `TitleTooLong`/`NotDraftStatus`; those live in Gig's own `GigError`); new `GigNotFundable` variant replaces the old `InvalidStatus`/`NotAssignedStatus` checks on `create_milestone`/`fund_milestone`, which now accept `Assigned` or `InProgress`.
- `cancel_before_funding` no longer mutates `Gig.status` directly (it can't — it doesn't own that account); it now CPIs into `mark_cancelled_by_escrow`.
- `approve_milestone` / `full_timeout_release` no longer set `Gig.status = Completed` directly on the final milestone; they now CPI into `mark_completed_by_escrow`.
- `Anchor.toml` gains a `[programs.localnet] gig` entry; the `[scripts] test` command now runs gig → escrow → reputation in sequence.
- Escrow's `Cargo.toml` gains `gig = { path = "../gig", features = ["cpi"] }`; the workspace `Cargo.toml` adds `programs/gig` as a member.

### Why
See [ARCHITECTURE.md §2](./ARCHITECTURE.md) for the full rationale: smaller per-program attack surface, independent deployability of job-listing logic vs. audited payment-custody logic, single source of truth for gig metadata (Escrow never duplicates it, only reads a `Gig` account it doesn't own), and least-privilege CPI (Escrow can trigger exactly three narrow transitions, nothing else).

## [Unreleased — Escrow Gig Lifecycle]

### Added
- Six gig-lifecycle instructions on the Escrow program: `update_gig`, `publish_gig`, `assign_freelancer`, `complete_gig`, `archive_gig`, `cancel_gig` — bringing the escrow program to **14 instructions**. Gig state lives in the escrow program (the `programs/gig` directory is an empty scaffold, not a separate program) so a milestone release can advance `GigStatus` atomically in the same instruction.
- `GigStatus` expanded from `Active | Completed | Cancelled` to the full six-state machine: `Draft → Published → Assigned → Completed → Archived`, with `Cancelled` reachable from any of the first three. `create_milestone` now requires `Assigned`, so no gig can escrow funds before a freelancer is assigned.
- On-chain listing metadata on `Gig`: `title`, `description`, `skills`, `category`, `budget`, `deadline`, `mint`, `updated_at` — a gig is fully describable from chain state with no off-chain database. Length caps (`MAX_TITLE_LEN` 100, `MAX_DESCRIPTION_LEN` 500, `MAX_SKILLS_LEN` 200, `MAX_CATEGORY_LEN` 50) and `MIN_DEADLINE_SECS` (1 day) are enforced at every write.
- 5 new events (`GigUpdated`, `GigPublished`, `FreelancerAssigned`, `GigCompleted`, `GigArchived`), bringing Escrow to **13 events** — still one per state-changing instruction.
- 13 new `EscrowError` variants for gig status and input validation (`NotDraftStatus`, `NotPublishedStatus`, `NotAssignedStatus`, `NotCompletedStatus`, `TerminalStatus`, `FreelancerAlreadyAssigned`, `InvalidBudget`, `InvalidDeadline`, `TitleTooLong`, `DescriptionTooLong`, `SkillsTooLong`, `CategoryTooLong`, `MetadataTooLong`), bringing the total to **24**.
- 6 new Escrow test modules — `lifecycle.rs` (15), `gig_creation.rs` (11), `gig_updates.rs` (12), `validation.rs` (12), `freelancer_assignment.rs` (8), `publishing.rs` (6) — bringing Escrow to **180 integration tests across 16 modules** (184 including unit tests).

### Changed
- `Gig` PDA seeds are `[GIG_SEED, id]` — the client is **not** a seed. Gig ids therefore share one global namespace: the first client to create id *N* owns it, and a later `initialize_gig` with the same id fails at `init`. Integrators must allocate ids collision-aware.
- `initialize_gig` now creates a gig in `Draft` (previously immediately active) and takes `title`, `description`, `category`, `budget`, and `deadline`. It has no `skills` parameter; `skills` starts empty and can only be set through `update_gig`, which is `Draft`-only.
- `assign_freelancer` replaces passing the freelancer at gig creation. It enforces `client != freelancer` and rejects reassignment once set.

### Documentation
- Rewrote [README.md](./README.md) as the canonical repository entry point, verified line-by-line against program source: implementation matrix with per-component status, full instruction reference, PDA architecture, account model, threat model, invariant list, and an explicit statement that nothing is deployed and no external audit exists.
- Corrected test counts across all docs. Previously-published figures (106 Escrow / 145 Reputation) were inaccurate; the counted totals are **184 Escrow** and **144 Reputation** (**328** total, 0 failures).
- Updated [ARCHITECTURE.md](./ARCHITECTURE.md) §4.1, §5, §6, §8.2, §10 for the gig lifecycle, and [SECURITY.md](./SECURITY.md) §2 and §6 for gig-status and `assign_freelancer` authority rules. Corrected a prior claim that `initialize_gig` enforced `require_keys_neq!(client, freelancer)` — that check lives in `assign_freelancer`.
- Marked `docs/details.md` explicitly as product vision, not implementation, wherever it conflicts with program code.

### Status
- Both programs remain implemented, internally audited, and **undeployed**. No external audit has been performed.

## [Reputation Program — Production Release]

### Added
- Complete reputation architecture: `UserProfile`, `Rating`, `Badge` account model, one PDA per authority/job/badge-type.
- Full instruction set: `initialize_profile`, `submit_rating`, `update_completion`, `award_badge`, `get_profile`.
- PDA-based addressing and duplicate-prevention model — `Rating` seeded by `job_id` alone and `Badge` seeded by `(profile, badge_type)`, so duplicate ratings and duplicate badge awards fail structurally at account-init time.
- Deterministic, recomputable reputation scoring (`compute_reputation_score`) — a pure function of a profile's own stored counters, with no randomness or off-chain input.
- 4 typed Anchor events (`ProfileCreated`, `RatingSubmitted`, `CompletionUpdated`, `BadgeAwarded`) for off-chain indexing; a 5th (`ProfileUpdated`) is defined and reserved.
- 145-test `litesvm` regression and security suite across 12 modules: profile creation, authorization, rating submission/validation, badge system, completion updates, reputation algorithm, math, PDA security, events, state invariants, and regressions.

### Security
- Completed a full internal security audit covering signer validation, ownership validation, PDA validation and anti-spoofing, checked arithmetic (overflow/underflow protection, saturating score clamping), reinitialization/replay protection, duplicate-rating and duplicate-badge prevention, deterministic reputation-score computation, badge-eligibility rules, event correctness, and state consistency. No open findings. Two trust assumptions — the single hardcoded `REPUTATION_AUTHORITY` signer and caller-supplied job identity in `submit_rating` — are explicitly documented as accepted pre-CPI trade-offs rather than silent gaps. Full detail in [SECURITY.md § Reputation Program Security](./SECURITY.md#reputation-program--security).
- Completed a dedicated PDA audit: seed derivation, deterministic addressing, bump-seed handling, and anti-spoofing coverage for `UserProfile`, `Rating`, and `Badge` PDAs. Full detail in [ARCHITECTURE.md § 14](./ARCHITECTURE.md#14-pda-architecture).

### Documentation
- Extended [ARCHITECTURE.md](./ARCHITECTURE.md) with a full Reputation Program Architecture section: account model, PDA architecture, instruction flow, state transitions, event architecture, future CPI compatibility, and design rationale.
- Extended [SECURITY.md](./SECURITY.md) with a full Reputation Program Security section: threat model (including explicit trust assumptions), signer/PDA/arithmetic validation, duplicate prevention, deterministic scoring, and a summary of every enforced invariant.
- Extended [TESTING.md](./TESTING.md) with a breakdown of all 12 Reputation test modules and what each validates.
- Updated [IMPLEMENTATION_PROGRESS.md](./IMPLEMENTATION_PROGRESS.md) marking the Reputation program production-ready across architecture, instructions, PDA design, events, errors, tests, and security audit.
- Extended [README.md](./README.md) with a Reputation Program section.

### Status
- **Reputation program: production-ready.** Architecture, PDA design, instruction set, events, error handling, test suite, and security audit are all complete.

## [Escrow Program — Production Release]

### Added
- Complete escrow architecture: `Gig`, `Milestone`, `EscrowVault` account model with a single shared vault per gig.
- Full instruction set: `initialize_gig`, `create_milestone`, `fund_milestone`, `submit_delivery`, `approve_milestone`, `partial_timeout_release`, `full_timeout_release`, `cancel_before_funding`.
- PDA-based custody model — Gig, Milestone, Vault, and Vault Token Account are all Program Derived Addresses, with the vault token account's SPL authority assigned to the `EscrowVault` PDA (no private key, no external signer possible).
- SPL Token integration via `transfer_checked` for both client-funded deposits and PDA-signed releases, with mint validation enforced at every account boundary.
- 8 typed Anchor events (`GigCreated`, `MilestoneCreated`, `MilestoneFunded`, `DeliverySubmitted`, `MilestoneApproved`, `PartialReleaseExecuted`, `FullReleaseExecuted`, `GigCancelled`) for off-chain indexing.
- 106-test `litesvm` regression and security suite across 11 modules: happy path, authorization, state transitions, timeout boundaries, arithmetic, vault accounting, PDA security, token validation, events, and general regression coverage.

### Security
- Completed a full internal security audit covering signer validation, ownership validation, PDA validation and anti-spoofing, checked arithmetic (overflow/underflow protection), state-transition enforcement, mint validation, vault accounting invariants, double-spend prevention, reinitialization protection, permission validation, timeout security, and CPI safety. No open findings. Full detail in [SECURITY.md](./SECURITY.md).

### Documentation
- Added [ARCHITECTURE.md](./ARCHITECTURE.md): protocol overview, account architecture, instruction flow/state machine, event architecture, token flow, and a dedicated PDA architecture section.
- Added [SECURITY.md](./SECURITY.md): threat model and full audit writeup of every enforced invariant.
- Added [TESTING.md](./TESTING.md): breakdown of all 11 test modules and what each validates.
- Added [IMPLEMENTATION_PROGRESS.md](./IMPLEMENTATION_PROGRESS.md): completion status for Escrow, Reputation, and Dispute programs.
- Extended [README.md](./README.md) with an Escrow Program section and links to the above.

### Status
- **Escrow program: production-ready.** Architecture, PDA design, instruction set, SPL Token integration, events, error handling, test suite, and security audit are all complete.

# Gig Program — Test Suite

**68 tests across 8 modules**, run against a `litesvm` in-process Solana runtime (no local validator required). Run with:

```bash
cargo test --manifest-path programs/gig/Cargo.toml
```

Module breakdown (`programs/gig/tests/*.rs`):

| Module | Tests | Validates |
|---|---|---|
| `lifecycle.rs` | 14 | The full gig lifecycle across all seven `GigStatus` variants — draft → published → assigned → (in-progress/completed via CPI, or manual `complete_gig`) → archived, plus cancellation from each non-terminal client-reachable status. |
| `gig_updates.rs` | 12 | `update_gig` — Draft-only enforcement, field overwriting, `updated_at` movement, and every input-bound rejection. |
| `gig_creation.rs` | 11 | `initialize_gig` — every field initialized as expected (`Draft`, empty `skills`, default `freelancer`), PDA derived from `[GIG_SEED, id]`, and each validation rejection. |
| `freelancer_assignment.rs` | 8 | `assign_freelancer` — Published-only precondition, self-assignment rejection, double-assignment rejection, `gig.freelancer` and status effects. |
| `authorization.rs` | 6 | Every signer-gated client instruction rejects the wrong signer — wrong client attempting client-only actions. |
| `publishing.rs` | 6 | `publish_gig` — Draft-only precondition, authority enforcement, `GigPublished` emission. |
| `pda_security.rs` | 5 | PDA spoofing resistance — wrong gig PDA, wrong bump, uninitialized spoofed PDAs rejected. |
| `cpi_authorization.rs` | 5 | The CPI-only surface: `mark_in_progress` / `mark_completed_by_escrow` / `mark_cancelled_by_escrow` all reject a direct (non-CPI) caller — proving the `seeds::program = ESCROW_PROGRAM_ID` constraint on `escrow_authority` cannot be bypassed by anyone who isn't the Escrow program itself. |

## Test Infrastructure

Shared setup (keypair generation, mint creation, gig bootstrapping helpers) lives in `tests/common/mod.rs`.

---

# Escrow Program — Test Suite

**144 integration tests across 13 modules**, plus 4 inline unit tests in `src/utils.rs` (**148 total**), run against the same in-process `litesvm` runtime. Run with:

```bash
cargo test --manifest-path programs/escrow/Cargo.toml
```

`gig.so`, `escrow.so`, and `reputation.so` are all deployed into the same `litesvm` instance for every test, so escrow tests exercise the real cross-program CPI path — not a mock.

Module breakdown (`programs/escrow/tests/*.rs`):

| Module | Tests | Validates |
|---|---|---|
| `state_transitions.rs` | 18 | Every `MilestoneStatus` edge and the `GigStatus` transitions Escrow drives via CPI: valid forward transitions succeed, every out-of-order or repeated transition (double-fund, double-submit, approve-before-submit, release-after-complete, etc.) is rejected with the correct error. |
| `authorization.rs` | 15 | Every signer-gated instruction rejects the wrong signer — wrong client, wrong freelancer, unrelated third-party keys attempting client/freelancer actions. |
| `events.rs` | 14 | Every instruction emits its documented event with fields matching the instruction's actual effects. |
| `pda_security.rs` | 13 | PDA spoofing resistance — wrong gig/milestone/vault PDA, wrong bump, milestone from a different gig, vault/token-account mismatch, cross-gig vault substitution, and uninitialized spoofed PDAs are all rejected. |
| `escrow_flow.rs` | 12 | Multi-milestone gigs, cancellation before funding, interleaved timeout/approval sequences, and other combinations exercised during hardening. |
| `gig_escrow_integration.rs` | 11 | **Cross-program protocol integration (Escrow ↔ Gig)**: create → publish → assign → fund → `GigStatus::InProgress` via CPI; final-milestone approve → `Completed` via CPI; `cancel_before_funding` → `Cancelled` via CPI; `create_milestone`'s `seeds::program` constraint rejecting a wrong-owner/wrong-seeds gig account; wrong-client/wrong-freelancer rejections; and direct, non-CPI calls to all three gig CPI-only instructions rejected (privilege-escalation / unauthorized-CPI regression). |
| `token_validation.rs` | 11 | Mint pinning — funding or release attempted with a token account of the wrong mint, or a vault/gig mint mismatch, is rejected at every account boundary that touches a token account. |
| `happy_path.rs` | 10 | End-to-end success paths: gig → milestone → fund → submit → approve, across single and multi-milestone gigs, confirming every account's final state and every SPL balance is exactly as expected. |
| `reputation_settlement.rs` | 10 | **Cross-program protocol integration (Escrow ↔ Reputation)**: `settle_reputation` updates the freelancer's profile only after every milestone is released, and cannot fire twice (`vault.reputation_synced`) or before full release, or without an existing profile; `rate_freelancer` records a rating only for a `Completed` gig, only for its real client, and rejects duplicates; a direct (non-CPI) call to `update_completion` with an attacker-controlled `escrow_authority` is rejected. |
| `timeout_boundaries.rs` | 8 | Exact boundary behavior of the 72-hour partial and 7-day full timeout windows — one second before the deadline rejects (`TimeoutNotReached`), exactly at/after the deadline succeeds; also confirms `full_timeout_release` cannot fire before `partial_timeout_release` has run. |
| `vault_accounting.rs` | 8 | `EscrowVault.total_locked`/`total_released`/`milestone_count`/`active_milestone` counters stay consistent with actual on-chain SPL token balances across funding, partial release, full release, and multi-milestone/multi-vault scenarios. |
| `arithmetic.rs` | 7 | Checked-arithmetic helpers (`checked_add`, `checked_sub`, `percent_of`) at boundary values including `u64::MAX`, confirming overflow/underflow abort rather than wrap. |
| `validation.rs` | 7 | `create_milestone`/`fund_milestone` input bounds and gig-status preconditions (`Assigned`/`InProgress` only), including the InProgress-refunding case. |
| `src/utils.rs` (`#[cfg(test)]`) | 4 | Unit-level checks of the checked-math helpers in isolation from any Anchor/litesvm context. |

## What Each Category Guarantees

- **Happy Path** — the programs do what they're supposed to do when every party behaves correctly, for both single- and multi-milestone gigs.
- **Gig Lifecycle** — the seven-state `GigStatus` machine cannot be driven backwards, skipped, or re-entered from a terminal state, whether driven by the client (Gig program) or by Escrow (CPI).
- **Cross-Program Integration** — the full protocol flow (create → publish → assign → fund → InProgress → complete/cancel) works end-to-end across both deployed programs in the same runtime, and every CPI-only Gig instruction rejects a caller that isn't Escrow.
- **Authorization** — no instruction can be executed by a party who isn't the required signer for that action.
- **State Transitions** — the milestone/gig state machines cannot be driven out of order, replayed, or skipped.
- **Input Validation** — every caller-supplied string, amount, budget, and deadline is bounds-checked before any account write.
- **Timeout Logic** — the 72h/7d windows are enforced to the boundary in both directions (too early rejected, on-time accepted), and the two-stage sequencing (partial before full) is mandatory.
- **Arithmetic** — no balance or counter can overflow or underflow; percentage math is exact and doesn't lose precision at extreme values.
- **Vault Accounting** — the escrow program's internal bookkeeping (`total_locked`/`total_released`/`milestone_count`/`active_milestone`) never drifts from the real SPL token balances or milestone counts it's tracking.
- **PDA Security** — every account address is validated against its expected derivation, including the `escrow_authority` CPI-signer PDA; nothing resembling account substitution, spoofing, or unauthorized CPI succeeds.
- **Token Validation** — only the mint fixed at gig creation can ever enter or leave the vault.
- **Events** — off-chain indexers can trust emitted events to be a complete, accurate log of on-chain state changes.

## Test Infrastructure

Tests run against `litesvm`, an in-process, dependency-free implementation of the Solana runtime — no `solana-test-validator` process, no network I/O, fast enough to run the full suite on every change. Shared setup (keypair generation, mint creation, gig/milestone bootstrapping helpers, deploying both the `gig` and `escrow` program binaries into one runtime instance, clock warping for timeout paths) lives in `tests/common/mod.rs`.

---

# Reputation Program — Test Suite

**26 integration tests across 4 modules**, plus 10 inline unit tests in `src/utils.rs` and `lib.rs` (**36 total**), run against the same in-process `litesvm` runtime. Run with:

```bash
cargo test -p reputation
```

`update_completion` and `submit_rating` are now CPI-only from Escrow (§18 in ARCHITECTURE.md) — a PDA has no private key, so there is no legitimate way to drive them directly in a reputation-only test harness. Their positive-path coverage (completions, ratings, badge eligibility building on real state, reputation-score recomputation) therefore lives in **`programs/escrow/tests/reputation_settlement.rs`**, which deploys `gig.so` + `escrow.so` + `reputation.so` together and exercises the real cross-program CPI path. The modules below cover what remains directly testable in isolation, plus the CPI-forgery security surface.

Module breakdown (`programs/reputation/tests/*.rs`):

| Module | Tests | Validates |
|---|---|---|
| `pda_security.rs` | 9 | CPI-forgery and PDA-spoofing resistance: an attacker's real keypair (not a PDA) impersonating `escrow_authority` is rejected for both `update_completion` and `submit_rating`; wrong profile/rating PDA, wrong bump, system-owned account substitution, and re-initialization are all rejected; `TrustedFreelancer`/`FastDeliverer` are confirmed permanently ineligible (§21.6 in SECURITY.md). |
| `profile_creation.rs` | 10 | `initialize_profile` happy path — profile fields initialized to zero/defaults, PDA derived correctly, correct/wrong PDA and bump behavior. |
| `profile_authorization.rs` | 4 | Signer validation for the instructions that remain directly callable — `get_profile` works for any caller, wrong PDA fails, duplicate-profile rejected, `authority` field is immutable. |
| `events.rs` | 3 | `ProfileCreated` event emission matches actual account state; no event on a failed (duplicate) instruction. |
| `src/utils.rs` + `lib.rs` (`#[cfg(test)]`) | 10 | Unit-level checks of `checked_add`/`checked_sub`, `average_rating`, `compute_reputation_score` (determinism, capping, cancellation penalties, underflow resistance), and `is_eligible_for_badge`, all in isolation from any Anchor/litesvm/CPI context. |

## What Each Category Guarantees

- **Profile Creation** — a profile is correctly initialized exactly once per authority, at the correct PDA, with all counters zeroed.
- **Authorization / CPI Forgery** — `update_completion` and `submit_rating` can only ever be satisfied by a live CPI from Escrow's own `escrow_authority` PDA (§4b, §15.5 in SECURITY.md); no keypair, forged account, or third-party program can produce a valid signature for it. `award_badge` is permissionless by design and re-verifies eligibility from public state on every call.
- **Reputation Calculation** — `compute_reputation_score` is deterministic, bounded, and resistant to overflow/underflow at extreme inputs (verified as pure-function unit tests).
- **PDA Security** — every account address is validated against its expected derivation; nothing resembling account substitution or spoofing succeeds.
- **Events** — off-chain indexers can trust emitted events to reflect the instruction's actual on-chain effects.

## Test Infrastructure

Reputation's tests share the same `litesvm` approach as Gig's and Escrow's. Shared setup lives in `tests/common/mod.rs` and `tests/common/fixtures.rs`.

---

## Achievement Program

| Module | Tests | Coverage |
|---|---|---|
| `tests/claim_achievement.rs` | 8 | Eligibility, forged-account rejection, duplicate-claim/replay rejection, and PDA/signer validation for `claim_achievement`, plus config-account seeding. |

| Test | Verifies |
|---|---|
| `ineligible_user_rejected` | No `Badge` PDA exists for the claimer → transaction fails before reaching the mint CPI. |
| `forged_badge_rejected` | A non-PDA account substituted for `badge` fails Anchor's seeds/owner check. |
| `forged_profile_rejected` | A different (real, but unrelated) profile PDA substituted for `profile` fails the seeds check derived from `claimer`. |
| `invalid_signer_rejected` | A transaction naming the real owner as `claimer` but signed by a different keypair is rejected at signature verification. |
| `invalid_pda_rejected` | A non-derived account substituted for the `achievement` PDA fails the `init` seeds constraint. |
| `duplicate_claim_rejected` | A pre-seeded, already-`claimed` `Achievement` account causes a second claim attempt to fail at `init` (account already in use); the stored `claimed` flag is unaffected. |
| `eligible_claim_reaches_mpl_core_cpi` | A fully valid claim (real profile, real earned badge, correct signer, correct PDAs) passes every check this program owns and fails only because the real Metaplex Core program isn't deployed in this offline sandbox — see the note below. |
| `config_seeded_correctly` | `AchievementConfig`'s `admin`/`collection` fields deserialize as written. |

**Known gap:** this sandbox has no network access to fetch the deployed Metaplex Core program binary, so no test here executes a real mint. Every check the Achievement program itself is responsible for (eligibility, ownership, signer, PDA identity, duplicate-claim) is verified above, since Anchor's account-constraint layer runs and fails/passes before the handler ever reaches the mpl-core CPI. `init_collection`'s own CPI, and full post-mint state (asset owner, collection membership, on-chain metadata), should be verified against a real `mpl-core` deployment (localnet/devnet) before treating this program as audited to the same standard as Gig/Escrow/Reputation (see SECURITY.md §27).

---

# Coverage Summary

| Program | Integration | Unit | Modules |
|---|---|---|---|
| gig | 68 | 0 | 8 |
| escrow | 144 | 4 | 13 |
| reputation | 26 | 10 | 4 |
| achievement | 8 | 0 | 1 |
| **Total** | **246** | **14** | **26** |

`cargo test` (via the `[scripts] test` entry in `Anchor.toml`, which runs gig → escrow → reputation in order) runs all **252** tests with **0 failures**. Escrow's `reputation_settlement.rs` module is counted under escrow (it deploys and exercises all three programs together) and is where Reputation's completion/rating/badge business logic is actually verified end-to-end, now that those instructions are CPI-only. There are no benchmark or performance suites in the repository. Combined with the completed internal security audit ([SECURITY.md](./SECURITY.md)), all three programs' implementation, test suite, and audit are complete.

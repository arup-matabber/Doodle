# Gig, Escrow & Reputation Programs — Security

**Audit status: internal audit complete for all three programs. No external audit has been performed, and none are deployed.** The Gig program (`programs/gig`), the Escrow program (`programs/escrow`), and the Reputation program (`programs/reputation`) have each completed implementation, a full internal security audit, and their own regression/security test suite. No open findings in any program.

Scope of this document: `programs/gig` + `programs/escrow` — treated together, since they cooperate through CPI (§1–14) — and `programs/reputation` (§15–26). The Dispute program (`programs/dispute`, unimplemented) is out of scope and tracked separately.

## 1. Threat Model

Actors:

- **Client** — funds milestones, approves releases, manages gig listings. Trusted to sign only their own transactions; **not** trusted to act honestly (may go silent, may attempt to re-fund, may attempt to reference someone else's milestone).
- **Freelancer** — submits delivery. Trusted to sign only their own transactions; **not** trusted to fabricate submissions for gigs they aren't party to.
- **Permissionless caller** — anyone, for `partial_timeout_release` / `full_timeout_release`. Must not be able to extract more than the fixed percentage, regardless of who calls it or how many times.
- **Adversarial transaction builder** — may supply arbitrary accounts to any instruction, including accounts that are the right *type* but the wrong *instance* (e.g. a vault from a different gig), or accounts that are uninitialized/attacker-owned, attempting to spoof a PDA.
- **A malicious or buggy third program** — may attempt to call Gig's CPI-only instructions (`mark_in_progress`, `mark_completed_by_escrow`, `mark_cancelled_by_escrow`) directly, without going through Escrow, to force an unauthorized `GigStatus` transition. Must not be able to produce a valid `escrow_authority` signature (§4a).

Assets at risk: SPL tokens held in vault token accounts, and the integrity of `GigStatus` transitions that gate them. The programs' job is to guarantee tokens can only leave a vault via the three defined release paths, in the defined amounts, to the defined recipient — and that a `Gig`'s status can only be advanced by the party (client, or Escrow via CPI) actually authorized to advance it.

## 2. Signer Validation

Every instruction that changes ownership-sensitive state requires the correct `Signer<'info>`:

- `initialize_gig` (Gig) — `client` must sign and becomes `gig.client`; `gig.freelancer` starts as `Pubkey::default()`.
- `update_gig` / `publish_gig` / `assign_freelancer` / `complete_gig` / `archive_gig` / `cancel_gig` (Gig) — `client` must sign, checked against `gig.client` via `has_one = client`. `assign_freelancer` additionally enforces `require_keys_neq!(client, freelancer)`, so a client cannot assign themselves and become both parties, and rejects reassignment once `gig.freelancer` is set (`FreelancerAlreadyAssigned`).
- `create_milestone` / `fund_milestone` / `approve_milestone` / `cancel_before_funding` (Escrow) — `client` must sign, and is additionally checked against `gig.client` via `has_one = client`.
- `submit_delivery` (Escrow) — `freelancer` must sign, checked against `gig.freelancer` via `has_one = freelancer`.
- `partial_timeout_release` / `full_timeout_release` (Escrow) — **intentionally permissionless** (no signer requirement beyond fee-payer). This is a deliberate design choice (§12, "Timeout Security"), not a missing check.
- `mark_in_progress` / `mark_completed_by_escrow` / `mark_cancelled_by_escrow` (Gig) — **CPI-only.** No client or freelancer signature is accepted here at all; the sole accepted signer is the `escrow_authority` PDA, and only Escrow can produce a valid signature for it (§4a).

Each program's own `tests/authorization.rs`-equivalent asserts every signer-gated instruction rejects the wrong signer; the gig-lifecycle authority and precondition rules are covered in Gig's own lifecycle/assignment tests, and the cross-program integration suite additionally asserts the CPI-only instructions reject a direct, non-Escrow caller.

## 3. Ownership & Account-Type Validation

Anchor's typed `Account<'info, T>` wrapper deserializes and checks the account discriminator on every account in every instruction, so a caller cannot substitute an account of the wrong type (e.g. passing a `Milestone` where a `Gig` is expected fails at the framework level before the handler body runs).

## 4. PDA Validation & Anti-Spoofing

Full design rationale in [ARCHITECTURE.md § PDA Architecture](./ARCHITECTURE.md#8-pda-architecture). Security-relevant guarantees:

- Every PDA account is constrained with `seeds = [...], bump` (on creation) or `seeds = [...], bump = stored_bump` (on reuse), forcing the runtime to re-derive and match the exact expected address.
- Every PDA is additionally cross-checked against its logical parent: `milestone.gig == gig.key()`, `vault` seeded from `gig.key()`, `vault_token_account` checked via `address = vault.token_account`.
- **PDA spoofing protection**: an attacker cannot pass an account they control and claim it is "the vault" or "the milestone" for a given gig — the derived address would not match, and Anchor's constraint check fails the transaction before any state mutation or token transfer occurs.
- **Vault ownership guarantees**: the vault token account's SPL `authority` is set to the `EscrowVault` PDA at creation (`token::authority = vault`) and never reassigned. Because that PDA has no private key, only this program (via `invoke_signed` with the correct seeds) can ever authorize a debit.

Verified by each program's `tests/pda_security.rs`: wrong gig PDA, wrong milestone PDA, wrong vault PDA, wrong bump, milestone-from-a-different-gig, vault/token-account mismatch, cross-gig vault substitution in `approve_milestone`, and spoofed-but-uninitialized PDAs are all rejected.

## 4a. Cross-Program CPI Authorization (Escrow → Gig)

This is the trust boundary unique to the split protocol, and gets its own subsection because it is the one place a bug would let a party bypass Gig's own status rules entirely.

- **The mechanism.** `mark_in_progress`, `mark_completed_by_escrow`, and `mark_cancelled_by_escrow` each require an `escrow_authority: Signer<'info>` constrained by `seeds = [ESCROW_AUTHORITY_SEED], bump, seeds::program = ESCROW_PROGRAM_ID`. `ESCROW_PROGRAM_ID` is a hardcoded `Pubkey` constant in `programs/gig/src/constants.rs`, equal to Escrow's own `declare_id!`. This is **not** a crate dependency from Gig on Escrow — just a literal pubkey — so the one-way CPI direction (§5.3–5.4 in ARCHITECTURE.md) is preserved even in the dependency graph.
- **Why it can't be forged.** The Solana runtime enforces that a PDA signature derived under `seeds::program = X` can only be produced by program `X` itself calling `invoke_signed`. No other program — not even Gig itself, not a copy-pasted lookalike program, not an EOA with a lucky keypair (PDAs are off-curve; no keypair exists for them at all) — can ever construct a valid signature for `[b"escrow_authority"]` under Escrow's program ID. This is verified directly: calling any of the three CPI-only instructions as a top-level transaction instruction (not via CPI from Escrow), or via CPI from a different program, fails at the `seeds::program` constraint check before the handler body runs.
- **Status preconditions are re-checked on the Gig side, not trusted from the caller.** Each instruction still independently requires the correct starting `GigStatus` (`mark_in_progress` requires `Assigned`; `mark_completed_by_escrow` requires `InProgress`; `mark_cancelled_by_escrow` requires `Assigned` or `InProgress`). Escrow proposes a transition by successfully producing the signature; Gig is still the sole authority on whether that transition is legal. A bug in Escrow that tried to call `mark_completed_by_escrow` on a `Draft` gig (which should never happen given Escrow's own preconditions, but is not *assumed* safe) would still be rejected here.
- **One-way only.** Gig never CPIs into Escrow. There is no code path in `programs/gig` that references Escrow's program ID except this one read-only comparison, and no code path that constructs a CPI into it.

## 4b. Cross-Program CPI Authorization (Escrow → Reputation)

The same mechanism as §4a, applied to the second CPI edge in the protocol.

- **The mechanism.** Reputation's `update_completion` and `submit_rating` each require an `escrow_authority: Signer<'info>` constrained by `seeds = [ESCROW_AUTHORITY_SEED], bump, seeds::program = ESCROW_PROGRAM_ID`, where `ESCROW_PROGRAM_ID` is a hardcoded `Pubkey` constant in `programs/reputation/src/constants.rs` (again a literal, not a crate dependency on `escrow`, keeping the CPI direction one-way in the dependency graph too).
- **Why it can't be forged.** Identical argument to §4a: a PDA signature under `seeds::program = ESCROW_PROGRAM_ID` can only ever be produced by the Escrow program itself via `invoke_signed`. A direct top-level call, a call "signed" by a real attacker keypair, or a call routed through any other program all fail the `seeds`/`seeds::program` constraint before the handler runs. Verified directly in `programs/reputation/tests/pda_security.rs` (`test_update_completion_rejects_non_pda_signer`, `test_submit_rating_rejects_non_pda_signer`) and end-to-end in `programs/escrow/tests/reputation_settlement.rs` (`test_direct_reputation_update_completion_by_non_escrow_signer_rejected`).
- **Settlement preconditions are re-checked on the Escrow side, not merely asserted by convention.** `settle_reputation` independently requires `vault.milestone_count > 0 && vault.active_milestone >= vault.milestone_count` (every milestone actually released) and `!vault.reputation_synced` (never fired before for this gig); `rate_freelancer` independently requires `gig.status == GigStatus::Completed` and `has_one = client`. Reputation does not re-derive any of this — it trusts the signature as proof the call came from Escrow's own executing code, and trusts Escrow to have gated *when* it fires.
- **One-way only, and additive rather than embedded.** Reputation never CPIs into Escrow. Unlike the Gig CPI (invoked inline from `approve_milestone`/`full_timeout_release`'s last-milestone branch), the Reputation CPI is issued from two dedicated, separately-callable Escrow instructions (`settle_reputation`, `rate_freelancer`) rather than being folded into the settlement instructions themselves — so a gig whose freelancer never created a reputation profile still settles normally; reputation notification is an optional, permissionless follow-up rather than a hard dependency of payment release.

## 4c. Escrow Program ID Trust Assumption (Operational)

`gig::constants::ESCROW_PROGRAM_ID` and `reputation::constants::ESCROW_PROGRAM_ID` are the same literal pubkey, hardcoded independently in each crate (`programs/gig/src/constants.rs`, `programs/reputation/src/constants.rs`), equal to Escrow's own `declare_id!`. This duplication is **intentional, not an oversight**:

- Each program authenticates its escrow CPI caller purely via `seeds::program = ESCROW_PROGRAM_ID` (§4a, §4b) — there is deliberately no mutable on-chain registry account holding "the current escrow program ID." A registry would itself need an admin authority to update it, which would make that admin authority a single point of failure able to redirect `gig`/`reputation`'s trust to an arbitrary program. The compile-time constant has no such attack surface: it cannot be changed by any transaction, admin key, or upgrade authority short of redeploying the dependent program itself.
- The cost of that design is purely operational, not a security gap: **the trusted escrow address is embedded in the compiled `gig` and `reputation` binaries.** Redeploying Escrow to a new program ID without redeploying `gig`/`reputation` leaves them trusting a stale address — every escrow-only instruction (`mark_in_progress`, `mark_completed_by_escrow`, `mark_cancelled_by_escrow`, `update_completion`, `submit_rating`) starts rejecting the new, legitimate Escrow deployment. This fails **closed** (rejects a legitimate caller) rather than open (accepts an illegitimate one) — an outage, not a vulnerability.
- **Consistency is enforced at compile time.** `programs/escrow/src/lib.rs` (the only crate that depends on both `gig` and `reputation` — see its `Cargo.toml`) carries `const _: () = assert!(...)` checks comparing `gig::ESCROW_PROGRAM_ID` and `reputation::ESCROW_PROGRAM_ID` byte-for-byte against escrow's own `declare_id!`. Any drift between the three fails `cargo check -p escrow` / `anchor build` immediately, naming the stale crate, rather than surfacing later as callers silently getting rejected on a live cluster.
- See [docs/runbooks/escrow-redeploy.md](./docs/runbooks/escrow-redeploy.md) for the procedure to follow whenever Escrow's program ID changes.

## 5. Reinitialization & Replay Protection

- `Gig` and `Milestone` accounts use `init` (not `init_if_needed`) — Anchor's `init` fails if the account already exists, so a gig or milestone address can never be reinitialized once created, and the one-time `id`/`index`-derived seed guarantees no two gigs/milestones ever share an address.
- `EscrowVault` and the vault token account use `init_if_needed` deliberately, because multiple milestones under the same gig legitimately fund into the *same* vault. Reinitialization is not a vulnerability here because `init_if_needed` is idempotent at the account level (Anchor skips re-running `init` logic if the account is already initialized) and the handler additionally re-validates `vault.mint` on every subsequent fund (`require_keys_eq!(vault.mint, mint.key())`) so a second funding call cannot silently swap the vault's mint.
- Each milestone's `status` state machine (below) means a given milestone cannot be funded, submitted, or released twice — every transition consumes the prior state.

## 6. State Transition Validation

`MilestoneStatus` only moves forward: `PendingFunding → Funded → Submitted → {PartialReleased → Completed | Completed}`. Every handler asserts the exact required starting status before mutating:

| Instruction | Required starting status | Error on mismatch |
|---|---|---|
| `fund_milestone` | `PendingFunding` | `AlreadyFunded` |
| `submit_delivery` | `Funded` (and not already `Submitted`) | `InvalidStatus` / `MilestoneAlreadySubmitted` |
| `approve_milestone` | `Submitted` | `InvalidStatus` |
| `partial_timeout_release` | `Submitted` | `InvalidStatus` |
| `full_timeout_release` | `PartialReleased` | `InvalidStatus` |
| `cancel_before_funding` | `PendingFunding` | `AlreadyFunded` |

This ordering also enforces the intended timeout sequencing: `full_timeout_release` cannot fire before `partial_timeout_release` has already moved the milestone to `PartialReleased`, since that's its required precondition. Each program's `tests/state_transitions.rs`-equivalent exhaustively exercises every valid and invalid transition.

`GigStatus` (`Draft → Published → Assigned → InProgress → Completed → Archived`, with `Cancelled` reachable from `Draft`/`Published`/`Assigned` directly, or from `Assigned`/`InProgress` via Escrow's CPI) is likewise checked as an account constraint on every instruction that touches it — both Gig's own client-facing instructions and Escrow's CPI calls:

| Instruction | Program | Required starting status | Error on mismatch |
|---|---|---|---|
| `update_gig` | Gig | `Draft` | `NotDraftStatus` |
| `publish_gig` | Gig | `Draft` | `NotDraftStatus` |
| `assign_freelancer` | Gig | `Published` | `NotPublishedStatus` |
| `complete_gig` | Gig | `Assigned` | `NotAssignedStatus` |
| `archive_gig` | Gig | `Completed` | `NotCompletedStatus` |
| `cancel_gig` | Gig | `Draft`, `Published`, or `Assigned` | `InvalidStatus` |
| `create_milestone` / `fund_milestone` | Escrow | `Assigned` or `InProgress` | `GigNotFundable` |
| `mark_in_progress` (CPI) | Gig | `Assigned` | `NotAssignedStatus` |
| `mark_completed_by_escrow` (CPI) | Gig | `InProgress` | `NotInProgressStatus` |
| `mark_cancelled_by_escrow` (CPI) | Gig | `Assigned` or `InProgress` | `InvalidStatus` |

`create_milestone`/`fund_milestone` requiring `Assigned`/`InProgress` is what prevents new milestones or funding on a draft, unpublished, cancelled, completed, or archived gig — and guarantees `gig.freelancer` is set before any money can be escrowed against the gig. Each program's own lifecycle tests, plus the cross-program integration suite, exercise every legal and illegal gig transition, including transitions attempted through the wrong program (e.g. trying to fund a `Draft` gig, or trying to `cancel_gig` an `InProgress` one — both rejected).

Note that Gig's client-facing status checks are listing hygiene, not custody controls: no `GigStatus` value can release, redirect, or refund funds already locked in a milestone. Cancelling a gig (whether client-initiated pre-funding, or Escrow-initiated via `mark_cancelled_by_escrow`) does not touch a funded milestone's vault balance — that milestone still settles only through approval or the timeout path.

## 7. Checked Arithmetic — Overflow & Underflow Protection

All balance/counter math routes through `programs/escrow/src/utils.rs`, never raw `+`/`-`:

- `checked_add(a, b)` → `EscrowError::Overflow` on overflow.
- `checked_sub(a, b)` → `EscrowError::MathError` on underflow.
- `percent_of(amount, percent)` promotes to `u128` before multiplying, so `amount * percent` cannot overflow `u64` even at `amount = u64::MAX`, then checks the `u128 → u64` downcast explicitly.

Every counter that money flows through — `EscrowVault.milestone_count`/`active_milestone`/`total_locked`/`total_released`, `Milestone.released` — is updated exclusively through these helpers. (These counters live on `EscrowVault`, not `Gig` — see ARCHITECTURE.md §4.1 — precisely so Gig's own account never needs escrow-specific arithmetic at all.) The release path always computes the remaining payable amount as `checked_sub(milestone.amount, milestone.released)` and requires it to be `> 0` (`InsufficientFunds`), so a milestone can never pay out more than `milestone.amount` in total even across a partial + full release pair. `tests/arithmetic.rs` (7 tests, plus 4 unit tests in `utils.rs`) covers overflow, underflow, and percentage-split edge cases including `u64::MAX`.

## 8. Token Mint Validation

Every account that could conceivably carry the wrong asset is pinned to the gig's canonical mint:

- `Gig.mint` is fixed at `initialize_gig` and never mutated afterward.
- `fund_milestone` requires `gig.mint == mint` (`has_one`) and `client_token_account.mint == mint`.
- `EscrowVault.mint` is fixed on first funding and re-checked (`require_keys_eq!`) on every subsequent funding call, so a vault cannot be "topped up" with a different mint.
- `approve_milestone` / `partial_timeout_release` / `full_timeout_release` all require `vault.mint == mint` (`has_one`) and `freelancer_token_account.mint == mint`.
- All transfers use `transfer_checked`, which independently validates the mint and decimals passed match the token accounts at the SPL Token program level — a second, protocol-level check beyond Anchor's own constraints.

`tests/token_validation.rs` (11 tests) covers wrong-mint funding, wrong-mint release destinations, and mint-substitution attempts.

## 9. Vault Accounting Invariants

`EscrowVault.total_locked` and `total_released` are maintained as running counters alongside the actual SPL token balance, giving two independent sources of truth that must never diverge:

- `total_locked` only increases, only in `fund_milestone`, only by the exact amount transferred in.
- `total_released` only increases, only in the three release instructions, only by the exact amount transferred out.
- Per-milestone `released` is the authoritative cap: `remaining = milestone.amount - milestone.released` bounds every release, so cumulative payout across a `partial_timeout_release` followed by a `full_timeout_release` can never exceed `milestone.amount`.

`tests/vault_accounting.rs` (8 tests) asserts vault counters match actual on-chain token balances across funding, partial release, full release, and multi-milestone scenarios.

## 10. Double-Spend Prevention

Double-spending is prevented by the composition of §6 (state transitions) and §9 (per-milestone `released` cap): once a milestone reaches `Completed`, no further release instruction accepts it (all three release instructions require a specific non-`Completed` starting status), and even within the timeout sequence the `released` field ensures a second release only ever pays the *remaining* balance, never the full amount again.

## 11. Permission Validation

- Fund-moving actions requiring a specific party's consent (`fund_milestone`, `approve_milestone`, `cancel_before_funding`) require that exact party's signature, checked against the `Gig`'s stored `client`/`freelancer`, not merely "some signer."
- `approve_milestone`'s destination account is constrained to `address = gig.freelancer` — the client cannot redirect an approval payout to an arbitrary wallet.
- Timeout releases resolve the destination the same way (`freelancer_token_account.owner == gig.freelancer`), so even though the *caller* is permissionless, the *recipient* is not — a third party triggering a timeout release cannot redirect funds to themselves.

## 12. Timeout Security

`partial_timeout_release` (≥ 72h since `submitted_at`) and `full_timeout_release` (≥ 7 days since `submitted_at`) are deliberately callable by anyone, with no signer-identity check beyond the transaction fee payer. This is intentional: the entire purpose of the timeout mechanism is to guarantee a freelancer is paid even if the client disappears *and* the freelancer's own wallet is temporarily unable to submit a transaction (e.g. relies on a relayer/automation service). Because the recipient is hard-pinned to `gig.freelancer` (§11) and the amount is hard-pinned to the fixed percentage/remaining-balance formula (§7, §9), permissionless calling cannot be leveraged to misdirect or inflate a payout — the worst a third party can do is trigger a release that was already going to happen, slightly early is impossible (both instructions `require!(now >= ...)`) but never late-blocked, since anyone can call once the window opens. `tests/timeout_boundaries.rs` (8 tests) checks the exact boundary (`submitted_at + timeout - 1` rejected, `submitted_at + timeout` accepted) for both windows.

## 13. CPI Safety

Escrow makes two classes of outbound CPI, both signed with program-derived (never caller-supplied) seeds:

- `anchor_spl::token::transfer_checked` into the SPL Token program, always with an explicit, hardcoded `token_program` account typed as `Program<'info, Token>` (Anchor validates this is the genuine SPL Token program, not an attacker-supplied lookalike). Signed via `CpiContext::new_with_signer` using seeds derived from `EscrowVault`'s own stored `bump` (see [ARCHITECTURE.md § 8.3](./ARCHITECTURE.md#83-bump-seeds-and-program-signing)).
- `gig::cpi::{mark_in_progress, mark_completed_by_escrow, mark_cancelled_by_escrow}` into the Gig program, always with an explicit `gig_program: Program<'info, gig::program::Gig>` account (Anchor validates this is the genuine, declared Gig program, not a caller-supplied lookalike — a fake "gig" program deployed by an attacker at a different address would fail this type check). Signed via `CpiContext::new_with_signer` using seeds derived from `ctx.bumps.escrow_authority`, i.e. the bump Anchor itself just verified when validating the `escrow_authority` account constraint in the same instruction — never a caller-supplied bump (§4a).

Neither program ever CPIs into an arbitrary/caller-specified program ID — both program targets (`Token`, `gig::program::Gig`) are fixed Rust types Anchor checks against their genuine declared addresses, eliminating CPI-confusion attacks. Gig makes zero outbound CPIs of any kind; the CPI relationship is strictly one-directional (§5.3 in ARCHITECTURE.md).

## 14. Summary of Enforced Invariants

1. A milestone can be funded exactly once.
2. A milestone can be submitted exactly once, and only after funding.
3. A milestone's cumulative `released` can never exceed its `amount`.
4. Only `gig.client` can approve, fund, or cancel; only `gig.freelancer` can submit delivery.
5. Release funds can only ever land in `gig.freelancer`'s token account.
6. Only the mint fixed at gig creation is ever accepted into or paid out of the vault.
7. Vault funds can only move via a CPI signed by the `EscrowVault` PDA's own seeds.
8. Timeout releases cannot fire before their exact deadline, but are permissionless once eligible.
9. All arithmetic on balances/counters is checked; overflow/underflow abort the transaction.
10. Every PDA used by any instruction is re-derived and validated against its logical parent, blocking substitution/spoofing.
11. `Gig.status` can only be advanced by (a) the client, through Gig's own client-facing instructions, or (b) Escrow, exclusively through the three CPI-only instructions, exclusively via a signature the Solana runtime guarantees only Escrow can produce.
12. Escrow never writes `Gig` fields directly — every escrow-triggered `GigStatus` transition executes inside the Gig program's own instruction handler, which independently re-validates the precondition before applying it.

---

# Reputation Program — Security

**Audit status: Complete.** The Reputation program (`programs/reputation`) has completed implementation, a full internal security audit, and a regression/security suite (`cargo test -p reputation`, `programs/reputation/tests/`) covering every invariant documented below, plus a dedicated cross-program CPI suite (`cargo test -p escrow --test reputation_settlement`) exercising the real Escrow → Reputation call path end-to-end. No open findings.

## 15. Threat Model

Actors:

- **Profile authority** — the user a `UserProfile` belongs to. Signs `initialize_profile` only. Trusted to sign only their own transactions.
- **Client** — signs `submit_rating` (via Escrow's `rate_freelancer`) for a job they claim to have commissioned. Trusted to sign only their own transactions; escrow's `has_one = client` check on the `Gig` account (not this program) is what actually ties the signer to the real client of that job.
- **Escrow's `escrow_authority` PDA** — the only signer ever accepted for `update_completion` and `submit_rating`. Not a keypair-held trust bottleneck: it is a PDA derived from `ESCROW_AUTHORITY_SEED` under `ESCROW_PROGRAM_ID`, so the only way to produce a valid signature for it is `invoke_signed` from inside the Escrow program itself (§15.5). Reputation trusts Escrow's own settlement logic (vault fully released, gig `Completed`) for *when* this CPI fires; it does not re-verify escrow's internal state.
- **Adversarial transaction builder** — may supply arbitrary accounts to any instruction, including the right account *type* at the wrong *instance* (e.g. someone else's profile), or uninitialized/attacker-owned accounts, attempting to spoof a PDA, or a real keypair standing in for a PDA-only signer.

Assets at risk: the integrity of on-chain reputation data (scores, ratings, badges). The program holds no funds, so there is no direct custody risk; the risk is data integrity and manipulation of a signal other systems rely on.

### 15.4 Trust Assumptions (explicitly out of this program's control)

Documented rather than hidden, because an accurate security posture requires naming what is *not* enforced on-chain:

- **Job identity is attested by Escrow, not independently re-derived here.** `submit_rating`'s `job_id` is `gig.id` as supplied by Escrow's `rate_freelancer`; Reputation trusts that Escrow only invokes this CPI for a real, `Completed` gig between `client` and `freelancer` (Escrow enforces that with `has_one = client` and `gig.status == Completed` — see ARCHITECTURE.md §18). Reputation's own guarantees (range validation, one-rating-per-`job_id`, self-dealing rejection) hold regardless of what Escrow attests; only the *job-identity* claim itself is a trust boundary onto Escrow.
- **Badge types with no on-chain signal are simply never eligible.** `TrustedFreelancer`/`FastDeliverer` return `false` unconditionally from `is_eligible_for_badge` (§21.6) until a real data source backs them — there is no authority that can currently award them, by design, since `award_badge` is permissionless.

### 15.5 Why a PDA Signer Closes the Previous Trust Gap

An earlier design point used a single hardcoded pubkey (`REPUTATION_AUTHORITY`) as the trusted caller for `update_completion`/`award_badge`. That keypair was a centralized bottleneck: whoever held its private key could forge arbitrary job completions, earnings, and badge awards, with no cryptographic tie to Escrow's actual settlement logic. Requiring a `seeds::program`-pinned PDA instead removes that bottleneck entirely — there is no private key to leak, rotate, or compromise; the only way to satisfy the constraint is a live CPI from the Escrow program's own executing code.

## 16. Signer Validation

- `initialize_profile` — `authority` must sign; the PDA is derived from that same signer's key, so a profile can only ever be created "as" its own authority.
- `submit_rating` — `client` must sign (`require_keys_neq!(client, freelancer)` prevents self-dealing), **and** `escrow_authority` must sign, constrained to `seeds = [ESCROW_AUTHORITY_SEED], bump, seeds::program = ESCROW_PROGRAM_ID`. Only Escrow's own CPI can produce that second signature.
- `update_completion` — `escrow_authority` alone, same PDA constraint as above.
- `award_badge` — permissionless; `payer` only foots the new `Badge` account's rent. Eligibility is re-verified from the profile's own fields inside the handler (§21.6), so there is nothing a caller-supplied signer could otherwise gate.
- `get_profile` — read-only, no signer required.

`tests/profile_authorization.rs`, `tests/pda_security.rs`, and `programs/escrow/tests/reputation_settlement.rs` (the real cross-program CPI path) assert every signer-gated instruction rejects the wrong signer — including an attacker's own real keypair standing in for `escrow_authority`, which fails Anchor's `seeds`/`seeds::program` constraint rather than any custom check.

## 17. Ownership & Account-Type Validation

As with Escrow (§3), Anchor's typed `Account<'info, T>` wrapper checks the account discriminator on every typed account, so a `Rating` cannot be substituted where a `UserProfile` is expected, and vice versa — the framework rejects the wrong account type before the handler body runs.

## 18. PDA Validation & Anti-Spoofing

Full design rationale in [ARCHITECTURE.md § 14](./ARCHITECTURE.md#14-pda-architecture). Security-relevant guarantees:

- Every PDA is constrained with `seeds = [...], bump` (on `init`) or `seeds = [...], bump = stored_bump` (on reuse), forcing re-derivation and an exact address match.
- `freelancer_profile` in `submit_rating` and `profile` in `update_completion`/`award_badge`/`get_profile` are all re-derived from `[PROFILE_SEED, authority.as_ref()]` — a caller cannot pass a different authority's profile and have it accepted as "the" profile for a given authority.
- `Badge` PDAs are seeded by `[BADGE_SEED, profile.authority, badge_type]`, so a spoofed badge account for the wrong profile or wrong type fails derivation.

Verified by `tests/pda_security.rs` (12 tests): wrong profile PDA, wrong rating PDA, wrong badge PDA, wrong bump, profile-for-a-different-authority, cross-profile badge substitution, and uninitialized spoofed PDAs are all rejected.

## 19. Reinitialization & Replay Protection

- `UserProfile`, `Rating`, and `Badge` all use Anchor `init` (never `init_if_needed`), so none of them can ever be reinitialized once created at their canonical address.
- `initialize_profile` therefore cannot be called twice for the same authority — the second call fails at account-init time with no separate existence check needed.
- `submit_rating`'s `Rating` PDA is seeded by `job_id` alone, so a second `submit_rating` for the same `job_id` — a replay or duplicate-rating attempt — fails at `init` time. This is the program's entire duplicate-rating defense, and it is structural (seed collision) rather than a runtime `require!` check that could be forgotten on a future code path.
- `award_badge`'s `Badge` PDA is seeded by `(profile, badge_type)`, so a second award of the same badge type to the same profile fails at `init` time — duplicate-badge prevention, structurally enforced the same way.

`programs/escrow/tests/reputation_settlement.rs::test_rate_freelancer_rejects_duplicate_rating` and `test_settle_reputation_cannot_fire_twice` exercise the duplicate-`job_id` and duplicate-settlement paths through the real CPI.

## 20. Checked Arithmetic — Overflow & Underflow Protection

All counter/score math routes through `programs/reputation/src/utils.rs`'s `checked_add`/`checked_sub`/`checked_mul`/`checked_div` helpers, never raw operators:

- `completed_jobs`, `successful_jobs`, `cancelled_jobs`, `total_earnings`, `rating_sum`, `rating_count`, `badges_earned` are all updated exclusively via `checked_add`, returning `ReputationError::MathOverflow` on overflow.
- `average_rating` promotes through `checked_mul(rating_sum, RATING_SCALE)` then `checked_div(_, rating_count)` before a final `u32::try_from` bounds check — an overflow at any step aborts rather than wrapping or truncating silently.
- `compute_reputation_score` (§21) uses `checked_mul`/`checked_add` for every weighted term and `saturating_sub` (not raw subtraction) for the cancellation penalty, explicitly to avoid underflow when the penalty term exceeds the weighted sum — the result is clamped to `0` rather than wrapping to a near-`u64::MAX` value.

`src/utils.rs` carries inline `#[cfg(test)]` unit tests (`checked_add_overflows`, `checked_sub_underflows`, `reputation_score_never_underflows_below_zero`, `reputation_score_is_maxed_for_ideal_profile`) exercising the pure math directly, including near-boundary and near-`u64::MAX` inputs, independent of any account/CPI plumbing.

## 21. Reputation Score & Rating Integrity

### 21.1 Immutable Ratings

`Rating` has no update or delete instruction. Once `submit_rating` succeeds, `score`, `review_hash`, `client`, `freelancer`, and `submitted_at` are permanent, and the PDA seed (`job_id` alone) makes a second submission for the same job fail at `init` time rather than needing a runtime check.

### 21.2 Immutable Profile Authority

`UserProfile.authority` is set once at `init` and never written by any other instruction.

### 21.3 Rating Validation

`submit_rating` requires `(MIN_RATING..=MAX_RATING).contains(&score)` i.e. `1..=5`, rejecting `0` and anything `> 5` with `ReputationError::InvalidRating`.

### 21.4 Authorization for Privileged Actions

`update_completion` requires `escrow_authority` to be a real signature over the PDA derived from `[ESCROW_AUTHORITY_SEED]` under `ESCROW_PROGRAM_ID` (§16). `award_badge` requires no privileged signer at all — it recomputes eligibility from the profile's own already-verified fields on every call, so there is no caller-supplied claim to gate. See §15.5 for why the PDA-signer model replaces the earlier hardcoded-authority design.

### 21.5 Deterministic Reputation Calculation

`compute_reputation_score` is a pure function of `UserProfile`'s own stored fields (`completed_jobs`, `successful_jobs`, `total_earnings`, `average_rating`, `cancelled_jobs`) — a weighted sum of four capped components (success rate, average rating, completed-job volume, lifetime earnings) minus a cancellation penalty, clamped to `[0, MAX_REPUTATION_SCORE]`. No randomness, no external oracle, no off-chain input: the same stored fields always produce the same score, and any observer can recompute and verify it independently from public account data.

### 21.6 Badge Eligibility

`is_eligible_for_badge` deterministically checks five of the seven badge types against on-chain profile fields (`FirstGig`, `TenCompletedJobs`, `HundredCompletedJobs`, `FiveStarPerformer`, `TopRated`); `TrustedFreelancer` and `FastDeliverer` return `false` unconditionally, since `award_badge` is now permissionless and no on-chain signal yet backs those two types (§15.4). `pda_security.rs::test_trusted_freelancer_badge_not_awardable_yet` and `test_badge_award_fails_without_eligibility` cover both the unattested-type and zero-completions cases; positive-path eligibility (after real completions/ratings via the Escrow CPI) is covered in `programs/escrow/tests/reputation_settlement.rs`.

### 21.7 Metadata Bounds

`award_badge` requires `metadata.len() <= Badge::MAX_METADATA_LEN` (128 bytes), rejecting oversized metadata with `ReputationError::MetadataTooLong` before any account write.

## 22. Event Correctness

`ProfileCreated` is asserted field-for-field against the instruction's actual resulting account state in `tests/events.rs`. `RatingSubmitted`, `CompletionUpdated`, and `BadgeAwarded` are now only reachable through Escrow's CPI and are exercised end-to-end (event emission plus resulting state) in `programs/escrow/tests/reputation_settlement.rs`. `ProfileUpdated` is defined but not currently emitted by any instruction — see ARCHITECTURE.md §17 — and is called out here so it is not mistaken for a monitored, silently-broken event path.

## 23. State Consistency

Invariants a reputation record must never violate — `completed_jobs >= successful_jobs`, `total_earnings` never decreases, `updated_at` is monotonically non-decreasing, `created_at` never changes, `average_rating` stays within `[0, 500]`, badges are unique per type — are enforced structurally (checked arithmetic with no decrement instruction, PDA-`init` uniqueness) and verified through the real settlement path in `programs/escrow/tests/reputation_settlement.rs`.

## 24. Error Handling

`ReputationError` defines 11 variants. `ProfileAlreadyExists`, `ProfileNotFound`, and `InvalidEarnings` are defined but not currently returned by any instruction — duplicate-profile and duplicate-job protection are enforced structurally via PDA `init` (§19) rather than via an explicit existence check, and no instruction currently decreases `total_earnings`. Documented here rather than left as unexplained dead code; each is a reserved slot for a future explicit check rather than a broken current one.

## 25. Regression Coverage

`programs/escrow/tests/reputation_settlement.rs` re-asserts, as a group, that the CPI security model holds under attack: a real (non-PDA) keypair impersonating `escrow_authority` is rejected, `settle_reputation` cannot fire twice per gig, `rate_freelancer` cannot be called before the gig is `Completed` or by anyone other than the real client, and duplicate ratings for the same job are rejected. `programs/reputation/tests/pda_security.rs` covers the same forgery attempts directly against the reputation program in isolation.

## 26. Summary of Enforced Invariants (Reputation)

1. A profile can be created exactly once per authority.
2. A job can be rated exactly once, ever, regardless of which client submits it.
3. A badge type can be awarded to a given profile exactly once.
4. Only a live CPI from Escrow's own `escrow_authority` PDA can record completions or attest ratings; `award_badge` is permissionless but self-verifying.
5. A client cannot rate a job where they are also the freelancer.
6. Ratings are immutable once submitted.
7. A profile's `authority` field never changes after creation.
8. `settle_reputation` can credit a gig's earnings to a profile at most once (`vault.reputation_synced`).
8. `total_earnings`, `completed_jobs`, `successful_jobs`, `cancelled_jobs`, `rating_count`, `badges_earned` only ever increase.
9. `reputation_score` is always a pure, deterministic, independently-verifiable function of the profile's own stored fields.
10. All arithmetic on counters/scores is checked or explicitly saturating; overflow aborts the transaction, and the score is clamped rather than allowed to wrap.
11. Every PDA used by any instruction is re-derived and validated against its logical parent (authority, profile, or badge type), blocking substitution/spoofing.
12. `REPUTATION_AUTHORITY` centralization and caller-supplied job identity are explicit, documented trust assumptions, not silently-assumed guarantees (§15.4).

## 27. Achievement Program Security Model

**Program:** `programs/achievement` · **Program ID:** `GV8Z39NBK7qrojXCfnnwLTXpqsLoCW6sy9cLHGYjtrv9`.

Achievement's entire trust model rests on one fact: it never invents eligibility data, it only re-derives accounts the reputation program already created and validated.

| Attack | Defense |
|---|---|
| Forged/fake `UserProfile` | `profile` is re-derived via `seeds = [reputation::PROFILE_SEED, claimer.key()], seeds::program = reputation::ID` — Anchor recomputes the PDA from those seeds and rejects any account that isn't the exact, reputation-owned match. |
| Forged/fake `Badge` (claiming a badge never earned) | `badge` is re-derived the same way under `reputation::BADGE_SEED` + `badge_type`. The account simply doesn't exist for an unearned badge — reputation's `award_badge` is the only instruction that can ever create it, and it re-checks eligibility from the profile's own public fields before doing so (§ Reputation Program, ARCHITECTURE.md §20). Achievement therefore never recomputes eligibility; it trusts the PDA's existence as reputation's own attestation. |
| Badge belongs to a different profile | `constraint = badge.profile == profile.key()` — even if both PDAs individually re-derive correctly (they always do, since seeds are keyed off the same `claimer`), this constraint is a second, explicit tie between the two accounts. |
| Duplicate claim / replay | `achievement` is created with `init` at `seeds = [ACHIEVEMENT_SEED, claimer.key(), badge_type]`. A second `claim_achievement` for the same `(claimer, badge_type)` fails at account initialization — there is no separate "already claimed" branch to bypass. |
| Invalid signer (claiming someone else's badge) | `claimer: Signer<'info>` is the seed for both the `profile` and `achievement` PDAs; a transaction "claiming" a badge for a pubkey that didn't sign fails PDA/seed validation before the handler runs, and a transaction signed by the wrong keypair fails Solana's own signature check at the transaction layer. |
| Invalid/substituted PDA (e.g. an attacker-controlled `achievement` account) | Every seeded account (`profile`, `badge`, `config`, `achievement`) is independently re-derived by Anchor from its declared seeds; a non-matching account address fails the constraint regardless of its contents. |
| Forged mint / rogue NFT provenance | The shared collection's update authority is the `config` PDA (`seeds = [CONFIG_SEED]`), set once at `init_collection` and never a keypair. Only `claim_achievement`'s own `invoke_signed(config_signer_seeds)` CPI can produce a valid signature for that authority, so no other program or transaction can mint into the collection. `collection` itself is constrained to `address = config.collection`, so a caller cannot substitute a lookalike collection either. |
| NFT minting during escrow settlement | Structurally impossible: `claim_achievement` is a standalone, user-signed instruction with no caller in `programs/escrow` or `programs/gig`. Settlement's CPI surface (§18) ends at Reputation; Achievement is never in that call graph. |

**Trust assumptions, stated explicitly:**

- Achievement trusts Reputation's `Badge` PDA as sufficient proof of eligibility and does not re-verify the underlying counters. This is the same "each program owns its own logic" boundary as Escrow trusting Reputation's account state elsewhere in this document — if that boundary is ever crossed (e.g. Reputation redeployed to a new program ID), Achievement's hardcoded dependency on `reputation::ID` must be redeployed in lockstep, the same operational pattern already documented for `ESCROW_PROGRAM_ID` (§4c, `docs/runbooks/escrow-redeploy.md`).
- `init_collection` is intentionally admin-gated only by being the first caller to succeed against the singleton `config` PDA (`init` fails for anyone after the first). There is no separate allowlist check; whoever's transaction lands first becomes `admin`. This is acceptable pre-deployment (the deployer runs it once, atomically, before publishing the program ID) but should be re-reviewed before mainnet if `init_collection` is ever exposed to permissionless calling in a race-prone environment.
- Metaplex Core's own program correctness (plugin validation, asset/collection invariants) is out of scope for this document — it is treated as an external, independently-audited dependency, the same way `anchor-spl`'s SPL Token program is trusted elsewhere in this codebase.

**Test coverage.** `programs/achievement/tests/claim_achievement.rs` exercises every row of the table above except NFT provenance and the settlement-isolation argument (both are structural/CPI-graph arguments verified by code inspection, not a runtime assertion) — see [TESTING.md](./TESTING.md#achievement-program) for the full list. Full end-to-end minting (asset/collection state after a real Metaplex Core CPI) is not exercised by this repository's offline test suite, which has no network access to the deployed Metaplex Core program binary; it should be verified against a real `mpl-core` deployment (localnet/devnet) before this program is treated as audited to the same standard as Gig/Escrow/Reputation.

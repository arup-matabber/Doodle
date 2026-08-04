# Gig + Escrow Programs — Architecture

Status: **Implemented and internally audited; not deployed.** Programs: `programs/gig` (Anchor, `declare_id!("9LpGZY8p8dYfYdWm5D9MuvGXh9VXdF8DqEEAmdNZ92Na")`) and `programs/escrow` (Anchor, `declare_id!("FFJ8YAVGUJP4SeDZrQ3g1d9fdQFq9hutsU1m4f3o1UXS")`).

Gig and Escrow are two independently-deployable programs that cooperate through a small, one-directional CPI surface. Gig owns job metadata and lifecycle; Escrow owns milestone funds and payment lifecycle. Neither embeds the other's state — they reference each other only by account (Gig's pubkey, stored on Escrow's `Milestone`/`EscrowVault`) and by CPI (Escrow calling three narrowly-scoped, escrow-only instructions on Gig).

## 1. Protocol Overview

PayGig splits on-chain responsibility into independent programs instead of one monolith:

```
┌─────────────┐      ┌──────────────┐      ┌──────────────┐      ┌────────────────┐
│     Gig      │─CPI─▶│   Escrow     │      │  Reputation  │      │    Dispute      │
│ (job/status) │◀─CPI─│  (payments)  │      │  (scoring)   │      │  (not yet live) │
└─────────────┘      └──────────────┘      └──────────────┘      └────────────────┘
```

Gig, Escrow, and Reputation are all implemented and internally audited; the Dispute program is an empty placeholder. None are deployed to a public cluster yet.

- **Gig** owns exactly one job: **describe a piece of work and track who's doing it, in what status.** It never touches tokens.
- **Escrow** owns exactly one job: **hold client funds and release them to the freelancer according to a fixed set of rules.** It never owns gig metadata — it reads a `Gig` account it doesn't own, and only ever *reads* its fields (`client`, `freelancer`, `mint`, `status`, `id`, `bump`) to validate against, never writes them directly.

The only writes Escrow causes on a `Gig` account happen through three CPI-only Gig instructions (§5.3) — Escrow proposes a state transition, Gig's own program enforces whether that transition is legal and executes it.

## 2. Why Job Metadata and Payments Are Separate Programs

- **Smaller attack surface per program.** Gig's instructions never move a token; Escrow's `Gig`-touching instructions never write gig metadata. An auditor reviewing the money path (`programs/escrow/src/instructions/{create,fund}_milestone.rs`, `approve_milestone.rs`, `*_timeout_release.rs`) never has to also reason about title-length validation or category strings.
- **Independent deployability.** Gig's client-facing lifecycle (listings, drafts, categories) can evolve — new fields, new validation, new instructions — without touching Escrow's declared program ID, IDL, or audited vault-custody code, and vice versa.
- **Single source of truth, no duplicated metadata.** Escrow's `Milestone` and `EscrowVault` store only `gig: Pubkey` — never a copy of title/description/skills/category/deadline/visibility. Anything about the job itself is read once, live, from the Gig program's own account; there is no second copy that can drift out of sync.
- **Least-privilege CPI.** Escrow does not have general write access to `Gig` — it can trigger exactly three, narrowly-defined transitions (`mark_in_progress`, `mark_completed_by_escrow`, `mark_cancelled_by_escrow`), each gated by Gig's own status-precondition check and by a signer PDA only Escrow can produce (§5.4). Escrow cannot, for instance, change a gig's title, reassign its freelancer, or force it into `Published`.
- **Failure isolation.** A bug in Gig's listing/metadata logic cannot corrupt vault accounting, because Escrow never lets Gig write to `Milestone` or `EscrowVault` — the CPI direction is one-way (Escrow → Gig only; Gig never calls back into Escrow).

## 3. Why Reputation and Disputes Are Separate Programs

The product vision (`docs/details.md`) describes future composition — escrow calling into reputation on approval, escrow handing off to a dispute program on conflict. Deliberately, the audited escrow program **does not implement those CPIs yet**. Reputation (`programs/reputation`) is a standalone, independently-deployed program today; the dispute program (`programs/dispute`) is not yet implemented (placeholder only).

This separation is intentional, not an oversight:

- A rating or a badge mint is a *side effect* of a client being happy, not a precondition of the client getting what they paid for. Escrow must succeed even if reputation-scoring logic is unavailable, buggy, or mid-upgrade.
- Cross-program calls widen the trust boundary of the escrow program to include the reputation program's account validation. Keeping them separate means an escrow security audit doesn't have to also audit reputation's account constraints.
- Future CPI wiring (escrow → reputation on `approve_milestone`) can be added later as an additive change without touching vault custody logic, once the reputation program has undergone its own audit.

## 4. Account Architecture

### 4.1 Gig Account (PDA — owned by `programs/gig`)

```
Gig {
  id: u64
  client: Pubkey
  freelancer: Pubkey       // Pubkey::default() until assign_freelancer
  mint: Pubkey             // SPL mint every milestone is funded in (e.g. USDC)
  status: GigStatus        // Draft | Published | Assigned | InProgress | Completed | Cancelled | Archived
  created_at: i64
  updated_at: i64
  title: String            // <= MAX_TITLE_LEN (100)
  description: String      // <= MAX_DESCRIPTION_LEN (500)
  skills: String           // <= MAX_SKILLS_LEN (200); empty at init, set via update_gig
  category: String         // <= MAX_CATEGORY_LEN (50)
  budget: u64              // > 0; advertised budget, independent of milestone amounts
  deadline: i64            // must be > now + MIN_DEADLINE_SECS (1 day)
  bump: u8
}
```

One `Gig` account exists per engagement between a client and a freelancer. It is the root of trust for every other account in the tree — `has_one` constraints on `client`/`freelancer` throughout both programs all resolve back to this account. Escrow imports this exact type from the `gig` crate (`gig = { path = "../gig", features = ["cpi"] }`); it never defines its own copy, so there is only ever one on-chain (and one Rust-type) definition of what a Gig is.

The gig also carries its own marketplace metadata (title, description, skills, category, budget, deadline), so a listing is fully describable from on-chain state without an off-chain database. `budget` is advertised intent; the amounts actually escrowed are the per-milestone `amount` fields. Note `milestone_count`/`active_milestone` are **not** stored here — they moved to `EscrowVault` (§4.3) because they are payment-lifecycle bookkeeping, not job metadata; keeping them off the Gig account means Gig never needs to know how many milestones exist or how many have cleared.

### 4.2 Milestone Account (PDA — owned by `programs/escrow`)

```
Milestone {
  gig: Pubkey
  index: u32
  amount: u64
  released: u64
  status: MilestoneStatus  // PendingFunding | Funded | Submitted | PartialReleased | Completed
  submitted_at: i64
  approved_at: i64
  bump: u8
}
```

Milestones are created sequentially (`index` == `vault.milestone_count` at creation time) and are independently funded, delivered, and released. `released` tracks cumulative payout so partial timeout releases and the final release never double-pay. `Milestone` stores only `gig: Pubkey` — never a copy of any Gig metadata — so the Gig program remains the single source of truth for everything about the job itself.

### 4.3 Vault (EscrowVault PDA + Vault Token Account — owned by `programs/escrow`)

```
EscrowVault {
  gig: Pubkey
  token_account: Pubkey
  mint: Pubkey
  total_locked: u64
  total_released: u64
  milestone_count: u32     // milestones created for this gig (escrow-owned counter)
  active_milestone: u32    // milestones fully released so far
  bump: u8
}
```

One vault per `Gig`, shared by every milestone under that gig. `EscrowVault` is a small bookkeeping account; the actual SPL tokens live in a separate **Vault Token Account** (an `spl-token` `TokenAccount` whose `authority` is the `EscrowVault` PDA itself). `total_locked`/`total_released` are running counters checked against actual token balances by the test suite. `milestone_count`/`active_milestone` live here (not on `Gig`) precisely because they are Escrow's own payment-lifecycle state — Gig never needs to track "how many milestones" for a job it doesn't fund.

### 4.4 Account Relationships

```
Gig (1, programs/gig) ──┬──< Milestone (N, programs/escrow, seeded by gig + index)
                         │
                         └──< EscrowVault (1, programs/escrow, seeded by gig)
                                   │
                                   └── Vault Token Account (1, seeded by gig + "token", authority = EscrowVault)
```

`Gig` is the only account in this tree owned by a different program than the rest — Escrow reads it via `Account<'info, gig::Gig>` (Anchor enforces the owner check automatically from the imported type's declared program ID) but never `init`s, closes, or directly mutates it.

## 5. Instruction Flow / State Machine

The protocol exposes **7 Gig instructions** (4 client lifecycle + 3 CPI-only) and **7 Escrow instructions** (all payment/milestone), cooperating through the CPI surface in §5.3–5.4.

### 5.1 Gig lifecycle (client-driven, `programs/gig`)

```
initialize_gig ──► Draft ──publish_gig──► Published ──assign_freelancer──► Assigned
                     │                        │                              │
                  update_gig                  │                        complete_gig
                  (Draft only)                │                     (no-escrow path only)
                     │                        │                              │
                     └──────── cancel_gig ────┴──────────────►          Completed ──archive_gig──► Archived
                            (Draft/Published/                                ▲
                             Assigned only)                                  │
                                              │                    mark_completed_by_escrow (CPI)
                                              ▼                              │
                                          Cancelled ◄── mark_cancelled_by_escrow (CPI)
                                              ▲                              │
                                              │                        InProgress
                                              └──────────────────── mark_in_progress (CPI)
                                                                      (from Assigned)
```

- `update_gig` is valid only in `Draft`; once published, listing metadata is frozen.
- `assign_freelancer` requires `Published`, rejects the client assigning themselves, and rejects reassignment once `gig.freelancer` is set.
- `cancel_gig` (client-signed) is valid from `Draft`, `Published`, or `Assigned` only — once a gig is `InProgress` (i.e. escrow has funded at least one milestone), cancellation must go through Escrow's `cancel_before_funding` → `mark_cancelled_by_escrow` CPI (§5.3), since only Escrow knows whether funds are already locked.
- `complete_gig` (client-signed) is the manual path for a gig that never entered escrow (no milestones ever funded) — it requires `Assigned`, not `InProgress`. Once a gig has been funded, only Escrow can complete it, via `mark_completed_by_escrow`.
- `Completed`, `Cancelled`, and `Archived` are terminal.
- Every client-facing instruction requires the client's signature and `has_one = client`.

### 5.2 Milestone / escrow flow (`programs/escrow`)

```
create_milestone (requires gig.status == Assigned || InProgress)
      │
      ▼
Milestone::PendingFunding
      │
      ▼ fund_milestone (client transfer_checked → vault)
      │      └─ if gig.status == Assigned: CPI mark_in_progress ──► Gig: Assigned → InProgress
Milestone::Funded
      │
      ▼ submit_delivery (freelancer)
Milestone::Submitted ──► submitted_at = now
      │
      ├── approve_milestone (client, any time) ─────────► Milestone::Completed  (100% remaining released)
      │
      ├── partial_timeout_release (permissionless, now ≥ submitted_at + 72h)
      │         └──► Milestone::PartialReleased (20% released)
      │                     │
      │                     └── full_timeout_release (permissionless, now ≥ submitted_at + 7d)
      │                               └──► Milestone::Completed (remaining 80% released)
```

`cancel_before_funding` is the only exit from `PendingFunding`: it closes the milestone (rent refunded to client) and, via CPI, marks the gig `Cancelled` (§5.3). It is unavailable once a milestone has been funded — funds can only leave the vault through `approve_milestone`, `partial_timeout_release`, or `full_timeout_release`.

When the last milestone under a gig reaches `Completed` (`vault.active_milestone + 1 >= vault.milestone_count`, via approval or full timeout), Escrow CPIs into `mark_completed_by_escrow`, flipping `Gig.status` to `Completed` in the same transaction.

### 5.3 The CPI Surface (Escrow → Gig)

Escrow calls exactly three Gig instructions, each gated by Gig's own precondition check — Escrow proposes, Gig enforces:

| Gig instruction | Called from | Precondition (checked by Gig) | Transition |
|---|---|---|---|
| `mark_in_progress` | `fund_milestone`, on the gig's first funding | `gig.status == Assigned` | `Assigned → InProgress` |
| `mark_completed_by_escrow` | `approve_milestone` / `full_timeout_release`, on the last milestone | `gig.status == InProgress` | `InProgress → Completed` |
| `mark_cancelled_by_escrow` | `cancel_before_funding` | `gig.status ∈ {Assigned, InProgress}` | `→ Cancelled` |

Gig never calls back into Escrow — the CPI direction is strictly one-way. Escrow's `Gig`-touching instructions never write `Gig` fields directly; every mutation of `GigStatus` triggered by escrow activity happens *inside the Gig program's own instruction handler*, reached only via CPI.

### 5.4 CPI Authorization — the `escrow_authority` Signer PDA

Each of the three CPI-only Gig instructions requires an `escrow_authority: Signer<'info>` account constrained by:

```rust
#[account(
    seeds = [ESCROW_AUTHORITY_SEED],   // b"escrow_authority"
    bump,
    seeds::program = ESCROW_PROGRAM_ID, // Escrow's declare_id!, hardcoded in programs/gig/src/constants.rs
)]
pub escrow_authority: Signer<'info>,
```

`seeds::program` tells Anchor (and, underneath it, the Solana runtime) that this PDA must have been derived — and *signed for* via `invoke_signed` — by the program whose ID is `ESCROW_PROGRAM_ID`. The runtime itself enforces this: a program can only produce a valid PDA signature for seeds derived under *its own* program ID, so no program other than Escrow can ever construct a valid signer for `[b"escrow_authority"]` under `ESCROW_PROGRAM_ID`. This is the entire trust mechanism — Gig does not maintain an allow-list, does not check `instruction_sysvar` introspection, and does not require any registration step; the cryptographic guarantee is structural. See SECURITY.md for the corresponding negative tests (calling these instructions directly, or from a forged signer, is rejected).

`ESCROW_PROGRAM_ID` is a compile-time constant, deliberately duplicated (not shared via a mutable registry) in both `gig` and `reputation` — see [SECURITY.md §4c](./SECURITY.md#4c-escrow-program-id-trust-assumption-operational) for why, how consistency is enforced at build time, and [docs/runbooks/escrow-redeploy.md](./docs/runbooks/escrow-redeploy.md) for the redeploy procedure.

## 6. Event Architecture

Every state-changing instruction emits a typed Anchor event, giving off-chain indexers (e.g. `services/reputation-indexer`) a complete, replayable log without needing to poll account state. Events are now split across two programs' `events.rs`:

### 6.1 Gig events (`programs/gig/src/events.rs`)

| Event | Emitted by |
|---|---|
| `GigCreated` | `initialize_gig` |
| `GigUpdated` | `update_gig` |
| `GigPublished` | `publish_gig` |
| `FreelancerAssigned` | `assign_freelancer` |
| `GigInProgress` | `mark_in_progress` (CPI from Escrow) |
| `GigCompleted` | `complete_gig` (manual) and `mark_completed_by_escrow` (CPI) |
| `GigArchived` | `archive_gig` |
| `GigCancelled` | `cancel_gig` (manual) and `mark_cancelled_by_escrow` (CPI) |

### 6.2 Escrow events (`programs/escrow/src/events.rs`)

| Event | Emitted by |
|---|---|
| `MilestoneCreated` | `create_milestone` |
| `MilestoneFunded` | `fund_milestone` |
| `DeliverySubmitted` | `submit_delivery` |
| `MilestoneApproved` | `approve_milestone` |
| `PartialReleaseExecuted` | `partial_timeout_release` |
| `FullReleaseExecuted` | `full_timeout_release` |
| `MilestoneCancelledBeforeFunding` | `cancel_before_funding` |

One event per state-changing instruction in each program. Coverage is verified directly in each program's `tests/events.rs` — every instruction's event fields are asserted against the instruction's actual effects.

## 7. Token Flow / SPL Token CPI Architecture

The program never holds token authority itself — it always CPIs into the SPL Token program using `transfer_checked` (mint-aware, decimal-checked transfers, not the legacy `transfer`):

```
fund_milestone:
  client_token_account ──transfer_checked (client signs)──► vault_token_account

approve_milestone / partial_timeout_release / full_timeout_release:
  vault_token_account ──transfer_checked (EscrowVault PDA signs via seeds)──► freelancer_token_account
```

- **Inbound (funding):** the client is the transaction signer and the token-account authority; the program does not need PDA signing to pull funds in.
- **Outbound (release):** only the `EscrowVault` PDA can authorize a debit from the vault token account. The program supplies `signer_seeds` derived from `[VAULT_SEED, gig, bump]` to `CpiContext::new_with_signer`, so only this exact program, for this exact gig, can move vault funds.
- `mint.decimals` is passed to every `transfer_checked` call and the mint is constrained (`has_one = mint`, `token::mint = mint`) at every account boundary that touches a token account, so a caller cannot substitute a token account belonging to a different mint.

## 8. PDA Architecture

### 8.1 Why PDAs

Every stateful account in the program — `Gig`, `Milestone`, `EscrowVault`, and the vault's SPL token account — is a Program Derived Address. A PDA is computed deterministically from a set of seed bytes plus the program ID; it has **no corresponding private key**, so nothing can produce a valid `Ed25519` signature for it. The only way to authorize an action "as" a PDA is for the *owning program* to invoke `invoke_signed` with the exact seeds that derive it. This is what lets the escrow program act as a custodian: it can prove control of vault funds without ever holding a private key that could leak, be phished, or be reused elsewhere.

### 8.2 Deterministic Address Generation

Every PDA in this program is derived from `Pubkey::find_program_address(seeds, program_id)`, where the seeds tie the account unambiguously to the data it represents:

| PDA | Seeds | Rationale |
|---|---|---|
| **Gig PDA** | `[GIG_SEED, id.to_le_bytes()]` | `id` is caller-supplied; deriving from it means the client and freelancer never need to pass around a generated address out-of-band — anyone can recompute the gig's address from its `id` alone. **Note: the client is not a seed**, so gig ids share one global namespace; the first client to create id *N* owns it, and any later `initialize_gig` with the same id fails at `init`. Clients must allocate ids collision-aware (e.g. randomly) |
| **Milestone PDA** | `[MILESTONE_SEED, gig.key(), gig.milestone_count.to_le_bytes()]` | Binds every milestone to its exact parent gig and its sequential position (the index is the gig's current `milestone_count`, incremented in the same instruction); two different gigs can never collide on a milestone address, and milestone `N` for a gig is always at the same deterministic address |
| **Vault PDA (`EscrowVault`)** | `[VAULT_SEED, gig.key()]` | One vault per gig; deriving from the gig alone (not from a milestone) is what lets multiple milestones share a single vault and its running `total_locked`/`total_released` counters |
| **Vault Token Account** | `[VAULT_SEED, gig.key(), b"token"]` | A distinct sub-seed from the `EscrowVault` bookkeeping PDA, so the SPL token account and the bookkeeping struct are two separately-addressable accounts even though they represent the same vault |
| **Escrow Authority PDA** | `[ESCROW_AUTHORITY_SEED]` (`b"escrow_authority"`), under the Escrow program | Escrow's own CPI-signer identity — not a data account, never `init`ed, exists purely so Escrow can `invoke_signed` into Gig's CPI-only instructions (§5.4). Verified on the Gig side via `seeds::program = ESCROW_PROGRAM_ID` |

`GIG_SEED` lives in `programs/gig/src/constants.rs`; `MILESTONE_SEED`, `VAULT_SEED`, and `ESCROW_AUTHORITY_SEED` live in `programs/escrow/src/constants.rs`. All are `#[constant]` byte strings, embedded in each program's IDL for client-side address derivation.

### 8.3 Bump Seeds and Program Signing

Each PDA account stores its own `bump: u8`, captured at creation time (`ctx.bumps.<account>`) via Anchor's `seeds`/`bump` account constraints. Storing the bump (rather than recomputing it on every instruction) is both a gas/compute optimization and a security property: instructions that need to *sign* on behalf of a PDA (`approve_milestone`, `partial_timeout_release`, `full_timeout_release`) build `signer_seeds` from the *stored* bump —

```rust
let signer_seeds: &[&[&[u8]]] = &[&[VAULT_SEED, gig_key.as_ref(), &[vault_bump]]];
```

— and pass them to `CpiContext::new_with_signer`. The same pattern secures the CPI into Gig: `fund_milestone`, `approve_milestone`/`full_timeout_release`, and `cancel_before_funding` each build `&[&[ESCROW_AUTHORITY_SEED, &[ctx.bumps.escrow_authority]]]` and pass it to `CpiContext::new_with_signer` when calling `gig::cpi::mark_in_progress` / `mark_completed_by_escrow` / `mark_cancelled_by_escrow` — the runtime independently confirms this signature could only have been produced by the Escrow program itself (§5.4). The runtime independently re-derives the PDA from these seeds and confirms it matches the account being signed for; a caller cannot forge a signature for the vault by supplying a different bump, because Anchor's `bump = vault.bump` constraint on the account already validated that the stored bump reproduces the vault's actual address before the instruction body ever runs.

### 8.4 Why PDAs Have No Private Keys — and Why That Matters Here

Because a PDA sits deliberately off the Ed25519 curve, no keypair exists that could sign for it directly. This is the entire basis of the escrow's custody guarantee:

- **Only the Escrow Program controls escrow funds.** The vault token account's SPL `authority` is set to the `EscrowVault` PDA (`token::authority = vault` in `fund_milestone`). Since nothing can sign for that PDA except an `invoke_signed` call issued by this exact program with these exact seeds, no other program, wallet, or validator operator can move vault funds — not even the client who funded it, and not even the deployer.
- **Clients and freelancers cannot directly move funds.** A client cannot call `spl-token transfer` against the vault token account, because a `TokenAccount`'s SPL-level authority is the PDA, not any user wallet. The only paths that debit the vault are the three release instructions, each of which is gated by its own status/timing/signer checks before the CPI is even reached.
- **PDA validation prevents spoofing.** Every instruction that touches a PDA re-derives and checks it via Anchor's `seeds`/`bump` (on init) or `seeds`/`bump = stored_bump` (on subsequent use) constraints, plus explicit `has_one`/`constraint`/`address` checks tying the PDA back to the expected `Gig`/`Milestone`/mint. An attacker cannot substitute an attacker-controlled account claiming to be "the vault" — Anchor rejects any account whose address doesn't match the expected derivation, and `tests/pda_security.rs` (13 tests) exercises exactly this: wrong gig PDA, wrong milestone PDA, wrong vault PDA, wrong bump, milestone-from-a-different-gig, vault-token mismatch, cross-gig vault substitution, and uninitialized spoofed PDAs are all asserted to fail.

## 9. Security Model (summary — full detail in SECURITY.md)

The vault token account's SPL `authority` field is the `EscrowVault` PDA, which has no private key. The only way to debit it is a CPI signed with that PDA's exact seeds — meaning the *only* code path capable of moving vault funds is the escrow program's own `approve_milestone` / `partial_timeout_release` / `full_timeout_release` handlers, each of which independently re-derives the vault from `[VAULT_SEED, gig]` and checks `vault.bump` and `vault.mint`.

Symmetrically, the only way to write `Gig.status` from outside the Gig program is a CPI signed with the `escrow_authority` PDA's exact seeds under Escrow's own program ID — meaning the *only* code path capable of moving a `Gig` through `mark_in_progress`/`mark_completed_by_escrow`/`mark_cancelled_by_escrow` is Escrow's own instruction handlers, each of which independently re-derives that signer and each of which Gig re-validates against its own status precondition before accepting the transition (§5.3–5.4). See [SECURITY.md](./SECURITY.md).

## 10. Design Rationale Summary

| Decision | Why |
|---|---|
| Milestones are separate PDAs, not a `Vec<Milestone>` inside `Gig` | Fixed account size, no realloc/resize logic, no per-gig cap on milestone count, cheaper writes (only the touched milestone is written) |
| One vault per gig, shared across milestones | Avoids re-creating a token account (and its rent) per milestone; simplifies accounting to one `total_locked`/`total_released` pair per engagement |
| `released` tracked per-milestone, not inferred from balance | Vault token balance alone cannot distinguish "not yet funded" from "already fully paid out" once multiple milestones share a vault |
| Timeout releases are permissionless | A silent client cannot hold a freelancer's payment hostage indefinitely by simply not signing anything — anyone (including automation) can trigger the timeout instructions once the deadline passes |
| Partial (20%) before full (7d) timeout | Gives the freelancer partial relief quickly (72h) while still preserving the client's ability to dispute or object within a longer window before the remainder auto-releases |
| No CPI to reputation/dispute programs | See §3 — isolates the audited payment path from higher-churn, less-audited logic |
| Gig lifecycle is a separate program from Escrow, connected by CPI | Keeps job metadata and payment custody independently auditable and deployable (§2). The atomicity concern from a single-program design is preserved: `mark_in_progress`/`mark_completed_by_escrow`/`mark_cancelled_by_escrow` execute inside the *same transaction* as the milestone action that triggers them, so `Gig.status` and `Milestone`/`EscrowVault` state never observably diverge — Escrow's instruction fails atomically if the CPI fails |
| `milestone_count`/`active_milestone` moved from `Gig` to `EscrowVault` | These are payment-lifecycle counters (how many milestones exist, how many have cleared), not job metadata — keeping them on Escrow's own account means Gig needs zero additional CPI instructions just to keep a counter in sync |
| CPI authorization via a `seeds::program`-constrained signer PDA, not an allow-list or admin flag | The Solana runtime itself guarantees only Escrow can produce a valid signature for `[b"escrow_authority"]` under Escrow's program ID (§5.4) — no mutable "trusted callers" list exists to be misconfigured or to require an upgrade to change |
| Listing metadata (title/description/skills/category/budget/deadline) stored on-chain | A gig is fully describable from chain state alone, so an indexer or an alternative frontend needs no privileged off-chain database to render the marketplace. Cost: a larger `Gig` account and length caps enforced at every write |
| `update_gig` restricted to `Draft` | Once a gig is published, freelancers may have evaluated it; silently mutating budget or deadline after publication would let a client bait-and-switch |

---

# Reputation Program — Architecture

Status: **Implemented and internally audited; not deployed.** Program: `programs/reputation` (Anchor, `declare_id!("mXn62yZ4KFvPsdtMmEdGkB71jXcr17SQJHXftgPVGNB")`).

## 11. Program Overview

The Reputation program is an independently-deployed Anchor program that tracks, on-chain, a freelancer's job-completion history, ratings, and derived reputation score. It has exactly one job: **maintain a tamper-evident, deterministically-recomputable reputation record per authority.** It holds no funds and has no custody responsibilities — see §3 above for why this is a separate program from Escrow rather than a module inside it.

## 12. Responsibilities

- Create one reputation profile per user authority (`initialize_profile`).
- Record an immutable, one-time rating per job (`submit_rating`).
- Record the outcome (success/cancellation) and earnings of a completed job, and recompute the profile's deterministic reputation score (`update_completion`).
- Award badges to profiles that meet deterministic on-chain eligibility criteria, or that a trusted authority attests to (`award_badge`).
- Expose a profile's current reputation score for on-chain composability (`get_profile`).

It does **not** move funds, does not verify that a `job_id` corresponds to a real Escrow `Gig`, and does not itself decide who the "client" and "freelancer" of a job are — those identities are supplied by the caller and are only as trustworthy as the caller. See §14.4 (Trust Assumptions) for the security implications of this.

## 13. Account Model

### 13.1 UserProfile PDA

```
UserProfile {
  authority: Pubkey        // the user this profile belongs to
  completed_jobs: u64
  successful_jobs: u64
  cancelled_jobs: u64
  total_earnings: u64
  rating_sum: u64           // sum of all raw 1-5 scores ever submitted
  rating_count: u64
  average_rating: u32        // rating_sum * 100 / rating_count, i.e. scaled by RATING_SCALE
  reputation_score: u64      // deterministic, recomputed on every mutation, 0..=1000
  badges_earned: u32
  created_at: i64
  updated_at: i64
  bump: u8
}
```

One `UserProfile` per authority. It is the single source of truth every reputation figure is derived from — `reputation_score` is never independently mutated, only recomputed from the other fields (§16).

### 13.2 Rating PDA

```
Rating {
  job_id: u64
  client: Pubkey
  freelancer: Pubkey
  score: u8                 // 1-5
  review_hash: [u8; 32]      // hash of off-chain review text; raw text stays off-chain
  submitted_at: i64
  bump: u8
}
```

One `Rating` per `job_id`. Immutable after creation — there is no `update_rating` or `delete_rating` instruction, so once written a rating can never be edited, retracted, or overwritten (§14.7).

### 13.3 Badge PDA

```
Badge {
  profile: Pubkey            // the UserProfile this badge was awarded to
  badge_type: BadgeType       // FirstGig | TenCompletedJobs | HundredCompletedJobs |
                               // FiveStarPerformer | TrustedFreelancer | FastDeliverer | TopRated
  issuer: Pubkey              // always REPUTATION_AUTHORITY today
  issued_at: i64
  metadata: String            // free-form, max 128 bytes (e.g. a URI)
  bump: u8
}
```

One `Badge` PDA per `(profile, badge_type)` pair — a profile can hold at most one badge of each type (§14.8).

## 14. PDA Architecture

### 14.1 Why PDAs

As with Escrow (§8.1), every stateful account here is a Program Derived Address: computed deterministically from seed bytes and the program ID, with **no corresponding private key**. Nothing can sign for a `UserProfile`, `Rating`, or `Badge` PDA except this program itself, via `invoke_signed` — and in practice this program never needs to sign *outbound* from these PDAs, since it holds no funds. The value of PDAs here is entirely about **deterministic addressing and spoofing resistance**, not custody: anyone can independently recompute a profile's, rating's, or badge's address and be certain it either doesn't exist, or holds exactly the data the program itself wrote.

### 14.2 Seed Derivation

| PDA | Seeds | Rationale |
|---|---|---|
| **UserProfile PDA** | `[PROFILE_SEED, authority.key()]` | One profile per authority, addressable by anyone who knows the authority's pubkey alone — no registry or off-chain index needed to find "Alice's profile" |
| **Rating PDA** | `[RATING_SEED, job_id.to_le_bytes()]` | Keyed solely by `job_id` (not by client/freelancer) so that a second `submit_rating` call for the same job fails at `init` time — this **is** the duplicate-rating guard, not a separate check (§14.6) |
| **Badge PDA** | `[BADGE_SEED, profile.authority.as_ref(), &[badge_type.as_seed()]]` | Scoped to both the profile and the badge type, so `award_badge` for a badge type the profile already holds fails at `init` time — this **is** the duplicate-badge guard (§14.8) |

`PROFILE_SEED`, `RATING_SEED`, `BADGE_SEED` are `#[constant]` byte strings (`programs/reputation/src/constants.rs`), embedded in the IDL for client-side derivation.

### 14.3 Account Relationships

```
UserProfile (1, seeded by authority)
      │
      ├──< Badge (0..7, seeded by profile.authority + badge_type — at most one per type)
      │
      └── updated by ──< Rating (N, seeded by job_id — each rating folds
                            its score into exactly one freelancer's profile)
```

A `Rating` references its `freelancer`'s `UserProfile` by account, not by embedding — `submit_rating` takes the `freelancer_profile` account directly and mutates it in the same instruction that creates the `Rating`.

### 14.4 Deterministic Addressing & Ownership Model

Because every PDA is derived from public inputs (an authority pubkey, a job ID, a badge type enum), any client, indexer, or auditor can compute a profile/rating/badge's address without querying the chain first, and can verify that an account claiming to be "Alice's profile" is in fact at the one address that could ever hold that role. There is no admin-settable "which account is the real profile" mapping to trust — the seed derivation *is* the mapping.

### 14.5 Why No Private Keys Exist — and Why That Matters Here

A PDA is deliberately computed off the Ed25519 curve, so no keypair exists that could sign for it. Combined with Anchor's `seeds`/`bump` account constraints (re-derived and matched on every instruction, exactly as in Escrow §8.3), this means:

- **No one can fabricate a `UserProfile`, `Rating`, or `Badge` at an address other than its one canonical derivation** — an attacker cannot pass an account they control and claim it is "the freelancer's profile" for a given authority; the derived address would not match and Anchor's constraint check fails the transaction (§14.6, verified by `tests/pda_security.rs`, 12 tests).
- **Program signing** is not exercised for outbound transfers here (no funds are held), but the same `invoke_signed` mechanism that protects Escrow's vault is available for any future instruction that needs the program to act as one of these PDAs.
- **Bump seeds**: every PDA stores its own `bump`, captured at `init` via `ctx.bumps.<account>`; every later instruction constrains reuse with `bump = stored_bump`, so a caller cannot supply an alternate bump to derive a different, attacker-influenced address that happens to collide.

### 14.6 Why Each Account Has Its Own PDA

Splitting `UserProfile`, `Rating`, and `Badge` into independently-addressed PDAs (rather than, say, an array of ratings inside the profile) gives:

- **Fixed account size** for `UserProfile` — it never grows regardless of how many ratings or badges a user accumulates, so there's no realloc/resize logic and no per-profile cap on rating or badge count.
- **Parallel, conflict-free writes** — two different freelancers' profiles, or two different jobs' ratings, never contend for the same account.
- **The seed itself enforces the core invariants** (one rating per job, one badge per type per profile) instead of requiring a manual "does this already exist" scan.

## 15. Instruction Flow

```
initialize_profile (authority signs)
      │
      ▼
UserProfile { all counters = 0 }
      │
      ├──► submit_rating (client signs + escrow_authority PDA co-signs via CPI, job_id + score 1-5 + review_hash)
      │        └──► Rating PDA created (job_id-keyed, immutable)
      │        └──► freelancer_profile.rating_sum/rating_count/average_rating/
      │              reputation_score updated in the same instruction
      │
      ├──► update_completion (escrow_authority PDA signs via CPI, successful + earnings)
      │        └──► completed_jobs += 1; successful_jobs or cancelled_jobs += 1;
      │              total_earnings += earnings (only if successful);
      │              reputation_score recomputed
      │
      ├──► award_badge (permissionless; anyone pays rent)
      │        └──► is_eligible_for_badge(profile, badge_type) checked
      │        └──► Badge PDA created (profile + badge_type-keyed, one per type)
      │
      └──► get_profile (read-only, no signer) ──► returns reputation_score
```

There is no ordering dependency between `submit_rating` and `update_completion` — a client can rate a job before or after the authority records its completion; both simply fold their respective inputs into the profile and recompute `reputation_score` from the resulting totals.

## 16. State Transitions

Unlike Escrow's `Gig`/`Milestone`, `UserProfile` has no discrete status enum — its "state" is the accumulated counters, and every mutating instruction moves it strictly forward:

| Field | Only ever | Enforced by |
|---|---|---|
| `completed_jobs`, `successful_jobs`, `cancelled_jobs` | increases | `checked_add`, no decrement instruction exists |
| `total_earnings` | increases | `checked_add`, only on `successful = true` |
| `rating_sum`, `rating_count` | increase | `checked_add`, one increment per `Rating` PDA ever created |
| `average_rating` | recomputed from `rating_sum`/`rating_count` | exact recomputation each time, not incremental blending (`utils::average_rating`) — avoids any drift from repeated rounding |
| `reputation_score` | recomputed from current totals | `utils::compute_reputation_score`, pure function of the profile's stored fields, called at the end of `submit_rating`, `update_completion`, and (implicitly, via `badges_earned`) `award_badge` |
| `created_at` | set once, at `init` | never written again |
| `updated_at` | monotonically non-decreasing | set to `Clock::get()?.unix_timestamp` on every mutating instruction |

`reputation_score` is a deterministic function of `(completed_jobs, successful_jobs, total_earnings, average_rating, cancelled_jobs)` alone — see §21.5. It is never set directly by an instruction argument, so there is no code path that lets a caller assign an arbitrary score.

## 17. Event Architecture

| Event | Emitted by | Notes |
|---|---|---|
| `ProfileCreated` | `initialize_profile` | |
| `RatingSubmitted` | `submit_rating` | includes `new_average_rating` so indexers don't need to re-derive it |
| `CompletionUpdated` | `update_completion` | includes the recomputed `reputation_score` |
| `BadgeAwarded` | `award_badge` | |
| `ProfileUpdated` | *(defined, not currently emitted by any instruction)* | Reserved for a future generic "profile mutated" event; today `RatingSubmitted`/`CompletionUpdated`/`BadgeAwarded` each already carry the fields an indexer needs, so nothing currently emits it. Documented here rather than silently left as dead code. |

Event coverage is verified in `tests/events.rs` (10 tests) — every emitting instruction's event fields are asserted against the instruction's actual effects.

## 18. Escrow → Reputation CPI

Reputation updates only after Escrow confirms a payment has actually settled — never on gig creation, funding, delivery, or cancellation. Escrow is the source of truth for *successful payment*; Reputation is the source of truth for *trust*. Concretely:

- `update_completion` and `submit_rating` no longer accept a hardcoded authority pubkey. They require a `Signer` at seeds `[b"escrow_authority"]` derived under `ESCROW_PROGRAM_ID` (`seeds::program = ESCROW_PROGRAM_ID`, mirroring the same pattern Escrow already uses to call Gig — see §5.4). A PDA has no private key, so the only way to produce a valid signature for it is `invoke_signed` from inside the Escrow program itself. A direct top-level call, or a call "signed" by any real keypair, fails Anchor's seeds constraint before the handler ever runs.
- Escrow exposes two new instructions that perform this CPI:
  - **`settle_reputation`** (permissionless) — callable once `vault.active_milestone >= vault.milestone_count` (every milestone released). It CPIs `update_completion(successful: true, earnings: vault.total_released)` and sets `vault.reputation_synced = true`, a new `EscrowVault` field that makes the CPI (and its earnings credit) fire at most once per gig. This is decoupled from `approve_milestone`/`full_timeout_release` deliberately: bundling it into those instructions would force every escrow settlement — including gigs whose freelancer never initialized a reputation profile — to carry reputation accounts, breaking the programs' independent-deployment guarantee. `settle_reputation` is an additive step a freelancer or indexer calls after settlement, gated purely by Escrow's own vault state.
  - **`rate_freelancer`** (client-signed) — callable once `gig.status == Completed`. It CPIs `submit_rating(job_id: gig.id, score, review_hash)` with `escrow_authority` as the trusted attester that `gig.id` really is a completed job between this client and freelancer. Reputation still enforces the one-rating-per-job (PDA-keyed by `job_id`) and valid-range rules itself.
- `award_badge` is permissionless (no signer requirement beyond the rent payer): eligibility is recomputed from the profile's own public, already-verified fields (`is_eligible_for_badge`), so there is no privileged data to gate — anyone (a freelancer, a client, an indexer) can trigger it once eligible, and the badge-type-keyed PDA prevents duplicates regardless of caller.

This mirrors the same trust pattern already used for Escrow → Gig: a hardcoded `ESCROW_PROGRAM_ID` constant in `programs/reputation/src/constants.rs` (no crate dependency on `escrow`, avoiding a circular build dependency) plus a fixed `ESCROW_AUTHORITY_SEED`. See SECURITY.md for the full threat model and attack-by-attack verification.

## 19. Design Rationale Summary

| Decision | Why |
|---|---|
| `Rating` PDA seeded by `job_id` alone (not `job_id + client`) | The seed itself is the duplicate-submission guard — a second `submit_rating` for the same job fails at `init`, with no separate "already rated" existence check needed |
| `average_rating` recomputed from `rating_sum`/`rating_count` every time, not blended incrementally | Guarantees the stored average is always the exact mean of every rating ever submitted, regardless of submission order — no floating-point or rounding drift accumulates |
| `reputation_score` is a pure function of stored counters, recomputed on every mutation | Anyone can independently recompute and verify a profile's score from its public fields alone; there is no code path where a score is set directly, so it cannot silently diverge from the data that justifies it |
| Badge eligibility is deterministic where possible (`FirstGig`, `*CompletedJobs`, `FiveStarPerformer`, `TopRated`); `TrustedFreelancer`/`FastDeliverer` are always ineligible for now | Some signals (delivery timing, external endorsements) aren't tracked on-chain by this program. Since `award_badge` is permissionless, eligibility can never depend on anything the caller merely asserts — so these two badge types return `false` until a real on-chain signal backs them, rather than being awardable on a bare claim |
| `update_completion`/`submit_rating` require a `seeds::program`-pinned `escrow_authority` PDA signer, not a hardcoded pubkey | The only account that can ever sign for that PDA is Escrow's own `invoke_signed` CPI — closing the gap where a single leaked/rotated keypair could forge job completions or ratings (§18, SECURITY.md) |
| `settle_reputation` is its own escrow instruction, not folded into `approve_milestone`/`full_timeout_release` | Keeps reputation genuinely optional per-gig instead of a hard dependency of every settlement path, preserving independent deployability; gated by `vault.reputation_synced` so it can still only ever fire once |
| Reputation is a separate program from Escrow | See §3 — isolates the audited payment path from reputation-scoring logic that will change more often |
| NFT minting lives in its own `achievement` program, triggered by the user, never by escrow settlement | Metaplex Core CPI accounts (collection, asset, mpl-core program) have no reason to appear in the money path; a bug or upgrade in NFT-minting logic can never block or slow down a milestone release |
| Achievement reuses `reputation::BadgeType` instead of declaring its own badge enum | The task badges *are* an eligibility calculation, which is Reputation's job; a second enum plus a second eligibility check would be duplicated state that could drift from Reputation's own rules |
| `claim_achievement` proves eligibility by re-deriving Reputation's own `Badge` PDA (`seeds::program = reputation::ID`), not by re-running eligibility logic | The PDA's mere existence already encodes "eligibility was checked once, by the program that owns that logic" — re-checking it in Achievement would be the duplicated-logic anti-pattern this split is meant to avoid |

## 20. Achievement Program

**Path:** `programs/achievement` (Anchor, `declare_id!("GV8Z39NBK7qrojXCfnnwLTXpqsLoCW6sy9cLHGYjtrv9")`).

Achievement is a fourth independently-deployable program, added after Gig/Escrow/Reputation were already stable. It sits downstream of Reputation only:

```
Escrow → Reputation (award_badge) → Achievement (claim_achievement) → Metaplex Core
```

It never appears in the Escrow → Gig or Escrow → Reputation CPI chain, and it makes no CPI into either Gig or Escrow — it only reads Reputation's `UserProfile` and `Badge` accounts directly (by re-deriving their PDAs under `reputation::ID`), and it makes exactly one outbound CPI, to the Metaplex Core program, to actually mint.

**Why claim-based instead of auto-minted on badge award.** `award_badge` is permissionless and cheap by design (§ Reputation Program, README.md) — anyone can trigger it once a profile's own public fields clear the threshold. Auto-minting an NFT at that same moment would tie Metaplex Core's CPI surface, rent cost, and failure modes to an instruction Reputation needs to stay lightweight and callable by anyone, including indexers. Splitting the mint into a separate, user-initiated `claim_achievement` call means: a badge being earned and an NFT existing are two independently-failable events, the user (not an indexer or a bot calling `award_badge`) pays the NFT's rent, and Reputation's hot path never touches an external program it doesn't control the deployment of.

**Why Reputation stays independent of Metaplex.** Reputation's account layout, CPI surface, and audit boundary were fixed before Achievement existed. Adding a Metaplex Core dependency to Reputation would mean every future Reputation change also has to reason about Metaplex Core's plugin/collection model — the same "isolate the audited path from logic that changes more often" argument as §3, applied one level further down the chain. Achievement absorbs that dependency instead, and if Metaplex Core's CPI interface changes, only Achievement needs to be redeployed.

See README.md's [Achievement Program](./README.md#achievement-program) section for the instruction/state reference and SECURITY.md's [Achievement Program Security Model](./SECURITY.md#achievement-program-security-model) for the threat model.

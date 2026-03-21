# linkd-protocol-soroban

Rust / Soroban SDK 20.3.2 — milestone-locked dual-signature escrow smart contract on Stellar.

> [!IMPORTANT]
> **Non-Custodial Regulatory Disclaimer**: This software is provided as a set of non-custodial protocol tools and interfaces. It does not provide financial services, investment advice, or asset management. All transaction signing and private key management are handled locally by the user. This implementation is designed to align with the **Kenyan VASP Act 2025** standards for non-custodial decentralized protocols.

## What This Is

The immutable state machine and settlement layer for the Linkdfund accountability rail. Funds are locked in escrow and released only when two independent parties (NGO + Auditor) cryptographically approve a milestone with a verified KRA eTIMS proof hash.

## Contract: `linkd_escrow`

```
contracts/linkd_escrow/src/lib.rs    Core contract logic
contracts/linkd_escrow/src/test.rs   Contract tests
```

## Architecture

```
Roles:     Admin, NGO, Auditor, Beneficiary (set once at initialize())
Token:     SEP-41 compatible (XLM or USDC on Stellar)
Storage:   Instance storage (roles, totals) + Persistent storage (milestones, 30-day TTL)
```

## On-Chain State Machine

```
Milestone lifecycle:
  Pending → AwaitingProof → UnderReview → Verified
                                        → Rejected

Release:
  approve_ngo(milestone_id)      → sets ngo_approved: true
  approve_auditor(milestone_id)  → sets auditor_approved: true
                                    if BOTH true → transfer target_amount to Beneficiary
```

Do not add intermediate states without understanding the dual-signature invariant.

## Contract Entry Points

| Function | Auth Required | Description |
|----------|--------------|-------------|
| `initialize(admin, ngo, auditor, beneficiary, token)` | — (one-time) | Set roles and token. Idempotent guard — reverts if already initialized. |
| `add_milestone(milestone_id, target_amount, description)` | Admin | Append a new funding tranche in `Pending` state to persistent storage. |
| `deposit_funds(donor, amount)` | Donor (any) | Lock SEP-41 tokens into escrow. Increments `total_escrowed`. |
| `submit_proof(milestone_id, proof_hash)` | NGO | Attach KRA eTIMS / IPFS hash. Transitions milestone to `AwaitingProof`. Rejects empty strings. |
| `approve_ngo(milestone_id)` | NGO | Set `ngo_approved: true`. If auditor already approved, releases funds to Beneficiary. |
| `approve_auditor(milestone_id)` | Auditor | Set `auditor_approved: true`. If NGO already approved, releases funds to Beneficiary. |
| `refund_milestone(milestone_id, refund_address)` | Admin | Cancel milestone, return `target_amount` to `refund_address`. Administrative exception path. |
| `get_milestone_count()` | — (read) | Return total number of milestones stored. |
| `get_total_escrowed()` | — (read) | Return total SEP-41 token amount currently locked. |
| `get_escrow_status()` | — (read) | Return full contract state: roles, totals, all milestone summaries. |

## Test Coverage

| Test | Scenario |
|------|----------|
| `test_initialize` | Roles set correctly; second call reverts |
| `test_add_milestone` | Milestone stored with correct amount and status |
| `test_deposit_funds` | Balance increases; total_escrowed updated |
| `test_submit_proof` | Proof hash stored; empty string rejected |
| `test_dual_approval_release` | Funds reach Beneficiary only after both approvals |
| `test_ngo_only_no_release` | No release if only NGO approved |
| `test_auditor_only_no_release` | No release if only Auditor approved |
| `test_refund_milestone` | Admin can cancel; funds routed to refund_address |
| `test_unauthorized_refund` | Non-admin refund call reverts |

## Soroban-Specific Rules

- `soroban-sdk = "20.3.2"` — pinned. Do not bump without reading the Stellar migration guide.
- Storage TTL: Instance and Persistent entries are extended 30 days on every write. Removing this logic causes permanent state archival on Stellar.
- `#[contracterror]` enums must be sequential integers starting at 1. Gaps break ABI serialization.
- Tests live in `test.rs`, not integration tests — Soroban has its own test environment.
- WASM must be built with `--target wasm32-unknown-unknown --release`. Never use debug WASM.

## Testnet Deployment

| Role | Address |
|------|---------|
| Contract ID | `CA5O24QV7UXTE4OFHULDAF5QWQOW6MJMN6NSMSMYUFCVLFBEUNMFESMT` |
| Admin | `GDNBJ2L4ADLHT2QPSVGUE44VOVDP6Y4NR6RNSFXOP4WHAKII4D36LPZ7` |
| NGO | `GDJF3OW2CVALMUG4EACMJEQLHHP23N6FYXQVCWVAHNUHEHO2CZMNKRUN` |
| Auditor | `GCVARRTZXCICMT2KDXVYDVE3Q3GNPDKWHJQDJIFYEGHOWA3PSPQ4263I` |
| Beneficiary | `GBLTSK6RUMU2OMETRIST6D3PJDHWJE2SROH3SQKQ2GTBFT6AMZA3CG5I` |

**Mainnet deployment requires: clean third-party audit report. Do not deploy to mainnet unaudited.**

## Security

- `Admin` role controls `refund_milestone()` and `add_milestone()`. Any change to role assignment is a critical security change.
- Token amounts use `i128`. Verify no overflow paths exist before adding new arithmetic.
- State updates strictly precede token transfers — eliminates re-entrancy attack vectors.
- `submit_proof()` rejects empty proof hashes at the entry point.

## Commands

```bash
cargo test                                                          # Run all tests (must be clean)
cargo build --target wasm32-unknown-unknown --release               # Build WASM
cargo clippy --all-targets --all-features -- -D warnings           # Lint (must be clean)
cargo fmt --check                                                   # Format check
```

All four must pass before any contract change is considered complete.

## What the SDK Layer Handles

`linkd-ts-sdk` constructs XDR and simulates operations. The contract does not validate XDR format, handle off-chain state, or know about M-Pesa, KRA, or Stellar Horizon. The contract's only job: enforce the dual-signature release invariant and hold tokens in custody.

## License

Proprietary. All rights reserved.

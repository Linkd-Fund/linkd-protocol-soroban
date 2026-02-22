# Linkdfund Accountability Protocol: Soroban Escrow

A non-custodial, dual-signature escrow primitive built on Stellar's Soroban smart contract platform. This contract serves as the immutable state machine and settlement layer for milestone-based capital disbursement using SEP-41 compatible assets.

> [!IMPORTANT]
> **Non-Custodial Regulatory Disclaimer**: This software is provided as a set of non-custodial protocol tools and interfaces. It does not provide financial services, investment advice, or asset management. All transaction signing and private key management are handled locally by the user. This implementation is designed to align with the **Kenyan VASP Act 2025** standards for non-custodial decentralized protocols.

## Core Mechanics

* **Non-Custodial Design:** The protocol natively locks and distributes assets on-chain. The deploying entity does not retain custody of funds, strictly separating infrastructure from asset management.
* **Dual-Signature Consensus:** Capital is tranche-locked. Disbursement requires cryptographic authorization (`require_auth`) from two independent actors (e.g., executing entity and verifying auditor) per milestone.
* **Storage Scalability:** Milestones are mapped to `persistent()` storage with independent composite keys, bypassing Soroban's 64KB `instance()` memory limits for unbounded operational scaling.
* **State Rent Management:** Automated Time-To-Live (TTL) extensions are integrated at the function level to prevent contract archival and state expiration on the Stellar ledger.
* **Exception Handling:** Includes administrative primitives to cancel stalled milestones and route capital to designated refund addresses, preventing permanent liquidity traps.

## Development

### Prerequisites
* Rust toolchain
* `wasm32-unknown-unknown` target
* Soroban CLI

### Build
Compile the contract to WebAssembly:
```bash
cargo build --target wasm32-unknown-unknown --release
```

The compiled binary will be located at `target/wasm32-unknown-unknown/release/linkd_escrow.wasm`.

### Test

Run the native test suite:

```bash
cargo test
```

## Security

This contract utilizes native Soroban authorization primitives. It avoids redundant parameter passing in favor of protocol-level signature verification. State updates strictly precede external token transfers to eliminate re-entrancy attack vectors.

## License

Proprietary. All rights reserved.
# Contributing to StellarSwipe-Contract

## Scaffold a new contract crate

Use the scaffold generator to create a new contract crate already wired with the
shared **Pausable**, **Initializable**, and **StorageTrait** conventions:

```bash
# From the repository root
./stellar-swipe/scripts/scaffold_contract.sh <contract_name>
```

### Example

```bash
./stellar-swipe/scripts/scaffold_contract.sh reward_distributor
```

This creates:

```
stellar-swipe/contracts/reward_distributor/
├── Cargo.toml          # depends on soroban-sdk + stellar-swipe-common
└── src/
    ├── lib.rs          # initialize / pause / unpause / storage_write / storage_read
    └── tests.rs        # starter tests covering init, pause, storage round-trip
```

The workspace `Cargo.toml` is updated automatically to include the new crate.

### Verify the scaffold

```bash
cd stellar-swipe
cargo test   -p stellar-swipe-reward-distributor
cargo clippy -p stellar-swipe-reward-distributor -- -D warnings
```

Both should pass with no manual fixes required.

### What the scaffold includes

| Feature | Implementation |
|---|---|
| Initializable guard | `initialize()` panics/returns error on double-init |
| Pausable | `pause()` / `unpause()` / `is_paused()` with events |
| StorageTrait pattern | `storage_write(key, value)` / `storage_read(key)` blocked while paused |
| Starter test file | `tests.rs` with 5 tests covering all three features |

Extend `DataKey` and `{ContractName}Error` with your contract-specific variants
before adding business logic.

## Structured panic messages for intentional panics

When a contract panics *intentionally* (a programming/configuration error
the caller can't meaningfully recover from in the same call — as opposed to
an expected, recoverable failure, which should return a `ContractError`/
`AdminError`/etc. `Result`, and as opposed to an `unwrap()`/`expect()` on a
condition that should truly never happen), use
`stellar_swipe_common::structured_panic!` instead of a bare `panic!("...")`:

```rust
use stellar_swipe_common::structured_panic;

structured_panic!(9100, "entry price cannot be zero");
structured_panic!(9100, "invalid amount: {}", amount);
```

This produces a message of the form `SSW-<code>: <context>` — a fixed
prefix, a stable numeric code, and a short context string — which is
visually and grep-ably distinct from a generic Rust panic (e.g. a bare
`unwrap()`), making on-chain log / post-mortem triage easier.

Codes are allocated in fixed ranges per contract so a code alone identifies
the source contract even without access to the source:

| Contract         | Code range  |
|------------------|-------------|
| oracle           | 9000–9099   |
| signal_registry  | 9100–9199   |
| stake_vault      | 9200–9299   |
| fee_collector    | 9300–9399   |
| user_portfolio   | 9400–9499   |
| auto_trade       | 9500–9599   |
| trade_executor   | 9600–9699   |
| governance       | 9700–9799   |
| bridge           | 9800–9899   |
| common / shared  | 9900–9999   |

Pick the next unused code in your contract's range; there's no central
registry beyond this table, so check existing `structured_panic!` call
sites in your contract before assigning a new one.

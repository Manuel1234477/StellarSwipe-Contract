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

## Clippy policy

CI runs `cargo clippy --workspace --all-targets -- -D warnings`.  Any clippy
warning fails the build.

### Suppressing a lint

- **Prefer fixing** the underlying issue over suppressing.
- Suppressions must be **as narrow as possible**: annotate the individual item
  (`fn`, `impl` block, expression) rather than the whole module or crate.
- The `#[allow]` attribute must include a brief comment explaining why the
  suppression is justified:

  ```rust
  // Soroban contract functions cannot use struct wrappers in the public ABI.
  #[allow(clippy::too_many_arguments)]
  pub fn my_contract_fn(env: Env, a: Address, …) { … }
  ```

- Workspace-wide suppressions live in `[workspace.lints.clippy]` in
  `stellar-swipe/Cargo.toml` and require a PR that explains why the lint is
  non-actionable across the whole workspace.

### Requesting an exception

Open a PR with the `lint-exception` label.  The PR description must include:

1. The lint name and the code it fires on.
2. Why fixing the code is not preferable.
3. A `#[allow]` annotation scoped to the narrowest applicable span.

Reviewers will merge only after confirming the suppression scope is minimal.

//! Commit-reveal helpers to **bind** a user’s trade parameters before execution.
//!
//! These functions do **not** by themselves stop ordering attacks inside a Stellar
//! validator’s mempool; they give integrators a canonical `SHA-256` over intent fields
//! so a future on-chain or off-chain “commit” phase can reference the same bytes.
//! See `docs/security/front_running_analysis.md`.

use soroban_sdk::{Address, Bytes, BytesN, Env, String};

/// `SHA-256( "sw_exec_v1" || user || signal_id || amount || min_out || salt
/// || valid_until_ledger )` as a [`BytesN<32>`].
///
/// - `min_out` — user-defined floor for received amount (slippage / MEV margin).
/// - `valid_until_ledger` — user expects execution by this ledger (inclusive);
///   contracts that adopt commit-reveal should reject reveals after this ledger.
/// - `salt` — high-entropy; clients should use a CSPRNG (or expand to 32 bytes in
///   a future version of this API).
pub fn hash_trade_intent(
    env: &Env,
    user: &Address,
    signal_id: u64,
    amount: i128,
    min_out: i128,
    salt: u64,
    valid_until_ledger: u32,
) -> BytesN<32> {
    let mut preimage = Bytes::new(env);
    preimage.append(&String::from_str(env, "sw_exec_v1").to_bytes());
    preimage.append(&user.to_string().to_bytes());
    preimage.append(&Bytes::from_array(env, &signal_id.to_be_bytes()));
    preimage.append(&Bytes::from_array(env, &amount.to_be_bytes()));
    preimage.append(&Bytes::from_array(env, &min_out.to_be_bytes()));
    preimage.append(&Bytes::from_array(env, &salt.to_be_bytes()));
    preimage.append(&Bytes::from_array(env, &valid_until_ledger.to_be_bytes()));
    env.crypto().sha256(&preimage).into()
}

/// Constant-time equality check for fixed-length (32-byte) hash/commitment values.
///
/// Rationale: comparing commitment hashes with standard `==` short-circuits on the
/// first mismatched byte, which can in principle leak timing information about how
/// much of a secret commitment matches an attacker's guess. That's a defense-in-depth
/// concern for commit-reveal schemes guarding economically meaningful actions (e.g.
/// [`hash_trade_intent`] reveals). This walks every byte unconditionally via XOR-accumulate
/// and only branches once, on the final accumulated result, so the comparison takes the
/// same number of steps regardless of where (or whether) the inputs first differ.
///
/// Future contributors: prefer this (or [`verify_commitment`]) over `==`/`!=` whenever
/// comparing a stored commitment hash against a freshly computed one.
pub fn constant_time_eq(a: &BytesN<32>, b: &BytesN<32>) -> bool {
    let a = a.to_array();
    let b = b.to_array();
    let mut diff: u8 = 0;
    for i in 0..32 {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

/// Verify a revealed commitment against the `expected` stored hash.
///
/// Use this instead of `expected == actual` when checking a commit-reveal hash —
/// see [`constant_time_eq`] for why.
pub fn verify_commitment(expected: &BytesN<32>, actual: &BytesN<32>) -> bool {
    constant_time_eq(expected, actual)
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn hash_is_deterministic() {
        let env = Env::default();
        let a = Address::generate(&env);
        let h1 = hash_trade_intent(&env, &a, 5, 1_000_000, 900_000, 42, 1_000_000);
        let h2 = hash_trade_intent(&env, &a, 5, 1_000_000, 900_000, 42, 1_000_000);
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_changes_when_amount_changes() {
        let env = Env::default();
        let a = Address::generate(&env);
        let h1 = hash_trade_intent(&env, &a, 5, 1_000_000, 900_000, 42, 1_000_000);
        let h2 = hash_trade_intent(&env, &a, 5, 1_000_001, 900_000, 42, 1_000_000);
        assert_ne!(h1, h2);
    }

    // ── constant_time_eq / verify_commitment (Issue #594) ─────────────────────

    #[test]
    fn constant_time_eq_identical_arrays_match() {
        let env = Env::default();
        let a: BytesN<32> = BytesN::from_array(&env, &[7u8; 32]);
        let b: BytesN<32> = BytesN::from_array(&env, &[7u8; 32]);
        assert!(constant_time_eq(&a, &b));
    }

    #[test]
    fn constant_time_eq_differs_in_first_byte() {
        let env = Env::default();
        let mut bytes_a = [0u8; 32];
        let mut bytes_b = [0u8; 32];
        bytes_a[0] = 1;
        bytes_b[0] = 2;
        let a = BytesN::from_array(&env, &bytes_a);
        let b = BytesN::from_array(&env, &bytes_b);
        assert!(!constant_time_eq(&a, &b));
    }

    #[test]
    fn constant_time_eq_differs_in_last_byte() {
        let env = Env::default();
        let mut bytes_a = [9u8; 32];
        let mut bytes_b = [9u8; 32];
        bytes_a[31] = 1;
        bytes_b[31] = 2;
        let a = BytesN::from_array(&env, &bytes_a);
        let b = BytesN::from_array(&env, &bytes_b);
        assert!(!constant_time_eq(&a, &b));
    }

    #[test]
    fn constant_time_eq_differs_in_middle_byte() {
        let env = Env::default();
        let mut bytes_a = [3u8; 32];
        let mut bytes_b = [3u8; 32];
        bytes_a[16] = 0xAA;
        bytes_b[16] = 0xBB;
        let a = BytesN::from_array(&env, &bytes_a);
        let b = BytesN::from_array(&env, &bytes_b);
        assert!(!constant_time_eq(&a, &b));
    }

    #[test]
    fn verify_commitment_matches_valid_reveal() {
        let env = Env::default();
        let a = Address::generate(&env);
        let committed = hash_trade_intent(&env, &a, 5, 1_000_000, 900_000, 42, 1_000_000);
        let revealed = hash_trade_intent(&env, &a, 5, 1_000_000, 900_000, 42, 1_000_000);
        assert!(verify_commitment(&committed, &revealed));
    }

    #[test]
    fn verify_commitment_rejects_invalid_reveal() {
        let env = Env::default();
        let a = Address::generate(&env);
        let committed = hash_trade_intent(&env, &a, 5, 1_000_000, 900_000, 42, 1_000_000);
        // Attacker reveals different trade parameters than what was committed to.
        let revealed = hash_trade_intent(&env, &a, 5, 1_000_001, 900_000, 42, 1_000_000);
        assert!(!verify_commitment(&committed, &revealed));
    }
}

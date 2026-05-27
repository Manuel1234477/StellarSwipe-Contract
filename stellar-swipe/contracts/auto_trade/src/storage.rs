#![allow(dead_code)]
use soroban_sdk::{contracttype, Address, Env};

#[contracttype]
#[derive(Clone)]
pub struct Signal {
    pub signal_id: u64,
    pub price: i128,
    pub expiry: u64,
    pub base_asset: u32,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RateLimitInfo {
    pub user: Address,
    pub is_limited: bool,
    pub expires_at: u64,
}

#[contracttype]
pub enum DataKey {
    Trades(Address, u64),
    Signal(u64),
    RateLimitInfo(Address),
}

/// Get a signal by ID
pub fn get_signal(env: &Env, id: u64) -> Option<Signal> {
    env.storage().persistent().get(&DataKey::Signal(id))
}

/// Set a signal
pub fn set_signal(env: &Env, id: u64, signal: &Signal) {
    env.storage().persistent().set(&DataKey::Signal(id), signal);
}

/// Get rate limit info for a user
pub fn get_rate_limit_info(env: &Env, user: &Address) -> Option<RateLimitInfo> {
    env.storage().persistent().get(&DataKey::RateLimitInfo(user.clone()))
}

/// Set rate limit info for a user
pub fn set_rate_limit_info(env: &Env, user: &Address, info: &RateLimitInfo) {
    env.storage()
        .persistent()
        .set(&DataKey::RateLimitInfo(user.clone()), info);
}

/// Check if user is rate limited (considering expiry)
pub fn is_rate_limited(env: &Env, user: &Address) -> bool {
    if let Some(info) = get_rate_limit_info(env, user) {
        if !info.is_limited {
            return false;
        }

        let now = env.ledger().timestamp();
        
        // If expired, clear the flag
        if now >= info.expires_at {
            let expired_info = RateLimitInfo {
                user: user.clone(),
                is_limited: false,
                expires_at: 0,
            };
            set_rate_limit_info(env, user, &expired_info);
            return false;
        }

        true
    } else {
        false
    }
}

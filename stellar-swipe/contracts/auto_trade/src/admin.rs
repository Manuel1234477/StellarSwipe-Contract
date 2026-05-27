use soroban_sdk::{contracttype, Address, Env, Symbol};

use crate::errors::AutoTradeError;
use crate::storage::{self, RateLimitInfo};

// Constants
/// Rate limit duration: 720 ledgers ≈ 1 hour (assuming 5-second block time)
pub const RATE_LIMIT_DURATION_LEDGERS: u64 = 720;

/// 1 hour in seconds (3600 seconds)
pub const RATE_LIMIT_DURATION_SECONDS: u64 = 3600;

#[contracttype]
#[derive(Clone)]
pub enum AdminStorageKey {
    Admin,
    Operator,
}

/// Initialize admin (called once at contract deployment)
pub fn init_admin(env: &Env, admin: Address) -> Result<(), AutoTradeError> {
    if has_admin(env) {
        return Err(AutoTradeError::Unauthorized);
    }

    env.storage().instance().set(&AdminStorageKey::Admin, &admin);
    Ok(())
}

/// Check if admin is initialized
pub fn has_admin(env: &Env) -> bool {
    env.storage().instance().has(&AdminStorageKey::Admin)
}

/// Get current admin
pub fn get_admin(env: &Env) -> Result<Address, AutoTradeError> {
    env.storage()
        .instance()
        .get(&AdminStorageKey::Admin)
        .ok_or(AutoTradeError::Unauthorized)
}

/// Require caller is admin
pub fn require_admin(env: &Env, caller: &Address) -> Result<(), AutoTradeError> {
    let admin = get_admin(env)?;
    if caller != &admin {
        return Err(AutoTradeError::Unauthorized);
    }
    caller.require_auth();
    Ok(())
}

/// Get current operator
pub fn get_operator(env: &Env) -> Result<Address, AutoTradeError> {
    env.storage()
        .instance()
        .get(&AdminStorageKey::Operator)
        .ok_or(AutoTradeError::Unauthorized)
}

/// Set operator (admin only)
pub fn set_operator(env: &Env, caller: &Address, operator: Address) -> Result<(), AutoTradeError> {
    require_admin(env, caller)?;

    env.storage()
        .instance()
        .set(&AdminStorageKey::Operator, &operator);

    #[allow(deprecated)]
    env.events().publish(
        (Symbol::new(env, "operator_set"), caller.clone()),
        operator.clone(),
    );

    Ok(())
}

/// Require caller is operator
pub fn require_operator(env: &Env, caller: &Address) -> Result<(), AutoTradeError> {
    let operator = get_operator(env)?;
    if caller != &operator {
        return Err(AutoTradeError::Unauthorized);
    }
    caller.require_auth();
    Ok(())
}

/// Set rate limit flag for a user (operator only)
/// Sets is_limited=true and expires_at = now + RATE_LIMIT_DURATION_SECONDS
pub fn set_rate_limited(
    env: &Env,
    caller: &Address,
    user: &Address,
) -> Result<(), AutoTradeError> {
    require_operator(env, caller)?;

    let now = env.ledger().timestamp();
    let expires_at = now + RATE_LIMIT_DURATION_SECONDS;

    let info = RateLimitInfo {
        user: user.clone(),
        is_limited: true,
        expires_at,
    };

    storage::set_rate_limit_info(env, user, &info);

    #[allow(deprecated)]
    env.events().publish(
        (Symbol::new(env, "user_rate_limited"), user.clone()),
        expires_at,
    );

    Ok(())
}

/// Clear rate limit flag for a user (operator only)
pub fn clear_rate_limited(
    env: &Env,
    caller: &Address,
    user: &Address,
) -> Result<(), AutoTradeError> {
    require_operator(env, caller)?;

    let info = RateLimitInfo {
        user: user.clone(),
        is_limited: false,
        expires_at: 0,
    };

    storage::set_rate_limit_info(env, user, &info);

    #[allow(deprecated)]
    env.events().publish(
        (Symbol::new(env, "user_rate_limit_cleared"), user.clone()),
        (),
    );

    Ok(())
}

/// Get rate limit info for a user
pub fn get_rate_limit_info(
    env: &Env,
    user: &Address,
) -> Option<RateLimitInfo> {
    storage::get_rate_limit_info(env, user)
}

/// Check if user is rate limited (and auto-expire if necessary)
pub fn is_rate_limited(env: &Env, user: &Address) -> bool {
    storage::is_rate_limited(env, user)
}

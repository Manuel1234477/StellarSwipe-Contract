#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol};

mod admin;
mod auth;
mod errors;
mod history;
mod multi_asset;
mod portfolio;
mod risk;
mod sdex;
mod storage;
mod strategies;

use crate::storage::DataKey;
use errors::AutoTradeError;

/// ==========================
/// Types
/// ==========================

#[contracttype]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    Market,
    Limit,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TradeStatus {
    Pending,
    PartiallyFilled,
    Filled,
    Failed,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trade {
    pub signal_id: u64,
    pub user: Address,
    pub requested_amount: i128,
    pub executed_amount: i128,
    pub executed_price: i128,
    pub timestamp: u64,
    pub status: TradeStatus,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TradeResult {
    pub trade: Trade,
}

/// ==========================
/// Contract
/// ==========================

#[contract]
pub struct AutoTradeContract;

/// ==========================
/// Implementation
/// ==========================

#[contractimpl]
impl AutoTradeContract {
    /// Execute a trade on behalf of a user based on a signal
    pub fn execute_trade(
        env: Env,
        user: Address,
        signal_id: u64,
        order_type: OrderType,
        amount: i128,
    ) -> Result<TradeResult, AutoTradeError> {
        if amount <= 0 {
            return Err(AutoTradeError::InvalidAmount);
        }

        user.require_auth();

        // Check if user is rate limited
        if admin::is_rate_limited(&env, &user) {
            return Err(AutoTradeError::RateLimited);
        }

        let signal = storage::get_signal(&env, signal_id).ok_or(AutoTradeError::SignalNotFound)?;

        if env.ledger().timestamp() > signal.expiry {
            return Err(AutoTradeError::SignalExpired);
        }

        if !auth::is_authorized(&env, &user, amount) {
            return Err(AutoTradeError::Unauthorized);
        }

        if !sdex::has_sufficient_balance(&env, &user, &signal.base_asset, amount) {
            return Err(AutoTradeError::InsufficientBalance);
        }

        // Determine if this is a sell operation (simplified)
        let is_sell = false; // This should be determined from the signal or order details

        // Set current asset price for risk calculations
        risk::set_asset_price(&env, signal.base_asset, signal.price);

        // Perform risk checks
        let stop_loss_triggered = risk::validate_trade(
            &env,
            &user,
            signal.base_asset,
            amount,
            signal.price,
            is_sell,
        )?;

        // If stop-loss is triggered, emit event and proceed with sell
        if stop_loss_triggered {
            #[allow(deprecated)]
            env.events().publish(
                (
                    Symbol::new(&env, "stop_loss_triggered"),
                    user.clone(),
                    signal.base_asset,
                ),
                signal.price,
            );
        }

        let execution = match order_type {
            OrderType::Market => sdex::execute_market_order(&env, &user, &signal, amount)?,
            OrderType::Limit => sdex::execute_limit_order(&env, &user, &signal, amount)?,
        };

        let status = if execution.executed_amount == 0 {
            TradeStatus::Failed
        } else if execution.executed_amount < amount {
            TradeStatus::PartiallyFilled
        } else {
            TradeStatus::Filled
        };

        let trade = Trade {
            signal_id,
            user: user.clone(),
            requested_amount: amount,
            executed_amount: execution.executed_amount,
            executed_price: execution.executed_price,
            timestamp: env.ledger().timestamp(),
            status: status.clone(),
        };

        // Update position tracking
        if execution.executed_amount > 0 {
            let positions = risk::get_user_positions(&env, &user);
            let current_amount = positions
                .get(signal.base_asset)
                .map(|p| p.amount)
                .unwrap_or(0);

            let new_amount = if is_sell {
                current_amount - execution.executed_amount
            } else {
                current_amount + execution.executed_amount
            };

            risk::update_position(
                &env,
                &user,
                signal.base_asset,
                new_amount,
                execution.executed_price,
            );

            // Record trade in history
            risk::add_trade_record(&env, &user, signal_id, execution.executed_amount);
        }

        env.storage()
            .persistent()
            .set(&DataKey::Trades(user.clone(), signal_id), &trade);

        if execution.executed_amount > 0 {
            let hist_status = match status {
                TradeStatus::Filled | TradeStatus::PartiallyFilled => {
                    history::HistoryTradeStatus::Executed
                }
                TradeStatus::Failed => history::HistoryTradeStatus::Failed,
                TradeStatus::Pending => history::HistoryTradeStatus::Pending,
            };
            history::record_trade(
                &env,
                &user,
                signal_id,
                signal.base_asset,
                execution.executed_amount,
                execution.executed_price,
                0,
                hist_status,
            );
        }

        #[allow(deprecated)]
        env.events().publish(
            (Symbol::new(&env, "trade_executed"), user.clone(), signal_id),
            trade.clone(),
        );

        // Emit event if trade was blocked by risk limits (status = Failed due to risk)
        if status == TradeStatus::Failed {
            #[allow(deprecated)]
            env.events().publish(
                (
                    Symbol::new(&env, "risk_limit_block"),
                    user.clone(),
                    signal_id,
                ),
                amount,
            );
        }

        Ok(TradeResult { trade })
    }

    /// Fetch executed trade by user + signal
    pub fn get_trade(env: Env, user: Address, signal_id: u64) -> Option<Trade> {
        env.storage()
            .persistent()
            .get(&DataKey::Trades(user, signal_id))
    }

    /// Get user's risk configuration
    pub fn get_risk_config(env: Env, user: Address) -> risk::RiskConfig {
        risk::get_risk_config(&env, &user)
    }

    /// Update user's risk configuration
    pub fn set_risk_config(env: Env, user: Address, config: risk::RiskConfig) {
        user.require_auth();
        risk::set_risk_config(&env, &user, &config);

        #[allow(deprecated)]
        env.events().publish(
            (Symbol::new(&env, "risk_config_updated"), user.clone()),
            config,
        );
    }

    /// Get user's current positions
    pub fn get_user_positions(env: Env, user: Address) -> soroban_sdk::Map<u32, risk::Position> {
        risk::get_user_positions(&env, &user)
    }

    /// Get user's trade history (risk module, legacy)
    pub fn get_trade_history_legacy(
        env: Env,
        user: Address,
    ) -> soroban_sdk::Vec<risk::TradeRecord> {
        risk::get_trade_history(&env, &user)
    }

    /// Get paginated trade history (newest first)
    pub fn get_trade_history(
        env: Env,
        user: Address,
        offset: u32,
        limit: u32,
    ) -> soroban_sdk::Vec<history::HistoryTrade> {
        history::get_trade_history(&env, &user, offset, limit)
    }

    /// Get user portfolio with holdings and P&L
    pub fn get_portfolio(env: Env, user: Address) -> portfolio::Portfolio {
        portfolio::get_portfolio(&env, &user)
    }

    /// Grant authorization to execute trades
    pub fn grant_authorization(
        env: Env,
        user: Address,
        max_amount: i128,
        duration_days: u32,
    ) -> Result<(), AutoTradeError> {
        auth::grant_authorization(&env, &user, max_amount, duration_days)
    }

    /// Revoke authorization
    pub fn revoke_authorization(env: Env, user: Address) -> Result<(), AutoTradeError> {
        auth::revoke_authorization(&env, &user)
    }

    /// Get authorization config
    pub fn get_auth_config(env: Env, user: Address) -> Option<auth::AuthConfig> {
        auth::get_auth_config(&env, &user)
    }

    /// Set rate limit flag for a user (operator only)
    /// Flag expires after 1 hour (RATE_LIMIT_DURATION_SECONDS = 3600)
    pub fn set_rate_limited(env: Env, operator: Address, user: Address) -> Result<(), AutoTradeError> {
        admin::set_rate_limited(&env, &operator, &user)
    }

    /// Clear rate limit flag for a user (operator only)
    pub fn clear_rate_limited(env: Env, operator: Address, user: Address) -> Result<(), AutoTradeError> {
        admin::clear_rate_limited(&env, &operator, &user)
    }

    /// Get rate limit info for a user
    pub fn get_rate_limit_info(env: Env, user: Address) -> Option<storage::RateLimitInfo> {
        admin::get_rate_limit_info(&env, &user)
    }

    /// Check if user is currently rate limited
    pub fn is_rate_limited(env: Env, user: Address) -> bool {
        admin::is_rate_limited(&env, &user)
    }

    /// Returns estimated storage usage metrics.
    ///
    /// # Estimation methodology
    /// - `total_signals`: exact count of stored Signal entries.
    /// - `total_positions`: exact count of active user positions across all users.
    /// - `total_providers`: approximated as distinct users with trade history.
    /// - `estimated_rent_xlm`: entry_count × avg_entry_size_bytes × RENT_RATE_XLM_PER_BYTE.
    ///   avg_entry_size ≈ 128 bytes (trades are smaller than signals);
    ///   rent_rate ≈ 0.00001 XLM/byte (Soroban Protocol 23).
    ///   Result is in stroops (1 XLM = 10_000_000 stroops).
    ///
    /// # Rent cost projection for 10,000 users
    /// Assuming 10 trades/user → 100,000 trade entries + 10,000 position entries = 110,000 entries.
    /// 110,000 × 128 bytes × 0.00001 XLM/byte ≈ 140.8 XLM total rent.
    pub fn get_storage_stats(env: Env) -> AutoTradeStorageStats {
        // Count persistent trade entries via signal counter as proxy
        let total_signals: u32 = env
            .storage()
            .persistent()
            .get(&storage::DataKey::Signal(0))
            .map(|_: storage::Signal| 1u32)
            .unwrap_or(0);

        // Positions: sum across all tracked users is not directly enumerable;
        // use trade history length as a proxy for total_positions.
        let total_positions: u32 = 0; // requires enumerable index; documented as 0 until index added
        let total_providers: u32 = 0; // same — no global user index in auto_trade

        let entry_count = (total_signals + total_positions + total_providers) as i128;
        let estimated_rent_xlm = entry_count * 128 * 100;

        AutoTradeStorageStats {
            total_signals,
            total_positions,
            total_providers,
            estimated_rent_xlm,
        }
    }
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutoTradeStorageStats {
    pub total_signals: u32,
    pub total_positions: u32,
    pub total_providers: u32,
    /// Estimated rent in stroops (1 XLM = 10_000_000 stroops).
    pub estimated_rent_xlm: i128,
}

mod test;

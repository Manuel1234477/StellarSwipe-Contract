use shared::errors::{ErrorCategory, RecoveryStrategy};
use shared::initializable;
use soroban_sdk::{contracttype, Address, Env, String, Vec};
use stellar_swipe_common::Asset;

// ── #690: Fee Distribution Waterfall ────────────────────────────────────────

/// A single tier in the fee distribution waterfall, processed in ascending
/// priority order (lower `priority` value = funded first).
#[contracttype]
#[derive(Clone, Debug)]
pub struct WaterfallTier {
    /// Human-readable label (e.g. "treasury", "insurance", "rewards").
    pub name: String,
    /// Lower value = higher priority. Tiers are processed in ascending order.
    pub priority: u32,
    /// Full allocation for this tier; lower-priority tiers receive leftovers.
    pub target_amount: i128,
    /// Minimum the tier must receive to be funded at all. If remaining funds
    /// fall below this, the tier receives nothing.
    pub minimum_amount: i128,
    /// Address that receives the allocation for this tier.
    pub recipient: Address,
}

/// Admin-configurable ordered waterfall of fee destinations.
#[contracttype]
#[derive(Clone, Debug)]
pub struct WaterfallConfig {
    pub tiers: Vec<WaterfallTier>,
}

/// Per-tier allocation record emitted in the waterfall_distribution event.
#[contracttype]
#[derive(Clone, Debug)]
pub struct WaterfallTierResult {
    pub name: String,
    pub recipient: Address,
    pub priority: u32,
    pub allocated: i128,
}

// ── #691: Provider Settlement Currency ──────────────────────────────────────

/// Stored preference for a provider's claim payout currency.
/// `preferred_token` is the SEP-41 token contract to receive fees in.
#[contracttype]
#[derive(Clone, Debug)]
pub struct PayoutCurrency {
    pub preferred_token: Address,
}

pub const MAX_FEE_RATE_BPS: u32 = 100; // 1%
pub const MIN_FEE_RATE_BPS: u32 = 1; // 0.01%
pub const DEFAULT_FEE_RATE_BPS: u32 = 30; // 0.3%
pub const DEFAULT_BURN_RATE_BPS: u32 = 1_000; // 10%
pub const MAX_BURN_RATE_BPS: u32 = 10_000; // 100%
pub const DEFAULT_NETWORK_SCORE_BPS: u32 = 0;
pub const DEFAULT_FEE_OPTIMIZATION_MAX_RATE_BPS: u32 = 100;
pub const DEFAULT_CONGESTION_SENSITIVITY_BPS: u32 = 50;
pub const DEFAULT_MAX_RETRY_ATTEMPTS: u32 = 3;
pub const LEDGERS_PER_MONTH_APPROX: u32 = 518_400; // ~30 days at ~5 seconds per ledger
pub const SILVER_TIER_VOLUME_USD: i128 = 10_000 * 10_000_000; // $10k, 7 decimals
pub const GOLD_TIER_VOLUME_USD: i128 = 50_000 * 10_000_000; // $50k, 7 decimals
pub const SILVER_DISCOUNT_BPS: u32 = 5;
pub const GOLD_DISCOUNT_BPS: u32 = 10;

#[contracttype]
pub enum StorageKey {
    Admin,
    Initialized,
    OracleContract,
    TreasuryBalance(Address),              // persistent, per-token
    QueuedWithdrawal,                      // instance, single-slot
    FeeRate,                               // instance, current fee rate in bps
    BurnRate,                              // instance, burn rate in bps
    ProviderPendingFees(Address, Address), // persistent, per (provider, token)
    MonthlyTradeVolume(Address),           // persistent, per user
    /// Accumulated fee shares per provider per day (day = unix_timestamp / SECONDS_PER_DAY).
    ProviderDailyFeeShares(Address, u64),
    /// Day number of the provider's first recorded earnings (for ALL_TIME period_start).
    ProviderEarningsFirstDay(Address),
    /// Total accumulated fee shares for a provider, used to rank earnings leaders.
    ProviderTotalEarnings(Address),
    /// Providers that have recorded earnings, for leaderboard scans.
    ProviderEarningsIndex,
    /// Whether a user has completed their first trade (Issue #428).
    HasTraded(Address),
    // ── Issue #438: Protocol Token Integration ─────────────────────
    /// Optional protocol token address for token-based fee payment.
    ProtocolToken,
    /// Revenue share rate in basis points (default: 2000 = 20%).
    RevenueShareRateBps,
    /// Last snapshot ledger for revenue sharing (Issue #442).
    LastRevenueShareSnapshot,
    /// Accumulated revenue share pool waiting for next distribution.
    RevenueSharePool(Address),
    /// Latest aggregated network score for fee optimization.
    NetworkConditionScore,
    /// Configurable dynamic fee optimization parameters.
    FeeOptimizationConfig,
    /// Last recorded contract error report.
    LastErrorReport,
    /// Persisted failed fee collection operation for retry.
    FailedFeeCollection(String),
    /// #690: Admin-configured waterfall distribution tiers.
    WaterfallConfig,
    /// #691: Per-provider preferred payout token.
    ProviderPayoutCurrency(Address),
    /// #664: Admin-configured volume-based discount tiers.
    VolumeDiscountConfig,
    /// #665: Fee forecast configuration.
    ForecastConfig,
    /// #665: Per-token daily fee total (token, day).
    DailyFeeTotal(Address, u64),
    /// #665: Last day a forecast was emitted per token.
    LastForecastDay(Address),
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct FeeOptimizationConfig {
    pub max_dynamic_rate_bps: u32,
    pub congestion_sensitivity_bps: u32,
    pub min_effective_rate_bps: u32,
    pub max_retry_attempts: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct ErrorReport {
    pub category: ErrorCategory,
    pub strategy: RecoveryStrategy,
    pub message: String,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct FailedFeeCollection {
    pub id: String,
    pub trader: Address,
    pub token: Address,
    pub trade_amount: i128,
    pub trade_asset: Asset,
    pub retry_count: u32,
    pub last_error: String,
}

#[contracttype]
#[derive(Clone)]
pub struct QueuedWithdrawal {
    pub recipient: Address,
    pub token: Address,
    pub amount: i128,
    pub queued_at: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct MonthlyTradeVolume {
    pub month_bucket: u32,
    pub volume_usd: i128,
}

/// Describes a discrepancy between the contract's stored treasury balance
/// and the actual on-chain token balance for a given token.
#[contracttype]
#[derive(Clone, Debug)]
pub struct BalanceMismatch {
    /// The token whose balances were compared.
    pub token: Address,
    /// Balance recorded in contract storage (`TreasuryBalance`).
    pub expected: i128,
    /// Actual token balance held by this contract on-chain.
    pub actual: i128,
    /// Difference: `actual - expected`. Positive means surplus, negative means deficit.
    pub delta: i128,
}

// --- Admin ---

pub fn get_admin(env: &Env) -> Address {
    env.storage().instance().get(&StorageKey::Admin).unwrap()
}

pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&StorageKey::Admin, admin);
}

// --- Initialized (migrated to shared::initializable, issue #584) ---

pub fn is_initialized(env: &Env) -> bool {
    initializable::is_initialized(env)
}

pub fn set_initialized(env: &Env) {
    initializable::mark_initialized(env);
}

// --- Oracle Contract ---

pub fn get_oracle_contract(env: &Env) -> Option<Address> {
    env.storage().instance().get(&StorageKey::OracleContract)
}

pub fn set_oracle_contract(env: &Env, contract: &Address) {
    env.storage()
        .instance()
        .set(&StorageKey::OracleContract, contract);
}

// --- Treasury Balance ---

pub fn get_treasury_balance(env: &Env, token: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&StorageKey::TreasuryBalance(token.clone()))
        .unwrap_or(0i128)
}

pub fn set_treasury_balance(env: &Env, token: &Address, balance: i128) {
    env.storage()
        .persistent()
        .set(&StorageKey::TreasuryBalance(token.clone()), &balance);
}

// --- Queued Withdrawal ---

pub fn get_queued_withdrawal(env: &Env) -> Option<QueuedWithdrawal> {
    env.storage().instance().get(&StorageKey::QueuedWithdrawal)
}

pub fn set_queued_withdrawal(env: &Env, withdrawal: &QueuedWithdrawal) {
    env.storage()
        .instance()
        .set(&StorageKey::QueuedWithdrawal, withdrawal);
}

pub fn remove_queued_withdrawal(env: &Env) {
    env.storage()
        .instance()
        .remove(&StorageKey::QueuedWithdrawal);
}

// --- Fee Rate ---

pub fn get_fee_rate(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&StorageKey::FeeRate)
        .unwrap_or(DEFAULT_FEE_RATE_BPS)
}

pub fn set_fee_rate(env: &Env, rate: u32) {
    env.storage().instance().set(&StorageKey::FeeRate, &rate);
}

// --- Fee Optimization ---

pub fn get_network_condition_score(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&StorageKey::NetworkConditionScore)
        .unwrap_or(DEFAULT_NETWORK_SCORE_BPS)
}

pub fn set_network_condition_score(env: &Env, score: u32) {
    env.storage()
        .instance()
        .set(&StorageKey::NetworkConditionScore, &score);
}

pub fn get_fee_optimization_config(env: &Env) -> FeeOptimizationConfig {
    env.storage()
        .instance()
        .get(&StorageKey::FeeOptimizationConfig)
        .unwrap_or(FeeOptimizationConfig {
            max_dynamic_rate_bps: DEFAULT_FEE_OPTIMIZATION_MAX_RATE_BPS,
            congestion_sensitivity_bps: DEFAULT_CONGESTION_SENSITIVITY_BPS,
            min_effective_rate_bps: MIN_FEE_RATE_BPS,
            max_retry_attempts: DEFAULT_MAX_RETRY_ATTEMPTS,
        })
}

pub fn set_fee_optimization_config(env: &Env, config: &FeeOptimizationConfig) {
    env.storage()
        .instance()
        .set(&StorageKey::FeeOptimizationConfig, config);
}

pub fn get_last_error_report(env: &Env) -> Option<ErrorReport> {
    env.storage().instance().get(&StorageKey::LastErrorReport)
}

pub fn set_last_error_report(env: &Env, report: &ErrorReport) {
    env.storage()
        .instance()
        .set(&StorageKey::LastErrorReport, report);
}

pub fn get_failed_fee_collection(env: &Env, id: &String) -> Option<FailedFeeCollection> {
    env.storage()
        .persistent()
        .get(&StorageKey::FailedFeeCollection(id.clone()))
}

pub fn set_failed_fee_collection(env: &Env, failed: &FailedFeeCollection) {
    env.storage()
        .persistent()
        .set(&StorageKey::FailedFeeCollection(failed.id.clone()), failed);
}

pub fn remove_failed_fee_collection(env: &Env, id: &String) {
    env.storage()
        .persistent()
        .remove(&StorageKey::FailedFeeCollection(id.clone()));
}

// --- Burn Rate ---

pub fn get_burn_rate(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&StorageKey::BurnRate)
        .unwrap_or(DEFAULT_BURN_RATE_BPS)
}

pub fn set_burn_rate(env: &Env, rate: u32) {
    env.storage().instance().set(&StorageKey::BurnRate, &rate);
}

// --- Provider Pending Fees ---

pub fn get_pending_fees(env: &Env, provider: &Address, token: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&StorageKey::ProviderPendingFees(
            provider.clone(),
            token.clone(),
        ))
        .unwrap_or(0i128)
}

pub fn set_pending_fees(env: &Env, provider: &Address, token: &Address, amount: i128) {
    env.storage().persistent().set(
        &StorageKey::ProviderPendingFees(provider.clone(), token.clone()),
        &amount,
    );
}

// --- Monthly Trade Volume ---

pub fn get_monthly_trade_volume(env: &Env, user: &Address) -> Option<MonthlyTradeVolume> {
    env.storage()
        .persistent()
        .get(&StorageKey::MonthlyTradeVolume(user.clone()))
}

pub fn set_monthly_trade_volume(env: &Env, user: &Address, volume: &MonthlyTradeVolume) {
    env.storage()
        .persistent()
        .set(&StorageKey::MonthlyTradeVolume(user.clone()), volume);
}

pub fn remove_monthly_trade_volume(env: &Env, user: &Address) {
    env.storage()
        .persistent()
        .remove(&StorageKey::MonthlyTradeVolume(user.clone()));
}

// --- Provider Daily Fee Shares (Issue #366) ---

pub fn get_provider_daily_fee_shares(env: &Env, provider: &Address, day: u64) -> i128 {
    env.storage()
        .persistent()
        .get(&StorageKey::ProviderDailyFeeShares(provider.clone(), day))
        .unwrap_or(0i128)
}

pub fn get_provider_total_earnings(env: &Env, provider: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&StorageKey::ProviderTotalEarnings(provider.clone()))
        .unwrap_or(0i128)
}

pub fn get_provider_earnings_index(env: &Env) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&StorageKey::ProviderEarningsIndex)
        .unwrap_or_else(|| Vec::new(env))
}

pub fn add_provider_to_earnings_index(env: &Env, provider: &Address) {
    let mut index = get_provider_earnings_index(env);
    for i in 0..index.len() {
        if index.get(i).unwrap() == *provider {
            return;
        }
    }
    index.push_back(provider.clone());
    env.storage()
        .persistent()
        .set(&StorageKey::ProviderEarningsIndex, &index);
}

pub fn add_provider_total_earnings(env: &Env, provider: &Address, amount: i128) {
    let key = StorageKey::ProviderTotalEarnings(provider.clone());
    let current: i128 = env.storage().persistent().get(&key).unwrap_or(0i128);
    let updated = current.saturating_add(amount);
    env.storage().persistent().set(&key, &updated);
    add_provider_to_earnings_index(env, provider);
}

pub fn add_provider_daily_fee_shares(env: &Env, provider: &Address, day: u64, amount: i128) {
    let key = StorageKey::ProviderDailyFeeShares(provider.clone(), day);
    let current: i128 = env.storage().persistent().get(&key).unwrap_or(0i128);
    let updated = current.saturating_add(amount);
    env.storage().persistent().set(&key, &updated);

    // Record first earnings day if not yet set
    let first_key = StorageKey::ProviderEarningsFirstDay(provider.clone());
    if !env.storage().persistent().has(&first_key) {
        env.storage().persistent().set(&first_key, &day);
    }
    add_provider_total_earnings(env, provider, amount);
}

pub fn get_provider_earnings_first_day(env: &Env, provider: &Address) -> Option<u64> {
    env.storage()
        .persistent()
        .get(&StorageKey::ProviderEarningsFirstDay(provider.clone()))
}

// --- First-trade tracking (Issue #428) ---

pub fn has_traded(env: &Env, user: &Address) -> bool {
    env.storage()
        .persistent()
        .get(&StorageKey::HasTraded(user.clone()))
        .unwrap_or(false)
}

pub fn set_has_traded(env: &Env, user: &Address) {
    env.storage()
        .persistent()
        .set(&StorageKey::HasTraded(user.clone()), &true);
}

// ── Issue #438: Protocol Token ──────────────────────────────────────

pub fn get_protocol_token(env: &Env) -> Option<Address> {
    env.storage().instance().get(&StorageKey::ProtocolToken)
}

pub fn set_protocol_token(env: &Env, token: &Address) {
    env.storage()
        .instance()
        .set(&StorageKey::ProtocolToken, token);
}

// ── Issue #442: Revenue Share ────────────────────────────────────────

pub const DEFAULT_REVENUE_SHARE_RATE_BPS: u32 = 2000; // 20%
pub const SECONDS_PER_WEEK: u64 = 604_800;

pub fn get_revenue_share_rate_bps(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&StorageKey::RevenueShareRateBps)
        .unwrap_or(DEFAULT_REVENUE_SHARE_RATE_BPS)
}

pub fn set_revenue_share_rate_bps(env: &Env, rate_bps: u32) {
    env.storage()
        .instance()
        .set(&StorageKey::RevenueShareRateBps, &rate_bps);
}

pub fn get_last_revenue_share_snapshot(env: &Env) -> Option<u64> {
    env.storage()
        .instance()
        .get(&StorageKey::LastRevenueShareSnapshot)
}

pub fn set_last_revenue_share_snapshot(env: &Env, ledger: u64) {
    env.storage()
        .instance()
        .set(&StorageKey::LastRevenueShareSnapshot, &ledger);
}

pub fn get_revenue_share_pool(env: &Env, token: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&StorageKey::RevenueSharePool(token.clone()))
        .unwrap_or(0)
}

pub fn add_revenue_share_pool(env: &Env, token: &Address, amount: i128) {
    let current: i128 = env
        .storage()
        .persistent()
        .get(&StorageKey::RevenueSharePool(token.clone()))
        .unwrap_or(0);
    env.storage().persistent().set(
        &StorageKey::RevenueSharePool(token.clone()),
        &current.saturating_add(amount),
    );
}

pub fn clear_revenue_share_pool(env: &Env, token: &Address) {
    env.storage()
        .persistent()
        .remove(&StorageKey::RevenueSharePool(token.clone()));
}

// ── #690: Waterfall Config ───────────────────────────────────────────────────

pub fn get_waterfall_config(env: &Env) -> Option<WaterfallConfig> {
    env.storage()
        .instance()
        .get(&StorageKey::WaterfallConfig)
}

pub fn set_waterfall_config(env: &Env, config: &WaterfallConfig) {
    env.storage()
        .instance()
        .set(&StorageKey::WaterfallConfig, config);
}

// ── #691: Provider Payout Currency ──────────────────────────────────────────

pub fn get_provider_payout_currency(env: &Env, provider: &Address) -> Option<Address> {
    env.storage()
        .persistent()
        .get::<_, PayoutCurrency>(&StorageKey::ProviderPayoutCurrency(provider.clone()))
        .map(|p| p.preferred_token)
}

pub fn set_provider_payout_currency(env: &Env, provider: &Address, preferred_token: &Address) {
    env.storage().persistent().set(
        &StorageKey::ProviderPayoutCurrency(provider.clone()),
        &PayoutCurrency {
            preferred_token: preferred_token.clone(),
        },
    );
}

pub fn remove_provider_payout_currency(env: &Env, provider: &Address) {
    env.storage()
        .persistent()
        .remove(&StorageKey::ProviderPayoutCurrency(provider.clone()));
}

// ── #664: Volume Discount Tiers ──────────────────────────────────────────────

/// A single volume-based discount tier.
/// Users who meet or exceed `volume_threshold_usd` receive a `discount_bps`
/// reduction on the base fee rate.
#[contracttype]
#[derive(Clone, Debug)]
pub struct VolumeTier {
    pub volume_threshold_usd: i128,
    pub discount_bps: u32,
}

/// Admin-configurable set of volume discount tiers.
/// Must contain at least 3 tiers, sorted ascending by `volume_threshold_usd`.
#[contracttype]
#[derive(Clone, Debug)]
pub struct VolumeDiscountConfig {
    pub tiers: Vec<VolumeTier>,
}

pub fn get_volume_discount_config(env: &Env) -> Option<VolumeDiscountConfig> {
    env.storage()
        .instance()
        .get(&StorageKey::VolumeDiscountConfig)
}

pub fn set_volume_discount_config_storage(env: &Env, config: &VolumeDiscountConfig) {
    env.storage()
        .instance()
        .set(&StorageKey::VolumeDiscountConfig, config);
}

// ── #665: Fee Forecast ───────────────────────────────────────────────────────

/// Admin-configurable forecast parameters.
#[contracttype]
#[derive(Clone, Debug)]
pub struct ForecastConfigData {
    /// How often a forecast event is auto-emitted (in days).
    pub epoch_cadence_days: u64,
    /// Number of historical days used to compute the trailing average.
    pub window_days: u64,
}

pub const DEFAULT_EPOCH_CADENCE_DAYS: u64 = 1;
pub const DEFAULT_FORECAST_WINDOW_DAYS: u64 = 7;
pub const SECONDS_PER_DAY_FC: u64 = 86_400;

pub fn get_forecast_config(env: &Env) -> ForecastConfigData {
    env.storage()
        .instance()
        .get(&StorageKey::ForecastConfig)
        .unwrap_or(ForecastConfigData {
            epoch_cadence_days: DEFAULT_EPOCH_CADENCE_DAYS,
            window_days: DEFAULT_FORECAST_WINDOW_DAYS,
        })
}

pub fn set_forecast_config_storage(env: &Env, config: &ForecastConfigData) {
    env.storage()
        .instance()
        .set(&StorageKey::ForecastConfig, config);
}

pub fn get_daily_fee_total(env: &Env, token: &Address, day: u64) -> i128 {
    env.storage()
        .persistent()
        .get(&StorageKey::DailyFeeTotal(token.clone(), day))
        .unwrap_or(0)
}

pub fn add_daily_fee_total(env: &Env, token: &Address, day: u64, amount: i128) {
    let current = get_daily_fee_total(env, token, day);
    env.storage().persistent().set(
        &StorageKey::DailyFeeTotal(token.clone(), day),
        &current.saturating_add(amount),
    );
}

pub fn get_last_forecast_day(env: &Env, token: &Address) -> u64 {
    env.storage()
        .persistent()
        .get(&StorageKey::LastForecastDay(token.clone()))
        .unwrap_or(0)
}

pub fn set_last_forecast_day(env: &Env, token: &Address, day: u64) {
    env.storage()
        .persistent()
        .set(&StorageKey::LastForecastDay(token.clone()), &day);
}

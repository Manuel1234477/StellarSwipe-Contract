#![no_std]

pub mod assets;
pub mod cross_contract;

pub use assets::{validate_asset_pair, Asset, AssetPair, AssetPairError};
pub use cross_contract::{
    CrossContractError, OracleCallee, OracleClient, StakeVaultCallee, StakeVaultClient,
    VersionedContract, VersionedContractClient, ORACLE_INTERFACE_VERSION,
    STAKE_VAULT_INTERFACE_VERSION,
};

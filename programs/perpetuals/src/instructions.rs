// admin instructions
pub mod add_custody;
pub mod add_pool;
pub mod add_vest;
pub mod init;
pub mod remove_custody;
pub mod remove_pool;
pub mod set_admin_signers;
pub mod set_custody_config;
pub mod set_custom_oracle_price;
pub mod set_permissions;
pub mod upgrade_custody;
pub mod withdraw_fees;
pub mod withdraw_sol_fees;

// test instructions
pub mod set_test_time;
pub mod test_admin_remove_collateral;

// public instructions
pub mod add_collateral;
pub mod add_genesis_liquidity;
pub mod add_liquid_stake;
pub mod add_liquidity;
pub mod add_locked_stake;
pub mod claim_stakes;
pub mod claim_vest;
pub mod close_position;
pub mod finalize_locked_stake;
pub mod get_add_liquidity_amount_and_fee;
pub mod get_assets_under_management;
pub mod get_entry_price_and_fee;
pub mod get_exit_price_and_fee;
pub mod get_liquidation_price;
pub mod get_liquidation_state;
pub mod get_lp_token_price;
pub mod get_oracle_price;
pub mod get_pnl;
pub mod get_remove_liquidity_amount_and_fee;
pub mod get_swap_amount_and_fees;
pub mod increase_position;
pub mod init_staking;
pub mod init_user_staking;
pub mod liquidate;
pub mod mint_lm_tokens_from_bucket;
pub mod open_position;
pub mod open_position_with_swap;
pub mod remove_collateral;
pub mod remove_liquid_stake;
pub mod remove_liquidity;
pub mod remove_locked_stake;
pub mod resolve_staking_round;
pub mod set_custom_oracle_price_permissionless;
pub mod swap;
pub mod update_pool_aum;

// bring everything in scope
pub use {
    add_collateral::*, add_custody::*, add_genesis_liquidity::*, add_liquid_stake::*,
    add_liquidity::*, add_locked_stake::*, add_pool::*, add_vest::*, claim_stakes::*,
    claim_vest::*, close_position::*, finalize_locked_stake::*,
    get_add_liquidity_amount_and_fee::*, get_assets_under_management::*,
    get_entry_price_and_fee::*, get_exit_price_and_fee::*, get_liquidation_price::*,
    get_liquidation_state::*, get_lp_token_price::*, get_oracle_price::*, get_pnl::*,
    get_remove_liquidity_amount_and_fee::*, get_swap_amount_and_fees::*, increase_position::*, init::*, init_staking::*,
    init_user_staking::*, liquidate::*, mint_lm_tokens_from_bucket::*, open_position::*,
    open_position_with_swap::*, remove_collateral::*, remove_custody::*, remove_liquid_stake::*,
    remove_liquidity::*, remove_locked_stake::*, remove_pool::*, resolve_staking_round::*,
    set_admin_signers::*, set_custody_config::*, set_custom_oracle_price::*,
    set_custom_oracle_price_permissionless::*, set_permissions::*, set_test_time::*, swap::*,
    test_admin_remove_collateral::*, update_pool_aum::*, upgrade_custody::*, withdraw_fees::*,
    withdraw_sol_fees::*,
};

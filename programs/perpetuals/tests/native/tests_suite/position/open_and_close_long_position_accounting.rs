use {
    crate::{test_instructions, utils},
    maplit::hashmap,
    perpetuals::{
        instructions::{ClosePositionParams, OpenPositionParams, SetCustomOraclePriceParams},
        state::{
            cortex::Cortex,
            custody::{Custody, PricingParams},
            position::{Position, Side},
        },
    },
};

const ETH_DECIMALS: u8 = 9;
const USDC_DECIMALS: u8 = 6;

pub async fn open_and_close_long_position_accounting() {
    let test_setup = utils::TestSetup::new(
        vec![
            utils::UserParam {
                name: "alice",
                token_balances: hashmap! {
                    "usdc" => utils::scale(150_000, USDC_DECIMALS),
                    "eth" => utils::scale(100, ETH_DECIMALS),
                },
            },
            utils::UserParam {
                name: "martin",
                token_balances: hashmap! {
                    "usdc" => utils::scale(150_000, USDC_DECIMALS),
                    "eth" => utils::scale(100, ETH_DECIMALS),
                },
            },
        ],
        vec![
            utils::MintParam {
                name: "usdc",
                decimals: USDC_DECIMALS,
            },
            utils::MintParam {
                name: "eth",
                decimals: ETH_DECIMALS,
            },
        ],
        vec!["admin_a", "admin_b", "admin_c"],
        "usdc",
        "usdc",
        6,
        "ADRENA",
        "main_pool",
        vec![
            utils::SetupCustodyWithLiquidityParams {
                setup_custody_params: utils::SetupCustodyParams {
                    mint_name: "usdc",
                    is_stable: true,
                    is_virtual: false,
                    target_ratio: utils::ratio_from_percentage(50.0),
                    min_ratio: utils::ratio_from_percentage(0.0),
                    max_ratio: utils::ratio_from_percentage(100.0),
                    initial_price: utils::scale(1, USDC_DECIMALS),
                    initial_conf: utils::scale_f64(0.01, USDC_DECIMALS),
                    pricing_params: None,
                    permissions: None,
                    fees: None,
                    borrow_rate: None,
                },
                liquidity_amount: utils::scale(150_000, USDC_DECIMALS),
                payer_user_name: "alice",
            },
            utils::SetupCustodyWithLiquidityParams {
                setup_custody_params: utils::SetupCustodyParams {
                    mint_name: "eth",
                    is_stable: false,
                    is_virtual: false,
                    target_ratio: utils::ratio_from_percentage(100.0),
                    min_ratio: utils::ratio_from_percentage(0.0),
                    max_ratio: utils::ratio_from_percentage(100.0),
                    initial_price: utils::scale(1_500, ETH_DECIMALS),
                    initial_conf: utils::scale(10, ETH_DECIMALS),
                    pricing_params: Some(PricingParams {
                        // Expressed in BPS, with BPS = 10_000
                        // 50_000 = x5, 100_000 = x10
                        max_leverage: 100_000,
                        ..utils::fixtures::pricing_params_regular(false)
                    }),
                    permissions: None,
                    fees: None,
                    borrow_rate: None,
                },
                liquidity_amount: utils::scale(100, ETH_DECIMALS),
                payer_user_name: "alice",
            },
        ],
        utils::scale(1_000_000, Cortex::LM_DECIMALS),
        utils::scale(1_000_000, Cortex::LM_DECIMALS),
        utils::scale(1_000_000, Cortex::LM_DECIMALS),
        utils::scale(1_000_000, Cortex::LM_DECIMALS),
    )
    .await;

    let martin = test_setup.get_user_keypair_by_name("martin");

    let admin_a = test_setup.get_multisig_member_keypair_by_name("admin_a");

    let multisig_signers = test_setup.get_multisig_signers();

    let eth_mint = &test_setup.get_mint_by_name("eth");

    let eth_custody_pda = test_setup.custodies_info[1].custody_pda;

    let eth_custody_account_before =
        utils::get_account::<Custody>(&test_setup.program_test_ctx, eth_custody_pda).await;

    // Martin: Open 1 ETH long position x5
    let position_pda = test_instructions::open_position(
        &test_setup.program_test_ctx,
        martin,
        &test_setup.payer_keypair,
        &test_setup.pool_pda,
        eth_mint,
        OpenPositionParams {
            // max price paid (slippage implied)
            price: utils::scale(1_550, ETH_DECIMALS),
            collateral: utils::scale(1, ETH_DECIMALS),
            size: utils::scale(5, ETH_DECIMALS),
            side: Side::Long,
        },
    )
    .await
    .unwrap()
    .0;

    {
        let eth_custody_account_after =
            utils::get_account::<Custody>(&test_setup.program_test_ctx, eth_custody_pda).await;

        // Check the position PDA info
        {
            let position =
                utils::get_account::<Position>(&test_setup.program_test_ctx, position_pda).await;

            assert_eq!(position.side, Side::Long);
            // entry price
            // price of the token + trade_spread_long (in BPS)
            assert_eq!(position.price, 1_515_000_000);
            // locked amount (size) * position price (entry price)
            assert_eq!(position.size_usd, 7_575_000_000);
            // locked amount (size) * position price (entry price)
            assert_eq!(position.borrow_size_usd, 7_575_000_000);
            // 1 ETH at price
            assert_eq!(position.collateral_usd, 1_500_000_000);
            // 1 ETH
            assert_eq!(position.collateral_amount, 1_000_000_000);
            assert_eq!(position.unrealized_profit_usd, 0);
            assert_eq!(position.unrealized_loss_usd, 0);
            assert_eq!(position.cumulative_interest_snapshot, 0);
            // 5 ETH
            assert_eq!(position.locked_amount, 5_000_000_000);
        }

        // Double check effect of opening position on ETH custody accounting
        {
            // Collected fees
            {
                // Change
                {
                    assert_eq!(
                        eth_custody_account_before.collected_fees.open_position_usd + 75_000_000,
                        eth_custody_account_after.collected_fees.open_position_usd
                    );
                }

                // No change
                {
                    assert_eq!(
                        eth_custody_account_before.collected_fees.swap_usd,
                        eth_custody_account_after.collected_fees.swap_usd
                    );

                    assert_eq!(
                        eth_custody_account_before.collected_fees.add_liquidity_usd,
                        eth_custody_account_after.collected_fees.add_liquidity_usd
                    );

                    assert_eq!(
                        eth_custody_account_before
                            .collected_fees
                            .remove_liquidity_usd,
                        eth_custody_account_after
                            .collected_fees
                            .remove_liquidity_usd
                    );

                    assert_eq!(
                        eth_custody_account_before.collected_fees.close_position_usd,
                        eth_custody_account_after.collected_fees.close_position_usd
                    );

                    assert_eq!(
                        eth_custody_account_before.collected_fees.liquidation_usd,
                        eth_custody_account_after.collected_fees.liquidation_usd
                    );
                }
            }

            // Distributed rewards
            {
                // Change
                {
                    assert_eq!(
                        eth_custody_account_before
                            .distributed_rewards
                            .open_position_lm
                            + 50_000,
                        eth_custody_account_after
                            .distributed_rewards
                            .open_position_lm
                    );
                }

                // No change
                {
                    assert_eq!(
                        eth_custody_account_before.distributed_rewards.swap_lm,
                        eth_custody_account_after.distributed_rewards.swap_lm
                    );

                    assert_eq!(
                        eth_custody_account_before
                            .distributed_rewards
                            .add_liquidity_lm,
                        eth_custody_account_after
                            .distributed_rewards
                            .add_liquidity_lm
                    );

                    assert_eq!(
                        eth_custody_account_before
                            .distributed_rewards
                            .remove_liquidity_lm,
                        eth_custody_account_after
                            .distributed_rewards
                            .remove_liquidity_lm
                    );

                    assert_eq!(
                        eth_custody_account_before
                            .distributed_rewards
                            .close_position_lm,
                        eth_custody_account_after
                            .distributed_rewards
                            .close_position_lm
                    );

                    assert_eq!(
                        eth_custody_account_before
                            .distributed_rewards
                            .liquidation_lm,
                        eth_custody_account_after.distributed_rewards.liquidation_lm
                    );
                }
            }

            // Volume stats
            {
                // Change
                {
                    // Swap open position ETH fees for USDC
                    assert_eq!(
                        // lm_staker_fee share of expected_protocol_fee is 14_962_500, multiplied by 1500 (eth price)
                        eth_custody_account_before.volume_stats.swap_usd + 22_443_750,
                        eth_custody_account_after.volume_stats.swap_usd
                    );

                    assert_eq!(
                        // size usd
                        eth_custody_account_before.volume_stats.open_position_usd + 7_575_000_000,
                        eth_custody_account_after.volume_stats.open_position_usd
                    );
                }

                // No change
                {
                    assert_eq!(
                        eth_custody_account_before.volume_stats.add_liquidity_usd,
                        eth_custody_account_after.volume_stats.add_liquidity_usd
                    );

                    assert_eq!(
                        eth_custody_account_before.volume_stats.remove_liquidity_usd,
                        eth_custody_account_after.volume_stats.remove_liquidity_usd
                    );

                    assert_eq!(
                        eth_custody_account_before.volume_stats.close_position_usd,
                        eth_custody_account_after.volume_stats.close_position_usd
                    );

                    assert_eq!(
                        eth_custody_account_before.volume_stats.liquidation_usd,
                        eth_custody_account_after.volume_stats.liquidation_usd
                    );
                }
            }

            // Trade Stats
            {
                // Change
                {
                    assert_eq!(
                        // size usd
                        eth_custody_account_before.trade_stats.oi_long_usd + 7_575_000_000,
                        eth_custody_account_after.trade_stats.oi_long_usd
                    );
                }

                // No change
                {
                    assert_eq!(
                        eth_custody_account_before.trade_stats.profit_usd,
                        eth_custody_account_after.trade_stats.profit_usd
                    );

                    assert_eq!(
                        eth_custody_account_before.trade_stats.loss_usd,
                        eth_custody_account_after.trade_stats.loss_usd
                    );

                    assert_eq!(
                        eth_custody_account_before.trade_stats.oi_short_usd,
                        eth_custody_account_after.trade_stats.oi_short_usd
                    );
                }
            }

            // Long positions
            {
                // Change
                {
                    assert_eq!(
                        eth_custody_account_before.long_positions.open_positions + 1,
                        eth_custody_account_after.long_positions.open_positions
                    );

                    assert_eq!(
                        eth_custody_account_before.long_positions.size_usd + 7_575_000_000,
                        eth_custody_account_after.long_positions.size_usd
                    );

                    assert_eq!(
                        eth_custody_account_before.long_positions.borrow_size_usd + 7_575_000_000,
                        eth_custody_account_after.long_positions.borrow_size_usd
                    );

                    assert_eq!(
                        // 5 ETH
                        eth_custody_account_before.long_positions.locked_amount + 5_000_000_000,
                        eth_custody_account_after.long_positions.locked_amount
                    );

                    assert_eq!(
                        eth_custody_account_before.long_positions.total_quantity + 50_000,
                        eth_custody_account_after.long_positions.total_quantity
                    );

                    // WeightedPrice = position_price * quantity
                    assert_eq!(
                        eth_custody_account_before.long_positions.weighted_price
                            + 75_750_000_000_000,
                        eth_custody_account_after.long_positions.weighted_price
                    );
                }
            }

            // No Change
            {
                // Should probably change, mark the parameter as deprecated
                assert_eq!(
                    eth_custody_account_before.long_positions.collateral_usd,
                    eth_custody_account_after.long_positions.collateral_usd
                );

                assert_eq!(
                    eth_custody_account_before
                        .long_positions
                        .cumulative_interest_usd,
                    eth_custody_account_after
                        .long_positions
                        .cumulative_interest_usd
                );

                assert_eq!(
                    eth_custody_account_before
                        .long_positions
                        .cumulative_interest_snapshot,
                    eth_custody_account_after
                        .long_positions
                        .cumulative_interest_snapshot
                );
            }

            // Short positions
            {
                // No change
                {
                    assert_eq!(
                        eth_custody_account_before.short_positions.open_positions,
                        eth_custody_account_after.short_positions.open_positions
                    );

                    assert_eq!(
                        eth_custody_account_before.short_positions.collateral_usd,
                        eth_custody_account_after.short_positions.collateral_usd
                    );

                    assert_eq!(
                        eth_custody_account_before.short_positions.size_usd,
                        eth_custody_account_after.short_positions.size_usd
                    );

                    assert_eq!(
                        eth_custody_account_before.short_positions.borrow_size_usd,
                        eth_custody_account_after.short_positions.borrow_size_usd
                    );

                    assert_eq!(
                        eth_custody_account_before.short_positions.locked_amount,
                        eth_custody_account_after.short_positions.locked_amount
                    );

                    assert_eq!(
                        eth_custody_account_before.short_positions.weighted_price,
                        eth_custody_account_after.short_positions.weighted_price
                    );

                    assert_eq!(
                        eth_custody_account_before.short_positions.total_quantity,
                        eth_custody_account_after.short_positions.total_quantity
                    );

                    assert_eq!(
                        eth_custody_account_before
                            .short_positions
                            .cumulative_interest_usd,
                        eth_custody_account_after
                            .short_positions
                            .cumulative_interest_usd
                    );

                    assert_eq!(
                        eth_custody_account_before
                            .short_positions
                            .cumulative_interest_snapshot,
                        eth_custody_account_after
                            .short_positions
                            .cumulative_interest_snapshot
                    );
                }
            }
        }
    }

    // Makes ETH price to drop 10%
    {
        let eth_test_oracle_pda = test_setup.custodies_info[1].custom_oracle_pda;
        let eth_custody_pda = test_setup.custodies_info[1].custody_pda;

        let publish_time = utils::get_current_unix_timestamp(&test_setup.program_test_ctx).await;

        test_instructions::set_custom_oracle_price(
            &test_setup.program_test_ctx,
            admin_a,
            &test_setup.payer_keypair,
            &test_setup.pool_pda,
            &eth_custody_pda,
            &eth_test_oracle_pda,
            SetCustomOraclePriceParams {
                price: utils::scale(1_350, ETH_DECIMALS),
                expo: -(ETH_DECIMALS as i32),
                conf: utils::scale(10, ETH_DECIMALS),
                ema: utils::scale(1_350, ETH_DECIMALS),
                publish_time,
            },
            &multisig_signers,
        )
        .await
        .unwrap();
    }

    utils::warp_forward(&test_setup.program_test_ctx, 1).await;

    let eth_custody_account_before =
        utils::get_account::<Custody>(&test_setup.program_test_ctx, eth_custody_pda).await;

    // Martin: Close the ETH position
    test_instructions::close_position(
        &test_setup.program_test_ctx,
        martin,
        &test_setup.payer_keypair,
        &test_setup.pool_pda,
        eth_mint,
        &position_pda,
        ClosePositionParams {
            // lower the price for slippage
            price: utils::scale(1_330, USDC_DECIMALS),
        },
    )
    .await
    .unwrap();

    {
        let eth_custody_account_after =
            utils::get_account::<Custody>(&test_setup.program_test_ctx, eth_custody_pda).await;

        // Double check effect of closing position on ETH custody accounting
        {
            // Collected fees
            {
                // Change
                {
                    assert_eq!(
                        eth_custody_account_before.collected_fees.close_position_usd + 75_750_001,
                        eth_custody_account_after.collected_fees.close_position_usd
                    );
                }

                // No change
                {
                    assert_eq!(
                        eth_custody_account_before.collected_fees.open_position_usd,
                        eth_custody_account_after.collected_fees.open_position_usd
                    );

                    assert_eq!(
                        eth_custody_account_before.collected_fees.swap_usd,
                        eth_custody_account_after.collected_fees.swap_usd
                    );

                    assert_eq!(
                        eth_custody_account_before.collected_fees.add_liquidity_usd,
                        eth_custody_account_after.collected_fees.add_liquidity_usd
                    );

                    assert_eq!(
                        eth_custody_account_before
                            .collected_fees
                            .remove_liquidity_usd,
                        eth_custody_account_after
                            .collected_fees
                            .remove_liquidity_usd
                    );

                    assert_eq!(
                        eth_custody_account_before.collected_fees.liquidation_usd,
                        eth_custody_account_after.collected_fees.liquidation_usd
                    );
                }
            }

            // Distributed rewards
            {
                // Change
                {
                    assert_eq!(
                        eth_custody_account_before
                            .distributed_rewards
                            .close_position_lm
                            + 56_111,
                        eth_custody_account_after
                            .distributed_rewards
                            .close_position_lm
                    );
                }

                // No change
                {
                    assert_eq!(
                        eth_custody_account_before
                            .distributed_rewards
                            .open_position_lm,
                        eth_custody_account_after
                            .distributed_rewards
                            .open_position_lm
                    );

                    assert_eq!(
                        eth_custody_account_before.distributed_rewards.swap_lm,
                        eth_custody_account_after.distributed_rewards.swap_lm
                    );

                    assert_eq!(
                        eth_custody_account_before
                            .distributed_rewards
                            .add_liquidity_lm,
                        eth_custody_account_after
                            .distributed_rewards
                            .add_liquidity_lm
                    );

                    assert_eq!(
                        eth_custody_account_before
                            .distributed_rewards
                            .remove_liquidity_lm,
                        eth_custody_account_after
                            .distributed_rewards
                            .remove_liquidity_lm
                    );

                    assert_eq!(
                        eth_custody_account_before
                            .distributed_rewards
                            .liquidation_lm,
                        eth_custody_account_after.distributed_rewards.liquidation_lm
                    );
                }
            }

            // Volume stats
            {
                // Change
                {
                    assert_eq!(
                        // locked amount (size) * position price (entry price)
                        eth_custody_account_before.volume_stats.close_position_usd + 7_575_000_000,
                        eth_custody_account_after.volume_stats.close_position_usd
                    );
                }

                // No change
                {
                    assert_eq!(
                        eth_custody_account_before.volume_stats.swap_usd,
                        eth_custody_account_after.volume_stats.swap_usd
                    );

                    assert_eq!(
                        eth_custody_account_before.volume_stats.open_position_usd,
                        eth_custody_account_after.volume_stats.open_position_usd
                    );

                    assert_eq!(
                        eth_custody_account_before.volume_stats.add_liquidity_usd,
                        eth_custody_account_after.volume_stats.add_liquidity_usd
                    );

                    assert_eq!(
                        eth_custody_account_before.volume_stats.remove_liquidity_usd,
                        eth_custody_account_after.volume_stats.remove_liquidity_usd
                    );

                    assert_eq!(
                        eth_custody_account_before.volume_stats.liquidation_usd,
                        eth_custody_account_after.volume_stats.liquidation_usd
                    );
                }
            }

            // Trade Stats
            {
                // Change
                {
                    assert_eq!(
                        // locked amount (size) * position price (entry price)
                        eth_custody_account_before.trade_stats.oi_long_usd - 7_575_000_000,
                        eth_custody_account_after.trade_stats.oi_long_usd
                    );

                    assert_eq!(
                        eth_custody_account_before.trade_stats.loss_usd + 968_250_016,
                        eth_custody_account_after.trade_stats.loss_usd
                    );
                }

                // No change
                {
                    assert_eq!(
                        eth_custody_account_before.trade_stats.profit_usd,
                        eth_custody_account_after.trade_stats.profit_usd
                    );

                    assert_eq!(
                        eth_custody_account_before.trade_stats.oi_short_usd,
                        eth_custody_account_after.trade_stats.oi_short_usd
                    );
                }
            }

            // Long positions
            {
                // Change
                {
                    assert_eq!(
                        eth_custody_account_before.long_positions.open_positions - 1,
                        eth_custody_account_after.long_positions.open_positions
                    );

                    assert_eq!(
                        // locked amount (size) * position price (entry price)
                        eth_custody_account_before.long_positions.size_usd - 7_575_000_000,
                        eth_custody_account_after.long_positions.size_usd
                    );

                    assert_eq!(
                        // locked amount (size) * position price (entry price)
                        eth_custody_account_before.long_positions.borrow_size_usd - 7_575_000_000,
                        eth_custody_account_after.long_positions.borrow_size_usd
                    );
                }

                // No Change
                {
                    assert_eq!(
                        // 5 ETH
                        eth_custody_account_before.long_positions.locked_amount - 5_000_000_000,
                        eth_custody_account_after.long_positions.locked_amount
                    );

                    assert_eq!(
                        eth_custody_account_before.long_positions.total_quantity - 50_000,
                        eth_custody_account_after.long_positions.total_quantity
                    );

                    assert_eq!(
                        eth_custody_account_before.long_positions.weighted_price
                            - 75_750_000_000_000,
                        eth_custody_account_after.long_positions.weighted_price
                    );

                    assert_eq!(
                        eth_custody_account_before.long_positions.collateral_usd,
                        eth_custody_account_after.long_positions.collateral_usd
                    );

                    assert_eq!(
                        eth_custody_account_before
                            .long_positions
                            .cumulative_interest_usd,
                        eth_custody_account_after
                            .long_positions
                            .cumulative_interest_usd
                    );

                    assert_eq!(
                        eth_custody_account_before
                            .long_positions
                            .cumulative_interest_snapshot,
                        eth_custody_account_after
                            .long_positions
                            .cumulative_interest_snapshot
                    );
                }
            }

            // Short positions
            {
                // No change
                {
                    assert_eq!(
                        eth_custody_account_before.short_positions.open_positions,
                        eth_custody_account_after.short_positions.open_positions
                    );

                    assert_eq!(
                        eth_custody_account_before.short_positions.collateral_usd,
                        eth_custody_account_after.short_positions.collateral_usd
                    );

                    assert_eq!(
                        eth_custody_account_before.short_positions.size_usd,
                        eth_custody_account_after.short_positions.size_usd
                    );

                    assert_eq!(
                        eth_custody_account_before.short_positions.borrow_size_usd,
                        eth_custody_account_after.short_positions.borrow_size_usd
                    );

                    assert_eq!(
                        eth_custody_account_before.short_positions.locked_amount,
                        eth_custody_account_after.short_positions.locked_amount
                    );

                    assert_eq!(
                        eth_custody_account_before.short_positions.weighted_price,
                        eth_custody_account_after.short_positions.weighted_price
                    );

                    assert_eq!(
                        eth_custody_account_before.short_positions.total_quantity,
                        eth_custody_account_after.short_positions.total_quantity
                    );

                    assert_eq!(
                        eth_custody_account_before
                            .short_positions
                            .cumulative_interest_usd,
                        eth_custody_account_after
                            .short_positions
                            .cumulative_interest_usd
                    );

                    assert_eq!(
                        eth_custody_account_before
                            .short_positions
                            .cumulative_interest_snapshot,
                        eth_custody_account_after
                            .short_positions
                            .cumulative_interest_snapshot
                    );
                }
            }
        }
    }
}

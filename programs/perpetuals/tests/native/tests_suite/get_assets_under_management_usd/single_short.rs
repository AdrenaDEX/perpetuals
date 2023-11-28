use {
    crate::{
        test_instructions,
        utils::{self, fixtures, warp_forward},
    },
    maplit::hashmap,
    perpetuals::{
        instructions::{OpenPositionParams, SetCustomOraclePriceParams},
        state::{
            cortex::Cortex,
            custody::{Custody, PricingParams},
            perpetuals::Perpetuals,
            position::Side,
        },
    },
};

const ETH_DECIMALS: u8 = 9;
const USDC_DECIMALS: u8 = 6;

pub async fn single_short() {
    let test_setup = utils::TestSetup::new(
        vec![
            utils::UserParam {
                name: "alice",
                token_balances: hashmap! {
                    "usdc" => utils::scale(200_000, USDC_DECIMALS),
                    "eth" => utils::scale(200, ETH_DECIMALS),
                },
            },
            utils::UserParam {
                name: "martin",
                token_balances: hashmap! {
                    "usdc" => utils::scale(200_000, USDC_DECIMALS),
                    "eth" => utils::scale(200, ETH_DECIMALS),
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
                    target_ratio: utils::ratio_from_percentage(40.0),
                    min_ratio: utils::ratio_from_percentage(0.0),
                    max_ratio: utils::ratio_from_percentage(100.0),
                    initial_price: utils::scale(1, USDC_DECIMALS),
                    // Make the price perfect to simplify calculations
                    initial_conf: utils::scale(0, USDC_DECIMALS),
                    pricing_params: None,
                    permissions: None,
                    // Disable fees to simplify calculations
                    fees: Some(utils::fixtures::no_fees()),
                    borrow_rate: None,
                },
                liquidity_amount: utils::scale(15_000, USDC_DECIMALS),
                payer_user_name: "alice",
            },
            utils::SetupCustodyWithLiquidityParams {
                setup_custody_params: utils::SetupCustodyParams {
                    mint_name: "eth",
                    is_stable: false,
                    target_ratio: utils::ratio_from_percentage(15.0),
                    min_ratio: utils::ratio_from_percentage(0.0),
                    max_ratio: utils::ratio_from_percentage(100.0),
                    initial_price: utils::scale(1_500, ETH_DECIMALS),
                    initial_conf: utils::scale(0, ETH_DECIMALS),
                    pricing_params: Some(PricingParams {
                        // Pay user maximum 50% of the size of the position
                        max_payoff_mult: 5_000,
                        ..fixtures::pricing_params_regular(false)
                    }),
                    permissions: None,
                    fees: Some(utils::fixtures::no_fees()),
                    borrow_rate: None,
                },
                liquidity_amount: utils::scale(10, ETH_DECIMALS),
                payer_user_name: "alice",
            },
        ],
        utils::scale(100_000, Cortex::LM_DECIMALS),
        utils::scale(200_000, Cortex::LM_DECIMALS),
        utils::scale(300_000, Cortex::LM_DECIMALS),
        utils::scale(500_000, Cortex::LM_DECIMALS),
    )
    .await;

    let martin = test_setup.get_user_keypair_by_name("martin");

    let admin_a = test_setup.get_multisig_member_keypair_by_name("admin_a");

    let multisig_signers = test_setup.get_multisig_signers();

    let eth_mint = &test_setup.get_mint_by_name("eth");
    let usdc_mint = &test_setup.get_mint_by_name("usdc");

    // Martin: Open 1 ETH short position x2
    test_instructions::open_position(
        &test_setup.program_test_ctx,
        martin,
        &test_setup.payer_keypair,
        &test_setup.pool_pda,
        eth_mint,
        Some(usdc_mint),
        OpenPositionParams {
            // max price paid (slippage implied)
            price: utils::scale(1_450, Perpetuals::USD_DECIMALS),
            collateral: utils::scale(750, USDC_DECIMALS),
            size: utils::scale(1, ETH_DECIMALS),
            side: Side::Short,
        },
    )
    .await
    .unwrap();

    // Value after add liquidity
    let aum_after_init: u128 = 30_000_000_000;

    let aum_usd_after_open_long_position =
        utils::get_assets_under_management_usd(&test_setup.program_test_ctx, test_setup.pool_pda)
            .await
            .unwrap();

    assert_eq!(
        aum_usd_after_open_long_position,
        // change comes from the difference between position exit_price and entry_price
        aum_after_init + 30_000_000
    );

    warp_forward(&test_setup.program_test_ctx, 3_600).await;

    let aum_usd_after_open_long_position_after_warp =
        utils::get_assets_under_management_usd(&test_setup.program_test_ctx, test_setup.pool_pda)
            .await
            .unwrap();

    // Check ETH custody stats
    {
        let eth_custody: Custody = utils::get_account::<Custody>(
            &test_setup.program_test_ctx,
            test_setup.custodies_info[1].custody_pda,
        )
        .await;

        let short_stats = eth_custody.short_positions;

        assert_eq!(eth_custody.assets.owned, utils::scale(10, ETH_DECIMALS));
        assert_eq!(short_stats.collateral_usd, 0);
        assert_eq!(short_stats.borrow_size_usd, 0);
        assert_eq!(short_stats.cumulative_interest_usd, 0);
        assert_eq!(short_stats.cumulative_interest_snapshot, 0);
        assert_eq!(short_stats.locked_amount, 0);
        assert_eq!(short_stats.open_positions, 1);
        assert_eq!(
            short_stats.size_usd,
            utils::scale(1_485, Perpetuals::USD_DECIMALS)
        );
        assert_eq!(short_stats.total_quantity, 10_000);
    }

    // Check USDC custody stats
    {
        let usdc_custody: Custody = utils::get_account::<Custody>(
            &test_setup.program_test_ctx,
            test_setup.custodies_info[0].custody_pda,
        )
        .await;

        let short_stats = usdc_custody.short_positions;

        assert_eq!(
            usdc_custody.assets.owned,
            utils::scale(15_000, USDC_DECIMALS)
        );
        assert_eq!(short_stats.collateral_usd, 0);
        assert_eq!(
            short_stats.borrow_size_usd,
            utils::scale_f64(742.5, Perpetuals::USD_DECIMALS)
        );
        assert_eq!(short_stats.cumulative_interest_usd, 0);
        assert_eq!(short_stats.cumulative_interest_snapshot, 0);
        assert_eq!(
            short_stats.locked_amount,
            utils::scale_f64(742.5, USDC_DECIMALS)
        );
        assert_eq!(short_stats.open_positions, 1);
        assert_eq!(short_stats.size_usd, 0);
        assert_eq!(short_stats.total_quantity, 0);
    }

    assert_eq!(
        aum_usd_after_open_long_position_after_warp,
        // change comes from the interest paid
        aum_usd_after_open_long_position + 3_675
    );

    // Makes ETH price to raise 10%
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
                price: utils::scale(1_650, ETH_DECIMALS),
                expo: -(ETH_DECIMALS as i32),
                conf: utils::scale(0, ETH_DECIMALS),
                ema: utils::scale(1_650, ETH_DECIMALS),
                publish_time,
            },
            &multisig_signers,
        )
        .await
        .unwrap();
    }

    warp_forward(&test_setup.program_test_ctx, 3_600).await;

    let aum_usd_after_open_long_position_after_price_increase =
        utils::get_assets_under_management_usd(&test_setup.program_test_ctx, test_setup.pool_pda)
            .await
            .unwrap();

    // When the price raised from 10%, user lose money that is accounted for in the PnL, thus in the assets under management
    //
    // change comes from the interest paid + user PnL (money loss + exit fee)

    // Asset under management price change explained:
    //
    // 10 ETH price changed (+10%) => +$1,500
    // User paid: $0.00735 in interest for 2h
    // User lost: $181,5 (entry price: $1,485 vs exit price: $1,666.5)
    //
    //
    assert_eq!(
        aum_usd_after_open_long_position_after_price_increase,
        aum_after_init + 1_681_507_350,
    );

    // Makes ETH price to drop 80%
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
                price: utils::scale(330, ETH_DECIMALS),
                expo: -(ETH_DECIMALS as i32),
                conf: utils::scale(0, ETH_DECIMALS),
                ema: utils::scale(330, ETH_DECIMALS),
                publish_time,
            },
            &multisig_signers,
        )
        .await
        .unwrap();
    }

    warp_forward(&test_setup.program_test_ctx, 3_600).await;

    // The idea here is to check that user gains are capped correctly
    //
    // The max user gains is 50% of position size: $742.5, thus max decrease of aum is $742.5 minus spread+fees
    //
    // This test have been added following a bug in the initial implementation where the gains wasn't capped when shorting (accounting bug)

    let aum_usd_after_open_long_position_after_price_decrease =
        utils::get_assets_under_management_usd(&test_setup.program_test_ctx, test_setup.pool_pda)
            .await
            .unwrap();

    // When the price crash of 65% (total), user gains money, thus the assets under management of the pool decrease
    //
    // change comes from the interest paid + user PnL (user money gain - exit fee)

    // Asset under management price change explained:
    //
    // 10 ETH price changed (-65%) => -$11,700
    // User paid: $0.011025 in interest for 3h
    // User potential gains: $1,151.700 (entry price: $1,485 vs exit price: $333.300)
    // User capped gains: $742.5 (capped at locked USDC => $742.5)
    //
    assert_eq!(
        aum_usd_after_open_long_position_after_price_decrease,
        aum_after_init - 12_442_488_974,
    );
}

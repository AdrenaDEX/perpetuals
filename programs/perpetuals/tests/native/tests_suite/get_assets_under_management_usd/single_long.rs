use {
    crate::{
        test_instructions,
        utils::{self, warp_forward},
    },
    maplit::hashmap,
    perpetuals::{
        instructions::OpenPositionParams,
        state::{cortex::Cortex, custody::Custody, perpetuals::Perpetuals, position::Side},
    },
};

const ETH_DECIMALS: u8 = 9;
const USDC_DECIMALS: u8 = 6;

pub async fn single_long() {
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
                    pricing_params: None,
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

    let eth_mint = &test_setup.get_mint_by_name("eth");

    // Martin: Open 1 ETH long position x1
    test_instructions::open_position(
        &test_setup.program_test_ctx,
        martin,
        &test_setup.payer_keypair,
        &test_setup.pool_pda,
        eth_mint,
        None,
        OpenPositionParams {
            // max price paid (slippage implied)
            price: utils::scale(1_550, ETH_DECIMALS),
            collateral: utils::scale(1, ETH_DECIMALS),
            size: utils::scale(1, ETH_DECIMALS),
            side: Side::Long,
        },
    )
    .await
    .unwrap()
    .0;

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

        let long_stats = eth_custody.long_positions;

        assert_eq!(eth_custody.assets.owned, utils::scale(10, ETH_DECIMALS));
        assert_eq!(long_stats.collateral_usd, 0);
        assert_eq!(
            long_stats.borrow_size_usd,
            utils::scale(1_515, Perpetuals::USD_DECIMALS)
        );
        assert_eq!(long_stats.cumulative_interest_usd, 0);
        assert_eq!(long_stats.cumulative_interest_snapshot, 0);
        assert_eq!(long_stats.locked_amount, utils::scale(1, ETH_DECIMALS));
        assert_eq!(long_stats.open_positions, 1);
        assert_eq!(
            long_stats.size_usd,
            utils::scale(1_515, Perpetuals::USD_DECIMALS)
        );
        assert_eq!(long_stats.total_quantity, 10_000);
    }

    assert_eq!(
        aum_usd_after_open_long_position_after_warp,
        // change comes from the interest paid
        aum_usd_after_open_long_position + 15_150
    );
}

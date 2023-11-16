use {
    crate::{
        test_instructions,
        utils::{self, find_associated_token_account, get_token_account_balance},
    },
    maplit::hashmap,
    perpetuals::{
        instructions::{
            ClosePositionParams, GetOpenPositionWithSwapAmountAndFeesParams,
            OpenPositionWithSwapParams,
        },
        state::{cortex::Cortex, perpetuals::Perpetuals, position::Side},
    },
    solana_sdk::signer::Signer,
};

const ETH_DECIMALS: u8 = 9;
const USDC_DECIMALS: u8 = 6;
const BTC_DECIMALS: u8 = 6;

pub async fn open_close_position_with_swap() {
    let test_setup = utils::TestSetup::new(
        vec![
            utils::UserParam {
                name: "alice",
                token_balances: hashmap! {
                    "usdc" => utils::scale(200_000, USDC_DECIMALS),
                    "eth" => utils::scale(200, ETH_DECIMALS),
                    "btc" => utils::scale(50, BTC_DECIMALS),
                },
            },
            utils::UserParam {
                name: "martin",
                token_balances: hashmap! {
                    "usdc" => utils::scale(200_000, USDC_DECIMALS),
                    "eth" => utils::scale(200, ETH_DECIMALS),
                    "btc" => utils::scale(0, BTC_DECIMALS),
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
            utils::MintParam {
                name: "btc",
                decimals: BTC_DECIMALS,
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
                    target_ratio: utils::ratio_from_percentage(40.0),
                    min_ratio: utils::ratio_from_percentage(0.0),
                    max_ratio: utils::ratio_from_percentage(100.0),
                    initial_price: utils::scale(1, USDC_DECIMALS),
                    initial_conf: utils::scale_f64(0.01, USDC_DECIMALS),
                    pricing_params: None,
                    permissions: None,
                    fees: None,
                    borrow_rate: None,
                },
                liquidity_amount: utils::scale(15_000, USDC_DECIMALS),
                payer_user_name: "alice",
            },
            utils::SetupCustodyWithLiquidityParams {
                setup_custody_params: utils::SetupCustodyParams {
                    mint_name: "eth",
                    is_stable: false,
                    is_virtual: false,
                    target_ratio: utils::ratio_from_percentage(15.0),
                    min_ratio: utils::ratio_from_percentage(0.0),
                    max_ratio: utils::ratio_from_percentage(100.0),
                    initial_price: utils::scale(1_500, ETH_DECIMALS),
                    initial_conf: utils::scale(10, ETH_DECIMALS),
                    pricing_params: None,
                    permissions: None,
                    fees: None,
                    borrow_rate: None,
                },
                liquidity_amount: utils::scale(10, ETH_DECIMALS),
                payer_user_name: "alice",
            },
            utils::SetupCustodyWithLiquidityParams {
                setup_custody_params: utils::SetupCustodyParams {
                    mint_name: "btc",
                    is_stable: false,
                    is_virtual: false,
                    target_ratio: utils::ratio_from_percentage(15.0),
                    min_ratio: utils::ratio_from_percentage(0.0),
                    max_ratio: utils::ratio_from_percentage(100.0),
                    initial_price: utils::scale(30_000, BTC_DECIMALS),
                    initial_conf: utils::scale(10, BTC_DECIMALS),
                    pricing_params: None,
                    permissions: None,
                    fees: None,
                    borrow_rate: None,
                },
                liquidity_amount: utils::scale_f64(0.5, BTC_DECIMALS),
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

    let usdc_mint = &test_setup.get_mint_by_name("usdc");
    let eth_mint = &test_setup.get_mint_by_name("eth");
    let btc_mint = &test_setup.get_mint_by_name("btc");

    let martin_usdc_ata = find_associated_token_account(&martin.pubkey(), usdc_mint).0;
    let martin_eth_ata = find_associated_token_account(&martin.pubkey(), eth_mint).0;
    let martin_btc_ata = find_associated_token_account(&martin.pubkey(), btc_mint).0;

    let mut martin_usdc_balance_before =
        get_token_account_balance(&test_setup.program_test_ctx, martin_usdc_ata).await;
    let mut martin_eth_balance_before =
        get_token_account_balance(&test_setup.program_test_ctx, martin_eth_ata).await;
    let mut martin_btc_balance_before =
        get_token_account_balance(&test_setup.program_test_ctx, martin_btc_ata).await;

    // Check preshot of what's happening
    {
        let open_position_with_swap_amount_and_fees =
            test_instructions::get_open_position_with_swap_amount_and_fees(
                &test_setup.program_test_ctx,
                &test_setup.payer_keypair,
                &test_setup.pool_pda,
                eth_mint,
                btc_mint,
                btc_mint,
                GetOpenPositionWithSwapAmountAndFeesParams {
                    collateral_amount: utils::scale_f64(0.005, ETH_DECIMALS),
                    size: utils::scale_f64(0.001, BTC_DECIMALS),
                    side: Side::Long,
                },
            )
            .await
            .unwrap();

        assert_eq!(
            open_position_with_swap_amount_and_fees.entry_price,
            utils::scale(30_300, Perpetuals::USD_DECIMALS)
        );
        assert_eq!(
            open_position_with_swap_amount_and_fees.liquidation_price,
            utils::scale(26_400, Perpetuals::USD_DECIMALS)
        );
        assert_eq!(
            open_position_with_swap_amount_and_fees.open_position_fee,
            utils::scale_f64(0.00001, BTC_DECIMALS)
        );
        assert_eq!(
            open_position_with_swap_amount_and_fees.swap_fee_in,
            utils::scale_f64(0.00005, ETH_DECIMALS)
        );
        assert_eq!(
            open_position_with_swap_amount_and_fees.swap_fee_out,
            utils::scale_f64(0.000003, BTC_DECIMALS)
        );
    }

    let position_pda = test_instructions::open_position_with_swap(
        &test_setup.program_test_ctx,
        martin,
        &test_setup.payer_keypair,
        &test_setup.pool_pda,
        eth_mint,
        btc_mint,
        OpenPositionWithSwapParams {
            // Amount of ETH to use as collateral
            // $7.5 of collateral
            collateral: utils::scale_f64(0.005, ETH_DECIMALS),
            // $30 position
            size: utils::scale_f64(0.001, BTC_DECIMALS),
            side: Side::Long,
            // max price paid for BTC when opening the position (slippage implied)
            price: utils::scale(30_400, Perpetuals::USD_DECIMALS),
        },
        None,
    )
    .await
    .unwrap()
    .0;

    let mut martin_usdc_balance_after =
        get_token_account_balance(&test_setup.program_test_ctx, martin_usdc_ata).await;
    let mut martin_eth_balance_after =
        get_token_account_balance(&test_setup.program_test_ctx, martin_eth_ata).await;
    let mut martin_btc_balance_after =
        get_token_account_balance(&test_setup.program_test_ctx, martin_btc_ata).await;

    // Double check numbers after swap & position opening
    {
        assert_eq!(martin_usdc_balance_before, martin_usdc_balance_after);
        assert_eq!(
            martin_eth_balance_before - 5_000_000,
            martin_eth_balance_after
        );
        assert_eq!(martin_btc_balance_before, martin_btc_balance_after);
    }

    martin_usdc_balance_before = martin_usdc_balance_after;
    martin_eth_balance_before = martin_eth_balance_after;
    martin_btc_balance_before = martin_btc_balance_after;

    // Martin: Close the ETH position
    test_instructions::close_position(
        &test_setup.program_test_ctx,
        martin,
        &test_setup.payer_keypair,
        &test_setup.pool_pda,
        btc_mint,
        &position_pda,
        ClosePositionParams {
            // lowest exit price paid (slippage implied)
            price: utils::scale(29_500, Perpetuals::USD_DECIMALS),
        },
    )
    .await
    .unwrap();

    martin_usdc_balance_after =
        get_token_account_balance(&test_setup.program_test_ctx, martin_usdc_ata).await;
    martin_eth_balance_after =
        get_token_account_balance(&test_setup.program_test_ctx, martin_eth_ata).await;
    martin_btc_balance_after =
        get_token_account_balance(&test_setup.program_test_ctx, martin_btc_ata).await;

    // Double check numbers after position closing
    {
        assert_eq!(martin_usdc_balance_before, martin_usdc_balance_after);
        assert_eq!(martin_eth_balance_before, martin_eth_balance_after);
        assert_eq!(martin_btc_balance_before + 198, martin_btc_balance_after);
    }
}

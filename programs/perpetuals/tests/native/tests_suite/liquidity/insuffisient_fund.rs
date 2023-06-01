use {
    crate::{
        instructions,
        utils::{
            self, MintParam, NamedSetupCustodyParams, NamedSetupCustodyWithLiquidityParams, Test,
            UserParam,
        },
    },
    maplit::hashmap,
    perpetuals::instructions::{AddLiquidityParams, RemoveLiquidityParams},
    solana_sdk::signer::Signer,
};

const USDC_DECIMALS: u8 = 6;
const ETH_DECIMALS: u8 = 9;

pub async fn insuffisient_fund() {
    let test = Test::new(
        vec![UserParam {
            name: "alice",
            token_balances: hashmap! {
                "usdc".to_string() => utils::scale(100_000, USDC_DECIMALS),
                "eth".to_string() => utils::scale(50, ETH_DECIMALS),
            },
        }],
        vec![
            MintParam {
                name: "usdc",
                decimals: USDC_DECIMALS,
            },
            MintParam {
                name: "eth",
                decimals: ETH_DECIMALS,
            },
        ],
        vec!["admin_a", "admin_b", "admin_c"],
        "usdc".to_string(),
        6,
        "ADRENA",
        "main_pool",
        vec![
            NamedSetupCustodyWithLiquidityParams {
                setup_custody_params: NamedSetupCustodyParams {
                    mint_name: "usdc",
                    is_stable: true,
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
                liquidity_amount: utils::scale(0, USDC_DECIMALS),
                payer_user_name: "alice",
            },
            NamedSetupCustodyWithLiquidityParams {
                setup_custody_params: NamedSetupCustodyParams {
                    mint_name: "eth",
                    is_stable: false,
                    target_ratio: utils::ratio_from_percentage(50.0),
                    min_ratio: utils::ratio_from_percentage(0.0),
                    max_ratio: utils::ratio_from_percentage(100.0),
                    initial_price: utils::scale(1_500, ETH_DECIMALS),
                    initial_conf: utils::scale(10, ETH_DECIMALS),
                    pricing_params: None,
                    permissions: None,
                    fees: None,
                    borrow_rate: None,
                },
                liquidity_amount: utils::scale(0, ETH_DECIMALS),
                payer_user_name: "alice",
            },
        ],
    )
    .await;

    let alice = test.get_user_keypair_by_name("alice");

    let cortex_stake_reward_mint = test.get_cortex_stake_reward_mint();

    let usdc_mint = &test.get_mint_by_name("usdc");
    let eth_mint = &test.get_mint_by_name("eth");

    // Trying to add more USDC than owned should fail
    assert!(instructions::test_add_liquidity(
        &mut test.program_test_ctx.borrow_mut(),
        alice,
        &test.payer_keypair,
        &test.pool_pda,
        &usdc_mint,
        &cortex_stake_reward_mint,
        AddLiquidityParams {
            amount_in: utils::scale(1_000_000, USDC_DECIMALS),
            min_lp_amount_out: 1
        },
    )
    .await
    .is_err());

    // Alice: add 15k USDC and 10 ETH liquidity
    {
        instructions::test_add_liquidity(
            &mut test.program_test_ctx.borrow_mut(),
            alice,
            &test.payer_keypair,
            &test.pool_pda,
            &usdc_mint,
            &cortex_stake_reward_mint,
            AddLiquidityParams {
                amount_in: utils::scale(15_000, USDC_DECIMALS),
                min_lp_amount_out: 1,
            },
        )
        .await
        .unwrap();

        instructions::test_add_liquidity(
            &mut test.program_test_ctx.borrow_mut(),
            alice,
            &test.payer_keypair,
            &test.pool_pda,
            &eth_mint,
            &cortex_stake_reward_mint,
            AddLiquidityParams {
                amount_in: utils::scale(10, ETH_DECIMALS),
                min_lp_amount_out: 1,
            },
        )
        .await
        .unwrap();
    }

    let alice_lp_token_mint_pda =
        utils::find_associated_token_account(&alice.pubkey(), &test.lp_token_mint_pda).0;

    let alice_lp_token_account_balance = utils::get_token_account_balance(
        &mut test.program_test_ctx.borrow_mut(),
        alice_lp_token_mint_pda,
    )
    .await;

    // Trying to remove more LP token than owned should fail
    assert!(instructions::test_remove_liquidity(
        &mut test.program_test_ctx.borrow_mut(),
        alice,
        &test.payer_keypair,
        &test.pool_pda,
        &usdc_mint,
        &cortex_stake_reward_mint,
        RemoveLiquidityParams {
            lp_amount_in: alice_lp_token_account_balance + 1,
            min_amount_out: 1
        },
    )
    .await
    .is_err());

    // Trying to remove more asset than owned by the pool should fail
    assert!(instructions::test_remove_liquidity(
        &mut test.program_test_ctx.borrow_mut(),
        alice,
        &test.payer_keypair,
        &test.pool_pda,
        &usdc_mint,
        &cortex_stake_reward_mint,
        RemoveLiquidityParams {
            lp_amount_in: alice_lp_token_account_balance * 75 / 100,
            min_amount_out: 1
        },
    )
    .await
    .is_err());
}

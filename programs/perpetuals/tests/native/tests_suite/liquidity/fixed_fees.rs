use {
    crate::{
        instructions,
        utils::{
            self, MintParam, NamedSetupCustodyParams, NamedSetupCustodyWithLiquidityParams, Test,
            UserParam,
        },
    },
    maplit::hashmap,
    perpetuals::{
        instructions::{AddLiquidityParams, RemoveLiquidityParams},
        state::{custody::Custody, perpetuals::Perpetuals, pool::Pool},
    },
};

const USDC_DECIMALS: u8 = 6;

pub async fn fixed_fees() {
    let test = Test::new(
        vec![UserParam {
            name: "alice",
            token_balances: hashmap! {
                "usdc".to_string() => utils::scale(100_000, USDC_DECIMALS),
            },
        }],
        vec![MintParam {
            name: "usdc",
            decimals: USDC_DECIMALS,
        }],
        vec!["admin_a", "admin_b", "admin_c"],
        // mint for the payouts of the LM token staking (ADX staking)
        "usdc".to_string(),
        6,
        "ADRENA",
        "main_pool",
        vec![NamedSetupCustodyWithLiquidityParams {
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
        }],
    )
    .await;

    let alice = test.get_user_keypair_by_name("alice");
    let cortex_stake_reward_mint = test.get_cortex_stake_reward_mint();
    let usdc_mint = &test.get_mint_by_name("usdc");

    // Check add liquidity fee
    {
        instructions::test_add_liquidity(
            &mut test.program_test_ctx.borrow_mut(),
            alice,
            &test.payer_keypair,
            &test.pool_pda,
            &usdc_mint,
            &cortex_stake_reward_mint,
            AddLiquidityParams {
                amount_in: utils::scale(1_000, USDC_DECIMALS),
                min_lp_amount_out: 1,
            },
        )
        .await
        .unwrap();

        {
            let pool_account =
                utils::get_account::<Pool>(&mut test.program_test_ctx.borrow_mut(), test.pool_pda)
                    .await;
            let custody_account = utils::get_account::<Custody>(
                &mut test.program_test_ctx.borrow_mut(),
                test.custodies_info[0].custody_pda,
            )
            .await;

            assert_eq!(
                pool_account.aum_usd,
                utils::scale_f64(999.95, USDC_DECIMALS).into(),
            );

            assert_eq!(
                custody_account.collected_fees.add_liquidity_usd,
                utils::scale(20, USDC_DECIMALS),
            );

            assert_eq!(
                custody_account.assets.protocol_fees,
                utils::scale_f64(0.05, USDC_DECIMALS),
            );
        }
    }

    // Check remove liquidity fee
    {
        instructions::test_remove_liquidity(
            &mut test.program_test_ctx.borrow_mut(),
            alice,
            &test.payer_keypair,
            &test.pool_pda,
            &usdc_mint,
            &cortex_stake_reward_mint,
            RemoveLiquidityParams {
                lp_amount_in: utils::scale(100, Perpetuals::LP_DECIMALS),
                min_amount_out: 1,
            },
        )
        .await
        .unwrap();

        {
            let pool_account =
                utils::get_account::<Pool>(&mut test.program_test_ctx.borrow_mut(), test.pool_pda)
                    .await;
            let custody_account = utils::get_account::<Custody>(
                &mut test.program_test_ctx.borrow_mut(),
                test.custodies_info[0].custody_pda,
            )
            .await;

            assert_eq!(
                pool_account.aum_usd,
                utils::scale_f64(900.967705, USDC_DECIMALS).into(),
            );

            assert_eq!(
                custody_account.collected_fees.remove_liquidity_usd,
                utils::scale_f64(3.061072, USDC_DECIMALS),
            );

            assert_eq!(
                custody_account.assets.protocol_fees,
                utils::scale_f64(0.057653, USDC_DECIMALS),
            );
        }
    }
}

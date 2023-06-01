use {
    crate::{
        instructions,
        utils::{
            self, scale, MintParam, NamedSetupCustodyParams, NamedSetupCustodyWithLiquidityParams,
            Test, UserParam,
        },
    },
    maplit::hashmap,
    perpetuals::{
        instructions::{
            AddStakeParams, AddVestParams, ClosePositionParams, OpenPositionParams,
            RemoveLiquidityParams, RemoveStakeParams, SwapParams,
        },
        state::{
            cortex::{Cortex, StakingRound},
            position::Side,
        },
    },
    solana_sdk::signer::Signer,
};

const USDC_DECIMALS: u8 = 6;
const ETH_DECIMALS: u8 = 9;

pub async fn basic_interactions() {
    let test = Test::new(
        vec![
            UserParam {
                name: "alice",
                token_balances: hashmap! {
                    "usdc".to_string() => utils::scale(1_000, USDC_DECIMALS),
                },
            },
            UserParam {
                name: "martin",
                token_balances: hashmap! {
                    "usdc".to_string()  => utils::scale(100, USDC_DECIMALS),
                    "eth".to_string()  => utils::scale(2, ETH_DECIMALS),
                },
            },
            UserParam {
                name: "paul",
                token_balances: hashmap! {
                    "usdc".to_string()  => utils::scale(150, USDC_DECIMALS),
                },
            },
        ],
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
        // mint for the payouts of the LM token staking (ADX staking)
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
                liquidity_amount: utils::scale(1_000, USDC_DECIMALS),
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
                liquidity_amount: utils::scale(1, ETH_DECIMALS),
                payer_user_name: "martin",
            },
        ],
    )
    .await;

    let alice = test.get_user_keypair_by_name("alice");
    let martin = test.get_user_keypair_by_name("martin");
    let paul = test.get_user_keypair_by_name("paul");

    let admin_a = test.get_multisig_member_keypair_by_name("admin_a");

    let cortex_stake_reward_mint = test.get_cortex_stake_reward_mint();
    let multisig_signers = test.get_multisig_signers();

    let usdc_mint = &test.get_mint_by_name("usdc");
    let eth_mint = &test.get_mint_by_name("eth");

    // warp to avoid expired blockhash
    utils::warp_forward(&mut test.program_test_ctx.borrow_mut(), 1).await;

    // Simple open/close position
    {
        // Martin: Open 0.1 ETH position
        let position_pda = instructions::test_open_position(
            &mut test.program_test_ctx.borrow_mut(),
            martin,
            &test.payer_keypair,
            &test.pool_pda,
            eth_mint,
            &cortex_stake_reward_mint,
            OpenPositionParams {
                // max price paid (slippage implied)
                price: utils::scale(1_550, USDC_DECIMALS),
                collateral: utils::scale_f64(0.1, ETH_DECIMALS),
                size: utils::scale_f64(0.1, ETH_DECIMALS),
                side: Side::Long,
            },
        )
        .await
        .unwrap()
        .0;

        // Martin: Close the ETH position
        instructions::test_close_position(
            &mut test.program_test_ctx.borrow_mut(),
            martin,
            &test.payer_keypair,
            &test.pool_pda,
            &eth_mint,
            &cortex_stake_reward_mint,
            &position_pda,
            ClosePositionParams {
                // lowest exit price paid (slippage implied)
                price: utils::scale(1_450, USDC_DECIMALS),
            },
        )
        .await
        .unwrap();
    }

    // Simple swap
    {
        // Paul: Swap 150 USDC for ETH
        instructions::test_swap(
            &mut test.program_test_ctx.borrow_mut(),
            paul,
            &test.payer_keypair,
            &test.pool_pda,
            &eth_mint,
            // The program receives USDC
            &usdc_mint,
            &cortex_stake_reward_mint,
            SwapParams {
                amount_in: utils::scale(150, USDC_DECIMALS),

                // 1% slippage
                min_amount_out: utils::scale(150, USDC_DECIMALS)
                    / utils::scale(1_500, ETH_DECIMALS)
                    * 99
                    / 100,
            },
        )
        .await
        .unwrap();
    }

    // Remove liquidity
    {
        let alice_lp_token =
            utils::find_associated_token_account(&alice.pubkey(), &test.lp_token_mint_pda).0;

        let alice_lp_token_balance = utils::get_token_account_balance(
            &mut test.program_test_ctx.borrow_mut(),
            alice_lp_token,
        )
        .await;

        // Alice: Remove 100% of provided liquidity (1k USDC less fees)
        instructions::test_remove_liquidity(
            &mut test.program_test_ctx.borrow_mut(),
            alice,
            &test.payer_keypair,
            &test.pool_pda,
            &usdc_mint,
            &cortex_stake_reward_mint,
            RemoveLiquidityParams {
                lp_amount_in: alice_lp_token_balance,
                min_amount_out: 1,
            },
        )
        .await
        .unwrap();
    }

    // Simple vest and claim
    {
        let current_time =
            utils::get_current_unix_timestamp(&mut test.program_test_ctx.borrow_mut()).await;

        // Alice: vest 2 token, unlock period from now to in 7 days
        instructions::test_add_vest(
            &mut test.program_test_ctx.borrow_mut(),
            admin_a,
            &test.payer_keypair,
            alice,
            &test.governance_realm_pda,
            &AddVestParams {
                amount: utils::scale(2, Cortex::LM_DECIMALS),
                unlock_start_timestamp: current_time,
                unlock_end_timestamp: utils::days_in_seconds(7) + current_time,
            },
            &multisig_signers,
        )
        .await
        .unwrap();

        // warp to have tokens to claim
        utils::warp_forward(
            &mut test.program_test_ctx.borrow_mut(),
            utils::days_in_seconds(7),
        )
        .await;

        // Alice: claim vest
        instructions::test_claim_vest(
            &mut test.program_test_ctx.borrow_mut(),
            &test.payer_keypair,
            alice,
            &test.governance_realm_pda,
        )
        .await
        .unwrap();
    }

    // Stake
    {
        // Alice: add stake LM token
        instructions::test_add_stake(
            &mut test.program_test_ctx.borrow_mut(),
            alice,
            &test.payer_keypair,
            AddStakeParams {
                amount: scale(1, Cortex::LM_DECIMALS),
            },
            &cortex_stake_reward_mint,
            &test.governance_realm_pda,
        )
        .await
        .unwrap();

        // Alice: remove stake LM token
        instructions::test_remove_stake(
            &mut test.program_test_ctx.borrow_mut(),
            alice,
            &test.payer_keypair,
            RemoveStakeParams {
                amount: scale(1, Cortex::LM_DECIMALS),
            },
            &cortex_stake_reward_mint,
            &test.governance_realm_pda,
        )
        .await
        .unwrap();

        // Alice: test claim stake (no stake account, none)
        instructions::test_claim_stake(
            &mut test.program_test_ctx.borrow_mut(),
            alice,
            alice,
            &test.payer_keypair,
            &test.governance_realm_pda,
            &cortex_stake_reward_mint,
        )
        .await
        .unwrap();

        // resolution of the round
        // warps to when the round is resolvable
        utils::warp_forward(
            &mut test.program_test_ctx.borrow_mut(),
            StakingRound::ROUND_MIN_DURATION_SECONDS,
        )
        .await;

        instructions::test_resolve_staking_round(
            &mut test.program_test_ctx.borrow_mut(),
            alice,
            alice,
            &test.payer_keypair,
            &cortex_stake_reward_mint,
        )
        .await
        .unwrap();
    }
}

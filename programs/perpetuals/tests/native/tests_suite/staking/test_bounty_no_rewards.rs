use {
    crate::{instructions, utils},
    bonfida_test_utils::ProgramTestContextExt,
    maplit::hashmap,
    perpetuals::{
        instructions::{AddStakeParams, AddVestParams, SwapParams},
        state::cortex::{Cortex, StakingRound},
    },
    solana_sdk::signer::Signer,
};

const USDC_DECIMALS: u8 = 6;
const ETH_DECIMALS: u8 = 9;

// this test is about filling the maximum number of staking rounds the systme can hold (StakingRound::MAX_RESOLVED_ROUNDS)
// and playing around that limit for different edge cases

pub async fn test_bounty_no_rewards() {
    let test = utils::Test::new(
        vec![
            utils::UserParam {
                name: "alice",
                token_balances: hashmap! {
                    "usdc".to_string() => utils::scale(1_000, USDC_DECIMALS),
                    "eth".to_string() => utils::scale(2, USDC_DECIMALS),
                },
            },
            utils::UserParam {
                name: "martin",
                token_balances: hashmap! {
                    "usdc".to_string()  => utils::scale(1_000, USDC_DECIMALS),
                    "eth".to_string()  => utils::scale(2, ETH_DECIMALS),
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
        // mint for the payouts of the LM token staking (ADX staking)
        "usdc".to_string(),
        6,
        "ADRENA",
        "main_pool",
        vec![
            utils::NamedSetupCustodyWithLiquidityParams {
                setup_custody_params: utils::NamedSetupCustodyParams {
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
            utils::NamedSetupCustodyWithLiquidityParams {
                setup_custody_params: utils::NamedSetupCustodyParams {
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

    let admin_a = test.get_multisig_member_keypair_by_name("admin_a");

    let cortex_stake_reward_mint = test.get_cortex_stake_reward_mint();
    let multisig_signers = test.get_multisig_signers();

    let usdc_mint = &test.get_mint_by_name("usdc");
    let eth_mint = &test.get_mint_by_name("eth");

    // Prep work: Alice get 2 governance tokens using vesting
    {
        let current_time =
            utils::get_current_unix_timestamp(&mut test.program_test_ctx.borrow_mut()).await;

        instructions::test_add_vest(
            &mut test.program_test_ctx.borrow_mut(),
            admin_a,
            &test.payer_keypair,
            alice,
            &test.governance_realm_pda,
            &AddVestParams {
                amount: utils::scale(2, Cortex::LM_DECIMALS),
                unlock_start_timestamp: current_time,
                unlock_end_timestamp: current_time + utils::days_in_seconds(7),
            },
            &multisig_signers,
        )
        .await
        .unwrap();

        // Move until vest end
        utils::warp_forward(
            &mut test.program_test_ctx.borrow_mut(),
            utils::days_in_seconds(7),
        )
        .await;

        instructions::test_claim_vest(
            &mut test.program_test_ctx.borrow_mut(),
            &test.payer_keypair,
            alice,
            &test.governance_realm_pda,
        )
        .await
        .unwrap();
    }

    // Prep work: Generate some platform activity to fill current round' rewards
    {
        // Martin: Swap 500 USDC for ETH
        instructions::test_swap(
            &mut test.program_test_ctx.borrow_mut(),
            martin,
            &test.payer_keypair,
            &test.pool_pda,
            &eth_mint,
            // The program receives USDC
            &usdc_mint,
            &cortex_stake_reward_mint,
            SwapParams {
                amount_in: utils::scale(500, USDC_DECIMALS),
                min_amount_out: 0,
            },
        )
        .await
        .unwrap();
    }

    // tests bounties
    {
        // GIVEN
        let alice_stake_reward_token_account_address =
            utils::find_associated_token_account(&alice.pubkey(), &cortex_stake_reward_mint).0;
        let martin_stake_reward_token_account_address =
            utils::find_associated_token_account(&martin.pubkey(), &cortex_stake_reward_mint).0;
        let alice_stake_reward_token_account_before = test
            .program_test_ctx
            .borrow_mut()
            .get_token_account(alice_stake_reward_token_account_address)
            .await
            .unwrap();
        let martin_stake_reward_token_account_before = test
            .program_test_ctx
            .borrow_mut()
            .get_token_account(martin_stake_reward_token_account_address)
            .await
            .unwrap();

        // Alice: add stake LM token
        instructions::test_add_stake(
            &mut test.program_test_ctx.borrow_mut(),
            alice,
            &test.payer_keypair,
            AddStakeParams {
                amount: utils::scale(1, Cortex::LM_DECIMALS),
            },
            &cortex_stake_reward_mint,
            &test.governance_realm_pda,
        )
        .await
        .unwrap();

        // Info - at this stage, alice won't be eligible for current round rewards, as she joined after round inception

        // go to next round warps in the future
        utils::warp_forward(
            &mut test.program_test_ctx.borrow_mut(),
            StakingRound::ROUND_MIN_DURATION_SECONDS,
        )
        .await;

        // resolve round
        instructions::test_resolve_staking_round(
            &mut test.program_test_ctx.borrow_mut(),
            alice,
            alice,
            &test.payer_keypair,
            &cortex_stake_reward_mint,
        )
        .await
        .unwrap();

        // Alice: test claim stake (stake account but not eligible for current round, none)
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

        // THEN
        let alice_stake_reward_token_account_after = test
            .program_test_ctx
            .borrow_mut()
            .get_token_account(alice_stake_reward_token_account_address)
            .await
            .unwrap();

        // alice didn't receive stake rewards
        assert_eq!(
            alice_stake_reward_token_account_after.amount,
            alice_stake_reward_token_account_before.amount
        );

        // Info - new round started, forwarding the previous reward since no stake previously
        // Info - this time Alice was subscribed in time and will qualify for rewards

        // go to next round warps in the future
        utils::warp_forward(
            &mut test.program_test_ctx.borrow_mut(),
            StakingRound::ROUND_MIN_DURATION_SECONDS,
        )
        .await;

        // resolve round
        instructions::test_resolve_staking_round(
            &mut test.program_test_ctx.borrow_mut(),
            alice,
            alice,
            &test.payer_keypair,
            &cortex_stake_reward_mint,
        )
        .await
        .unwrap();

        // Martin: test claim stake (stake account eligible for round, some for Alice only)
        // but not eligible for bounty
        instructions::test_claim_stake(
            &mut test.program_test_ctx.borrow_mut(),
            martin,
            alice,
            &test.payer_keypair,
            &test.governance_realm_pda,
            &cortex_stake_reward_mint,
        )
        .await
        .unwrap();

        // THEN
        let alice_stake_reward_token_account_before = alice_stake_reward_token_account_after;
        let alice_stake_reward_token_account_after = test
            .program_test_ctx
            .borrow_mut()
            .get_token_account(alice_stake_reward_token_account_address)
            .await
            .unwrap();
        let martin_stake_reward_token_account_after = test
            .program_test_ctx
            .borrow_mut()
            .get_token_account(martin_stake_reward_token_account_address)
            .await
            .unwrap();

        // alice received stake rewards
        assert!(
            alice_stake_reward_token_account_after.amount
                > alice_stake_reward_token_account_before.amount
        );
        // martin did not received stake rewards (bounty)
        assert_eq!(
            martin_stake_reward_token_account_after.amount,
            martin_stake_reward_token_account_before.amount
        );
    }
}

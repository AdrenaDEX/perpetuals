use {
    crate::{
        test_instructions,
        utils::{self, pda},
    },
    maplit::hashmap,
    perpetuals::{
        instructions::{
            AddLiquidityParams, AddLockedStakeParams, AddVestParams, BucketName,
            RemoveLockedStakeParams,
        },
        state::{cortex::Cortex, staking::StakingRound},
    },
    solana_sdk::signer::Signer,
};

const USDC_DECIMALS: u8 = 6;
const ETH_DECIMALS: u8 = 9;

pub async fn locked_staking_30d() {
    let test_setup = utils::TestSetup::new(
        vec![
            utils::UserParam {
                name: "alice",
                token_balances: hashmap! {
                    "usdc" => utils::scale(3_000, USDC_DECIMALS),
                    "eth" => utils::scale(2, ETH_DECIMALS),
                },
            },
            utils::UserParam {
                name: "martin",
                token_balances: hashmap! {
                    "usdc" => utils::scale(3_000, USDC_DECIMALS),
                    "eth" => utils::scale(2, ETH_DECIMALS),
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
                liquidity_amount: utils::scale(1_500, USDC_DECIMALS),
                payer_user_name: "alice",
            },
            utils::SetupCustodyWithLiquidityParams {
                setup_custody_params: utils::SetupCustodyParams {
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
                payer_user_name: "alice",
            },
        ],
        utils::scale(1_000_000, Cortex::LM_DECIMALS),
        utils::scale(1_000_000, Cortex::LM_DECIMALS),
        utils::scale(1_000_000, Cortex::LM_DECIMALS),
        utils::scale(1_000_000, Cortex::LM_DECIMALS),
    )
    .await;

    let alice = test_setup.get_user_keypair_by_name("alice");
    let martin = test_setup.get_user_keypair_by_name("martin");

    let admin_a = test_setup.get_multisig_member_keypair_by_name("admin_a");

    let cortex_stake_reward_mint = test_setup.get_cortex_stake_reward_mint();
    let multisig_signers = test_setup.get_multisig_signers();

    let eth_mint = &test_setup.get_mint_by_name("eth");

    let clockwork_worker = test_setup.get_clockwork_worker();
    let lm_token_mint_pda = pda::get_lm_token_mint_pda().0;

    let alice_lm_token_account_address =
        utils::find_associated_token_account(&alice.pubkey(), &lm_token_mint_pda).0;

    let alice_staking_reward_token_account_address =
        utils::find_associated_token_account(&alice.pubkey(), &cortex_stake_reward_mint).0;

    // Prep work: Alice get 2 governance tokens using vesting
    {
        let current_time = utils::get_current_unix_timestamp(&test_setup.program_test_ctx).await;

        test_instructions::add_vest(
            &test_setup.program_test_ctx,
            admin_a,
            &test_setup.payer_keypair,
            alice,
            &test_setup.governance_realm_pda,
            &AddVestParams {
                amount: utils::scale(2, Cortex::LM_DECIMALS),
                origin_bucket: BucketName::CoreContributor,
                unlock_start_timestamp: current_time,
                unlock_end_timestamp: current_time + utils::days_in_seconds(7),
            },
            &multisig_signers,
        )
        .await
        .unwrap();

        // Move until vest end
        utils::warp_forward(&test_setup.program_test_ctx, utils::days_in_seconds(7)).await;

        test_instructions::claim_vest(
            &test_setup.program_test_ctx,
            &test_setup.payer_keypair,
            alice,
            &test_setup.governance_realm_pda,
        )
        .await
        .unwrap();
    }

    // Alice: start 30d locked staking
    {
        let stakes_claim_cron_thread_id =
            utils::get_current_unix_timestamp(&test_setup.program_test_ctx).await as u64;

        test_instructions::init_user_staking(
            &test_setup.program_test_ctx,
            alice,
            &test_setup.payer_keypair,
            &lm_token_mint_pda,
            perpetuals::instructions::InitUserStakingParams {
                stakes_claim_cron_thread_id,
            },
        )
        .await
        .unwrap();

        utils::warp_forward(&test_setup.program_test_ctx, 1).await;

        let stake_resolution_thread_id =
            utils::get_current_unix_timestamp(&test_setup.program_test_ctx).await as u64;

        test_instructions::add_locked_stake(
            &test_setup.program_test_ctx,
            alice,
            &test_setup.payer_keypair,
            AddLockedStakeParams {
                amount: utils::scale(1, Cortex::LM_DECIMALS),
                locked_days: 30,
                stake_resolution_thread_id,
            },
            &lm_token_mint_pda,
            &test_setup.governance_realm_pda,
        )
        .await
        .unwrap();
    }

    utils::warp_forward(&test_setup.program_test_ctx, 1).await;

    // Alice: claim when there is nothing to claim yet
    {
        let balance_before = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            alice_staking_reward_token_account_address,
        )
        .await;

        let lm_balance_before = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            alice_lm_token_account_address,
        )
        .await;

        test_instructions::claim_stakes(
            &test_setup.program_test_ctx,
            alice,
            alice,
            &test_setup.payer_keypair,
            &lm_token_mint_pda,
        )
        .await
        .unwrap();

        let balance_after = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            alice_staking_reward_token_account_address,
        )
        .await;

        let lm_balance_after = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            alice_lm_token_account_address,
        )
        .await;

        assert_eq!(balance_before, balance_after);
        assert_eq!(lm_balance_before, lm_balance_after);
    }

    // warp to the next round and resolve the current one
    // this round bear no rewards for the new staking at the staking started during the round
    {
        utils::warp_forward(
            &test_setup.program_test_ctx,
            StakingRound::ROUND_MIN_DURATION_SECONDS,
        )
        .await;

        test_instructions::resolve_staking_round(
            &test_setup.program_test_ctx,
            alice,
            alice,
            &test_setup.payer_keypair,
            &lm_token_mint_pda,
        )
        .await
        .unwrap();
    }

    // Use add liquidity to generate rewards for the current round
    {
        // Generate platform activity to fill current round' rewards
        test_instructions::add_liquidity(
            &test_setup.program_test_ctx,
            martin,
            &test_setup.payer_keypair,
            &test_setup.pool_pda,
            eth_mint,
            AddLiquidityParams {
                amount_in: utils::scale_f64(0.25, ETH_DECIMALS),
                min_lp_amount_out: 1,
            },
        )
        .await
        .unwrap();
    }

    // warp to the next round and resolve the current one
    // this round bear rewards for the new staking at the staking started before the round
    {
        utils::warp_forward(
            &test_setup.program_test_ctx,
            StakingRound::ROUND_MIN_DURATION_SECONDS,
        )
        .await;

        test_instructions::resolve_staking_round(
            &test_setup.program_test_ctx,
            alice,
            alice,
            &test_setup.payer_keypair,
            &lm_token_mint_pda,
        )
        .await
        .unwrap();
    }

    utils::warp_forward(&test_setup.program_test_ctx, 1).await;

    // Claim when there is one round worth of rewards to claim
    {
        let balance_before = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            alice_staking_reward_token_account_address,
        )
        .await;

        let lm_balance_before = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            alice_lm_token_account_address,
        )
        .await;

        test_instructions::claim_stakes(
            &test_setup.program_test_ctx,
            alice,
            alice,
            &test_setup.payer_keypair,
            &lm_token_mint_pda,
        )
        .await
        .unwrap();

        let balance_after = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            alice_staking_reward_token_account_address,
        )
        .await;

        let lm_balance_after = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            alice_lm_token_account_address,
        )
        .await;

        assert_eq!(balance_after - balance_before, 38_176_930);
        assert_eq!(lm_balance_after - lm_balance_before, 2_000_000);
    }

    // Move 30d in the future where staking have ended
    {
        utils::warp_forward(&test_setup.program_test_ctx, utils::days_in_seconds(30)).await;

        test_instructions::resolve_staking_round(
            &test_setup.program_test_ctx,
            alice,
            alice,
            &test_setup.payer_keypair,
            &lm_token_mint_pda,
        )
        .await
        .unwrap();

        utils::execute_claim_stakes_thread(
            &test_setup.program_test_ctx,
            &clockwork_worker,
            &test_setup.clockwork_signatory,
            alice,
            &test_setup.payer_keypair,
            &cortex_stake_reward_mint,
        )
        .await
        .unwrap();
    }

    // Remove the stake without resolving it first should fail
    assert!(test_instructions::remove_locked_stake(
        &test_setup.program_test_ctx,
        alice,
        &test_setup.payer_keypair,
        RemoveLockedStakeParams {
            locked_stake_index: 0,
        },
        &cortex_stake_reward_mint,
        &test_setup.governance_realm_pda,
    )
    .await
    .is_err());

    // Trigger clockwork thread execution manually
    utils::execute_finalize_locked_stake_thread(
        &test_setup.program_test_ctx,
        &clockwork_worker,
        &test_setup.clockwork_signatory,
        alice,
        &test_setup.payer_keypair,
        &test_setup.governance_realm_pda,
        0,
    )
    .await
    .unwrap();

    utils::warp_forward(&test_setup.program_test_ctx, 1).await;

    // Remove the stake
    test_instructions::remove_locked_stake(
        &test_setup.program_test_ctx,
        alice,
        &test_setup.payer_keypair,
        RemoveLockedStakeParams {
            locked_stake_index: 0,
        },
        &cortex_stake_reward_mint,
        &test_setup.governance_realm_pda,
    )
    .await
    .unwrap();
}

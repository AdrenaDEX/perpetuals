use {
    crate::{test_instructions, utils},
    maplit::hashmap,
    perpetuals::{
        instructions::{AddLiquidStakeParams, AddLiquidityParams, AddVestParams},
        state::cortex::{Cortex, StakingRound},
    },
    solana_sdk::signer::Signer,
};

const USDC_DECIMALS: u8 = 6;
const ETH_DECIMALS: u8 = 9;

pub async fn auto_claim() {
    let test_setup = utils::TestSetup::new(
        vec![utils::UserParam {
            name: "alice",
            token_balances: hashmap! {
                "usdc" => utils::scale(3_000, USDC_DECIMALS),
                "eth" => utils::scale(50, ETH_DECIMALS),
            },
        }],
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
                liquidity_amount: utils::scale(1_500, USDC_DECIMALS),
                payer_user_name: "alice",
            },
            utils::SetupCustodyWithLiquidityParams {
                setup_custody_params: utils::SetupCustodyParams {
                    mint_name: "eth",
                    is_stable: false,
                    is_virtual: false,
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
    )
    .await;

    let alice = test_setup.get_user_keypair_by_name("alice");

    let eth_mint = test_setup.get_mint_by_name("eth");
    let usdc_mint = test_setup.get_mint_by_name("usdc");

    let admin_a = test_setup.get_multisig_member_keypair_by_name("admin_a");

    let cortex_stake_reward_mint = test_setup.get_cortex_stake_reward_mint();
    let multisig_signers = test_setup.get_multisig_signers();

    let clockwork_worker = test_setup.get_clockwork_worker();

    let alice_usdc_ata = utils::find_associated_token_account(&alice.pubkey(), &usdc_mint).0;

    // Prep work: Alice get 2 governance tokens using vesting
    {
        let current_time =
            utils::get_current_unix_timestamp(&mut test_setup.program_test_ctx.borrow_mut()).await;

        test_instructions::add_vest(
            &mut test_setup.program_test_ctx.borrow_mut(),
            admin_a,
            &test_setup.payer_keypair,
            alice,
            &test_setup.governance_realm_pda,
            &AddVestParams {
                amount: utils::scale(10, Cortex::LM_DECIMALS),
                unlock_start_timestamp: current_time,
                unlock_end_timestamp: current_time + utils::days_in_seconds(7),
            },
            &multisig_signers,
        )
        .await
        .unwrap();

        // Move until vest end
        utils::warp_forward(
            &mut test_setup.program_test_ctx.borrow_mut(),
            utils::days_in_seconds(7),
        )
        .await;

        test_instructions::claim_vest(
            &mut test_setup.program_test_ctx.borrow_mut(),
            &test_setup.payer_keypair,
            alice,
            &test_setup.governance_realm_pda,
        )
        .await
        .unwrap();
    }

    let stakes_claim_cron_thread_id =
        utils::get_current_unix_timestamp(&mut test_setup.program_test_ctx.borrow_mut()).await
            as u64;

    test_instructions::init_staking(
        &mut test_setup.program_test_ctx.borrow_mut(),
        alice,
        &test_setup.payer_keypair,
        &cortex_stake_reward_mint,
        perpetuals::instructions::InitStakingParams {
            stakes_claim_cron_thread_id,
        },
    )
    .await
    .unwrap();

    // Alice: add liquid staking
    test_instructions::add_liquid_stake(
        &mut test_setup.program_test_ctx.borrow_mut(),
        alice,
        &test_setup.payer_keypair,
        AddLiquidStakeParams {
            amount: utils::scale(1, Cortex::LM_DECIMALS),
        },
        &cortex_stake_reward_mint,
        &test_setup.governance_realm_pda,
    )
    .await
    .unwrap();

    utils::warp_forward(&mut test_setup.program_test_ctx.borrow_mut(), 1).await;

    // Simulate rounds being created and auto-claim cron getting triggered
    //
    // User should receive rewards on the way automatically, and resolved_rounds array should never overflow
    let alice_usdc_balance_before = utils::get_token_account_balance(
        &mut test_setup.program_test_ctx.borrow_mut(),
        alice_usdc_ata,
    )
    .await;

    // Store the index of the round where cron has been executed
    let mut cron_executions: Vec<u64> = Vec::new();

    for i in 0..100 {
        // Generate platform activity to fill current round' rewards
        {
            test_instructions::add_liquidity(
                &mut test_setup.program_test_ctx.borrow_mut(),
                alice,
                &test_setup.payer_keypair,
                &test_setup.pool_pda,
                &eth_mint,
                &cortex_stake_reward_mint,
                AddLiquidityParams {
                    amount_in: utils::scale_f64(0.01, ETH_DECIMALS),
                    min_lp_amount_out: 1,
                },
            )
            .await
            .unwrap();
        }

        // Resolve current round
        {
            utils::warp_forward(
                &mut test_setup.program_test_ctx.borrow_mut(),
                StakingRound::ROUND_MIN_DURATION_SECONDS,
            )
            .await;

            test_instructions::resolve_staking_round(
                &mut test_setup.program_test_ctx.borrow_mut(),
                alice,
                alice,
                &test_setup.payer_keypair,
                &cortex_stake_reward_mint,
            )
            .await
            .unwrap();

            utils::warp_forward(&mut test_setup.program_test_ctx.borrow_mut(), 1).await;
        }

        // Check if cron claim can trigger
        {
            let has_cron_be_executed = utils::execute_claim_stakes_thread(
                &mut test_setup.program_test_ctx.borrow_mut(),
                &clockwork_worker,
                &test_setup.clockwork_signatory,
                alice,
                &test_setup.payer_keypair,
                &cortex_stake_reward_mint,
            )
            .await
            .unwrap();

            if has_cron_be_executed {
                cron_executions.push(i);
            }
        }
    }

    let alice_usdc_balance_after = utils::get_token_account_balance(
        &mut test_setup.program_test_ctx.borrow_mut(),
        alice_usdc_ata,
    )
    .await;

    // Check that alice received rewards on the way
    assert!(alice_usdc_balance_before < alice_usdc_balance_after);

    // Check number of time cron has been executed
    // Cron execution is not deterministic as hour/min/sec impact trigger conditions, depends on when tests are runned
    // cron execution may trigger on different rounds
    assert!(!cron_executions.is_empty() && cron_executions.len() <= 2);

    println!("Cron executions: {:?}", cron_executions);
}

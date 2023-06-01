use {
    crate::{
        adapters, instructions,
        utils::{self, fixtures, pda, scale},
    },
    bonfida_test_utils::ProgramTestExt,
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
    solana_program_test::ProgramTest,
    solana_sdk::signer::Signer,
};

const ROOT_AUTHORITY: usize = 0;
const PERPETUALS_UPGRADE_AUTHORITY: usize = 1;
const MULTISIG_MEMBER_A: usize = 2;
const MULTISIG_MEMBER_B: usize = 3;
const MULTISIG_MEMBER_C: usize = 4;
const PAYER: usize = 5;
const USER_ALICE: usize = 6;
const USER_MARTIN: usize = 7;
const USER_PAUL: usize = 8;

const KEYPAIRS_COUNT: usize = 9;

const USDC_DECIMALS: u8 = 6;
const ETH_DECIMALS: u8 = 9;
const GOV_TOKEN_DECIMALS: u8 = 6;

pub async fn basic_interactions() {
    let mut program_test = ProgramTest::default();

    // Initialize the accounts that will be used during the test suite
    let keypairs =
        utils::create_and_fund_multiple_accounts(&mut program_test, KEYPAIRS_COUNT).await;

    // Initialize mints
    let usdc_mint = program_test
        .add_mint(None, USDC_DECIMALS, &keypairs[ROOT_AUTHORITY].pubkey())
        .0;
    let eth_mint = program_test
        .add_mint(None, ETH_DECIMALS, &keypairs[ROOT_AUTHORITY].pubkey())
        .0;

    // Deploy programs
    utils::add_perpetuals_program(&mut program_test, &keypairs[PERPETUALS_UPGRADE_AUTHORITY]).await;
    utils::add_spl_governance_program(&mut program_test, &keypairs[PERPETUALS_UPGRADE_AUTHORITY])
        .await;

    // Start the client and connect to localnet validator
    let mut program_test_ctx = program_test.start_with_context().await;

    let upgrade_authority = &keypairs[PERPETUALS_UPGRADE_AUTHORITY];

    let multisig_signers = &[
        &keypairs[MULTISIG_MEMBER_A],
        &keypairs[MULTISIG_MEMBER_B],
        &keypairs[MULTISIG_MEMBER_C],
    ];

    let governance_realm_pda = pda::get_governance_realm_pda("ADRENA".to_string());

    // mint for the payouts of the LM token staking (ADX staking)
    let cortex_stake_reward_mint = usdc_mint;

    instructions::test_init(
        &mut program_test_ctx,
        upgrade_authority,
        fixtures::init_params_permissions_full(1),
        &governance_realm_pda,
        &cortex_stake_reward_mint,
        multisig_signers,
    )
    .await
    .unwrap();

    let gov_token_mint_pda = pda::get_governance_token_mint_pda().0;

    adapters::spl_governance::create_realm(
        &mut program_test_ctx,
        &keypairs[ROOT_AUTHORITY],
        &keypairs[PAYER],
        "ADRENA".to_string(),
        utils::scale(10_000, GOV_TOKEN_DECIMALS),
        &gov_token_mint_pda,
    )
    .await
    .unwrap();

    // Initialize and fund associated token accounts
    {
        let lm_token_mint = utils::pda::get_lm_token_mint_pda().0;

        // Alice: mint 1k USDC, create LM token account, create stake reward token account
        {
            utils::initialize_and_fund_token_account(
                &mut program_test_ctx,
                &usdc_mint,
                &keypairs[USER_ALICE].pubkey(),
                &keypairs[ROOT_AUTHORITY],
                utils::scale(1_000, USDC_DECIMALS),
            )
            .await;

            utils::initialize_token_account(
                &mut program_test_ctx,
                &lm_token_mint,
                &keypairs[USER_ALICE].pubkey(),
            )
            .await;

            utils::initialize_token_account(
                &mut program_test_ctx,
                &cortex_stake_reward_mint,
                &keypairs[USER_ALICE].pubkey(),
            )
            .await;
        }

        // Martin: mint 100 USDC and 2 ETH, create LM token account
        {
            utils::initialize_and_fund_token_account(
                &mut program_test_ctx,
                &usdc_mint,
                &keypairs[USER_MARTIN].pubkey(),
                &keypairs[ROOT_AUTHORITY],
                utils::scale(100, USDC_DECIMALS),
            )
            .await;

            utils::initialize_and_fund_token_account(
                &mut program_test_ctx,
                &eth_mint,
                &keypairs[USER_MARTIN].pubkey(),
                &keypairs[ROOT_AUTHORITY],
                utils::scale(2, ETH_DECIMALS),
            )
            .await;

            utils::initialize_token_account(
                &mut program_test_ctx,
                &lm_token_mint,
                &keypairs[USER_MARTIN].pubkey(),
            )
            .await;
        }

        // Paul: mint 150 USDC, create LM token account
        {
            utils::initialize_and_fund_token_account(
                &mut program_test_ctx,
                &usdc_mint,
                &keypairs[USER_PAUL].pubkey(),
                &keypairs[ROOT_AUTHORITY],
                utils::scale(150, USDC_DECIMALS),
            )
            .await;

            utils::initialize_token_account(
                &mut program_test_ctx,
                &eth_mint,
                &keypairs[USER_PAUL].pubkey(),
            )
            .await;

            utils::initialize_token_account(
                &mut program_test_ctx,
                &lm_token_mint,
                &keypairs[USER_PAUL].pubkey(),
            )
            .await;
        }
    }

    println!("usdc mint: {}", usdc_mint);
    println!("eth mint: {}", eth_mint);
    println!(
        "cortex_stake_reward_mint mint: {}",
        cortex_stake_reward_mint
    );
    let (pool_pda, _, lp_token_mint_pda, _, _) = utils::setup_pool_with_custodies_and_liquidity(
        &mut program_test_ctx,
        &keypairs[MULTISIG_MEMBER_A],
        "FOO",
        &keypairs[PAYER],
        &cortex_stake_reward_mint,
        multisig_signers,
        vec![
            utils::SetupCustodyWithLiquidityParams {
                setup_custody_params: utils::SetupCustodyParams {
                    mint: usdc_mint,
                    decimals: USDC_DECIMALS,
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
                // Alice: add 1k USDC liquidity
                liquidity_amount: utils::scale(1_000, USDC_DECIMALS),
                payer: utils::copy_keypair(&keypairs[USER_ALICE]),
            },
            utils::SetupCustodyWithLiquidityParams {
                setup_custody_params: utils::SetupCustodyParams {
                    mint: eth_mint,
                    decimals: ETH_DECIMALS,
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
                // Martin: add 1 ETH liquidity
                liquidity_amount: utils::scale(1, ETH_DECIMALS),
                payer: utils::copy_keypair(&keypairs[USER_MARTIN]),
            },
        ],
    )
    .await;

    // warp to avoid expired blockhash
    utils::warp_forward(&mut program_test_ctx, 1).await;

    // Simple open/close position
    {
        // Martin: Open 0.1 ETH position
        let position_pda = instructions::test_open_position(
            &mut program_test_ctx,
            &keypairs[USER_MARTIN],
            &keypairs[PAYER],
            &pool_pda,
            &eth_mint,
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
            &mut program_test_ctx,
            &keypairs[USER_MARTIN],
            &keypairs[PAYER],
            &pool_pda,
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
            &mut program_test_ctx,
            &keypairs[USER_PAUL],
            &keypairs[PAYER],
            &pool_pda,
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
        let alice_lp_token = utils::find_associated_token_account(
            &keypairs[USER_ALICE].pubkey(),
            &lp_token_mint_pda,
        )
        .0;

        let alice_lp_token_balance =
            utils::get_token_account_balance(&mut program_test_ctx, alice_lp_token).await;

        // Alice: Remove 100% of provided liquidity (1k USDC less fees)
        instructions::test_remove_liquidity(
            &mut program_test_ctx,
            &keypairs[USER_ALICE],
            &keypairs[PAYER],
            &pool_pda,
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
        let current_time = utils::get_current_unix_timestamp(&mut program_test_ctx).await;

        // Alice: vest 2 token, unlock period from now to in 7 days
        instructions::test_add_vest(
            &mut program_test_ctx,
            &keypairs[MULTISIG_MEMBER_A],
            &keypairs[PAYER],
            &keypairs[USER_ALICE],
            &governance_realm_pda,
            &AddVestParams {
                amount: utils::scale(2, Cortex::LM_DECIMALS),
                unlock_start_timestamp: current_time,
                unlock_end_timestamp: utils::days_in_seconds(7) + current_time,
            },
            multisig_signers,
        )
        .await
        .unwrap();

        // warp to have tokens to claim
        utils::warp_forward(&mut program_test_ctx, utils::days_in_seconds(7)).await;

        // Alice: claim vest
        instructions::test_claim_vest(
            &mut program_test_ctx,
            &keypairs[PAYER],
            &keypairs[USER_ALICE],
            &governance_realm_pda,
        )
        .await
        .unwrap();
    }

    // Stake
    {
        // Alice: add stake LM token
        instructions::test_add_stake(
            &mut program_test_ctx,
            &keypairs[USER_ALICE],
            &keypairs[PAYER],
            AddStakeParams {
                amount: scale(1, Cortex::LM_DECIMALS),
            },
            &cortex_stake_reward_mint,
            &governance_realm_pda,
        )
        .await
        .unwrap();

        // Alice: remove stake LM token
        instructions::test_remove_stake(
            &mut program_test_ctx,
            &keypairs[USER_ALICE],
            &keypairs[PAYER],
            RemoveStakeParams {
                amount: scale(1, Cortex::LM_DECIMALS),
            },
            &cortex_stake_reward_mint,
            &governance_realm_pda,
        )
        .await
        .unwrap();

        // Alice: test claim stake (no stake account, none)
        instructions::test_claim_stake(
            &mut program_test_ctx,
            &keypairs[USER_ALICE],
            &keypairs[USER_ALICE],
            &keypairs[PAYER],
            &governance_realm_pda,
            &cortex_stake_reward_mint,
        )
        .await
        .unwrap();

        // resolution of the round
        // warps to when the round is resolvable
        utils::warp_forward(
            &mut program_test_ctx,
            StakingRound::ROUND_MIN_DURATION_SECONDS,
        )
        .await;

        instructions::test_resolve_staking_round(
            &mut program_test_ctx,
            &keypairs[USER_ALICE],
            &keypairs[USER_ALICE],
            &keypairs[PAYER],
            &cortex_stake_reward_mint,
        )
        .await
        .unwrap();
    }
}

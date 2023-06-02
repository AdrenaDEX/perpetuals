use {
    crate::{adapters, instructions, utils},
    maplit::hashmap,
    perpetuals::{instructions::AddVestParams, state::cortex::Cortex},
};

const USDC_DECIMALS: u8 = 6;

pub async fn vote() {
    let test = utils::Test::new(
        vec![utils::UserParam {
            name: "alice",
            token_balances: hashmap! {},
        }],
        vec![utils::MintParam {
            name: "usdc",
            decimals: USDC_DECIMALS,
        }],
        vec!["admin_a", "admin_b", "admin_c"],
        // mint for the payouts of the LM token staking (ADX staking)
        "usdc".to_string(),
        6,
        "ADRENA",
        "main_pool",
        vec![utils::NamedSetupCustodyWithLiquidityParams {
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
            liquidity_amount: utils::scale(0, USDC_DECIMALS),
            payer_user_name: "alice",
        }],
    )
    .await;

    let alice = test.get_user_keypair_by_name("alice");

    let admin_a = test.get_multisig_member_keypair_by_name("admin_a");

    let multisig_signers = test.get_multisig_signers();

    // Alice: vest 1m token, unlock period from now to in 7 days
    let current_time =
        utils::get_current_unix_timestamp(&mut test.program_test_ctx.borrow_mut()).await;

    let alice_vest_pda = instructions::test_add_vest(
        &mut test.program_test_ctx.borrow_mut(),
        admin_a,
        &test.payer_keypair,
        alice,
        &test.governance_realm_pda,
        &AddVestParams {
            amount: utils::scale(1_000_000, Cortex::LM_DECIMALS),
            unlock_start_timestamp: current_time,
            unlock_end_timestamp: utils::days_in_seconds(7) + current_time,
        },
        &multisig_signers,
    )
    .await
    .unwrap()
    .0;

    let governance_pda = adapters::spl_governance::create_governance(
        &mut test.program_test_ctx.borrow_mut(),
        &alice_vest_pda,
        alice,
        &test.payer_keypair,
        &test.governance_realm_pda,
        &test.lm_token_mint,
        None,
        None,
        None,
        None,
        None,
    )
    .await
    .unwrap()
    .0;

    let proposal_pda = adapters::spl_governance::create_proposal(
        &mut test.program_test_ctx.borrow_mut(),
        &test.payer_keypair,
        "Test Proposal".to_string(),
        "Description".to_string(),
        &test.governance_realm_pda,
        &governance_pda,
        &test.lm_token_mint,
        &alice_vest_pda,
        alice,
    )
    .await
    .unwrap();

    adapters::spl_governance::cast_vote(
        &mut test.program_test_ctx.borrow_mut(),
        &test.payer_keypair,
        &test.governance_realm_pda,
        &governance_pda,
        &proposal_pda,
        &test.lm_token_mint,
        &alice_vest_pda,
        &alice_vest_pda,
        alice,
        true,
    )
    .await
    .unwrap();

    adapters::spl_governance::cancel_proposal(
        &mut test.program_test_ctx.borrow_mut(),
        &test.payer_keypair,
        &test.governance_realm_pda,
        &governance_pda,
        &proposal_pda,
        &test.lm_token_mint,
        &alice_vest_pda,
        alice,
    )
    .await
    .unwrap();

    adapters::spl_governance::relinquish_vote(
        &mut test.program_test_ctx.borrow_mut(),
        &test.payer_keypair,
        &test.governance_realm_pda,
        &governance_pda,
        &proposal_pda,
        &test.lm_token_mint,
        &alice_vest_pda,
        alice,
    )
    .await
    .unwrap();

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

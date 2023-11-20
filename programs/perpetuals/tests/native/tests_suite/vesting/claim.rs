use {
    crate::{
        test_instructions,
        utils::{self, pda, scale, warp_forward},
    },
    maplit::hashmap,
    perpetuals::{
        instructions::{AddVestParams, BucketName},
        state::cortex::Cortex,
    },
};

const USDC_DECIMALS: u8 = 6;

pub async fn claim() {
    let core_contributor_bucket_starting_allocation = 1_000_000;
    let test_setup = utils::TestSetup::new(
        vec![utils::UserParam {
            name: "alice",
            token_balances: hashmap! {},
        }],
        vec![utils::MintParam {
            name: "usdc",
            decimals: USDC_DECIMALS,
        }],
        vec!["admin_a", "admin_b", "admin_c"],
        "usdc",
        "usdc",
        6,
        "ADRENA",
        "main_pool",
        vec![utils::SetupCustodyWithLiquidityParams {
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
            liquidity_amount: utils::scale(0, USDC_DECIMALS),
            payer_user_name: "alice",
        }],
        utils::scale(
            core_contributor_bucket_starting_allocation,
            Cortex::LM_DECIMALS,
        ),
        utils::scale(2_000_000, Cortex::LM_DECIMALS),
        utils::scale(3_000_000, Cortex::LM_DECIMALS),
        utils::scale(4_000_000, Cortex::LM_DECIMALS),
    )
    .await;

    let alice = test_setup.get_user_keypair_by_name("alice");

    let admin_a = test_setup.get_multisig_member_keypair_by_name("admin_a");

    let multisig_signers = test_setup.get_multisig_signers();

    // Alice: vest 250k token, unlock period from now to in 7 days
    let vest_amount = 250_000;
    let current_time = utils::get_current_unix_timestamp(&test_setup.program_test_ctx).await;
    let (cortex_pda, _) = pda::get_cortex_pda();

    let cortex_before =
        utils::get_account::<Cortex>(&test_setup.program_test_ctx, cortex_pda).await;

    test_instructions::add_vest(
        &test_setup.program_test_ctx,
        admin_a,
        &test_setup.payer_keypair,
        alice,
        &test_setup.governance_realm_pda,
        &AddVestParams {
            amount: utils::scale(vest_amount, Cortex::LM_DECIMALS),
            origin_bucket: BucketName::CoreContributor,
            unlock_start_timestamp: current_time,
            unlock_end_timestamp: utils::days_in_seconds(7) + current_time,
        },
        &multisig_signers,
    )
    .await
    .unwrap()
    .0;

    // Check state after vest creation, before claim
    {
        let cortex_after =
            utils::get_account::<Cortex>(&test_setup.program_test_ctx, cortex_pda).await;

        // Nothing changes yet regarding the amount of token minted
        assert_eq!(
            cortex_after
                .get_token_amount_left_in_bucket(BucketName::CoreContributor)
                .unwrap(),
            cortex_before
                .get_token_amount_left_in_bucket(BucketName::CoreContributor)
                .unwrap()
        );

        // Check that the reserved token are reserved
        assert_eq!(
            cortex_after
                .get_non_reserved_token_amount_left_in_bucket(BucketName::CoreContributor)
                .unwrap(),
            cortex_after
                .get_token_amount_left_in_bucket(BucketName::CoreContributor)
                .unwrap()
                .checked_sub(scale(vest_amount, Cortex::LM_DECIMALS))
                .unwrap()
        );
    }

    // move 7 days forward
    warp_forward(&test_setup.program_test_ctx, 7 * 24 * 60 * 60 + 1).await;

    // Alice: claim vest
    test_instructions::claim_vest(
        &test_setup.program_test_ctx,
        &test_setup.payer_keypair,
        alice,
        &test_setup.governance_realm_pda,
    )
    .await
    .unwrap();

    // Verify the internal vest account after claim
    {
        let cortex_after =
            utils::get_account::<Cortex>(&test_setup.program_test_ctx, cortex_pda).await;

        assert_eq!(
            cortex_after
                .get_token_amount_left_in_bucket(BucketName::CoreContributor)
                .unwrap(),
            cortex_before
                .get_token_amount_left_in_bucket(BucketName::CoreContributor)
                .unwrap()
                .checked_sub(scale(vest_amount, Cortex::LM_DECIMALS))
                .unwrap()
        );
        // The vest is claimed, so no more reserved tokens
        assert_eq!(
            cortex_after
                .get_non_reserved_token_amount_left_in_bucket(BucketName::CoreContributor)
                .unwrap(),
            cortex_after
                .get_token_amount_left_in_bucket(BucketName::CoreContributor)
                .unwrap()
        );
        // Verify that other buckets didn't move
        {
            assert_eq!(
                cortex_after
                    .get_token_amount_left_in_bucket(BucketName::DaoTreasury)
                    .unwrap(),
                cortex_before
                    .get_token_amount_left_in_bucket(BucketName::DaoTreasury)
                    .unwrap()
            );
            assert_eq!(
                cortex_after
                    .get_token_amount_left_in_bucket(BucketName::Ecosystem)
                    .unwrap(),
                cortex_before
                    .get_token_amount_left_in_bucket(BucketName::Ecosystem)
                    .unwrap()
            );
            assert_eq!(
                cortex_after
                    .get_token_amount_left_in_bucket(BucketName::PoL)
                    .unwrap(),
                cortex_before
                    .get_token_amount_left_in_bucket(BucketName::PoL)
                    .unwrap()
            );
            assert_eq!(
                cortex_after
                    .get_non_reserved_token_amount_left_in_bucket(BucketName::DaoTreasury)
                    .unwrap(),
                cortex_before
                    .get_non_reserved_token_amount_left_in_bucket(BucketName::DaoTreasury)
                    .unwrap()
            );
            assert_eq!(
                cortex_after
                    .get_non_reserved_token_amount_left_in_bucket(BucketName::Ecosystem)
                    .unwrap(),
                cortex_before
                    .get_non_reserved_token_amount_left_in_bucket(BucketName::Ecosystem)
                    .unwrap()
            );
            assert_eq!(
                cortex_after
                    .get_non_reserved_token_amount_left_in_bucket(BucketName::PoL)
                    .unwrap(),
                cortex_before
                    .get_non_reserved_token_amount_left_in_bucket(BucketName::PoL)
                    .unwrap()
            );
        }
    }
}

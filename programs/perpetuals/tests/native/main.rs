pub mod adapters;
pub mod instructions;
pub mod tests_suite;
pub mod utils;

/*

  // Prep work: Vest and claim (to get some governance tokens)
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
*/

#[tokio::test]
pub async fn test_integration() {
    tests_suite::basic_interactions().await;

    /*tests_suite::swap::insuffisient_fund().await;

    tests_suite::liquidity::fixed_fees().await;
    tests_suite::liquidity::insuffisient_fund().await;
    tests_suite::liquidity::min_max_ratio().await;

    tests_suite::position::min_max_leverage().await;
    // tests_suite::position::liquidate_position().await;
    // tests_suite::position::max_user_profit().await;

    tests_suite::staking::test_staking_rewards_from_swap().await;
    tests_suite::staking::test_staking_rewards_from_open_and_close_position().await;
    tests_suite::staking::test_staking_rewards_from_add_and_remove_liquidity().await;
    tests_suite::staking::test_bounty_no_rewards().await;
    tests_suite::staking::test_bounty_phase_one().await;*/
}

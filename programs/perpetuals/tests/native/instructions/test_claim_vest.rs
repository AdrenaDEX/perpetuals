use bonfida_test_utils::ProgramTestContextExt;

use {
    crate::utils::{self, pda},
    anchor_lang::ToAccountMetas,
    perpetuals::state::{cortex::Cortex, vest::Vest},
    solana_program_test::BanksClientError,
    solana_program_test::ProgramTestContext,
    solana_sdk::signer::{keypair::Keypair, Signer},
};

pub async fn test_claim_vest(
    program_test_ctx: &mut ProgramTestContext,
    payer: &Keypair,
    owner: &Keypair,
) -> std::result::Result<(), BanksClientError> {
    // ==== WHEN ==============================================================
    let transfer_authority_pda = pda::get_transfer_authority_pda().0;
    let perpetuals_pda = pda::get_perpetuals_pda().0;
    let cortex_pda = pda::get_cortex_pda().0;
    let (vest_pda, vest_bump) = pda::get_vest_pda(owner.pubkey());
    let (lm_token_mint_pda, _) = pda::get_lm_token_mint_pda();

    let lm_token_account_address =
        utils::find_associated_token_account(&owner.pubkey(), &lm_token_mint_pda).0;

    // Save account state before tx execution
    let owner_lm_token_account_before = program_test_ctx
        .get_token_account(lm_token_account_address)
        .await
        .unwrap();

    utils::create_and_execute_perpetuals_ix(
        program_test_ctx,
        perpetuals::accounts::ClaimVest {
            owner: owner.pubkey(),
            receiving_account: lm_token_account_address,
            transfer_authority: transfer_authority_pda,
            cortex: cortex_pda,
            perpetuals: perpetuals_pda,
            vest: vest_pda,
            lm_token_mint: lm_token_mint_pda,
            system_program: anchor_lang::system_program::ID,
            token_program: anchor_spl::token::ID,
            rent: solana_program::sysvar::rent::ID,
        }
        .to_account_metas(None),
        {}, // wat do
        Some(&payer.pubkey()),
        &[payer, owner],
    )
    .await?;

    // ==== THEN ==============================================================
    let vest_account = utils::get_account::<Vest>(program_test_ctx, vest_pda).await;

    assert_eq!(vest_account.owner, owner.pubkey());
    assert_eq!(vest_account.bump, vest_bump);

    // TODO: check tokens are in user account

    let cortex_account = utils::get_account::<Cortex>(program_test_ctx, cortex_pda).await;

    assert_eq!(*cortex_account.vests.last().unwrap(), vest_pda);

    Ok(())
}

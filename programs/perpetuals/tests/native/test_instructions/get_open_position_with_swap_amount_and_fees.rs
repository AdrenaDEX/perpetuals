use {
    crate::utils::{self, pda},
    anchor_lang::{prelude::Pubkey, ToAccountMetas},
    perpetuals::{
        instructions::GetOpenPositionWithSwapAmountAndFeesParams,
        state::{custody::Custody, perpetuals::OpenPositionWithSwapAmountAndFees},
    },
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::keypair::Keypair,
    tokio::sync::RwLock,
};

#[allow(clippy::too_many_arguments)]
pub async fn get_open_position_with_swap_amount_and_fees(
    program_test_ctx: &RwLock<ProgramTestContext>,
    payer: &Keypair,
    pool_pda: &Pubkey,
    // Mint sent by the user
    receiving_custody_token_mint: &Pubkey,
    // Mint used as collateral
    collateral_custody_token_mint: &Pubkey,
    // Targetted principal
    principal_custody_token_mint: &Pubkey,
    params: GetOpenPositionWithSwapAmountAndFeesParams,
) -> std::result::Result<OpenPositionWithSwapAmountAndFees, BanksClientError> {
    // ==== WHEN ==============================================================
    // Prepare PDA and addresses
    let perpetuals_pda = pda::get_perpetuals_pda().0;

    let receiving_custody_pda = pda::get_custody_pda(pool_pda, receiving_custody_token_mint).0;
    let collateral_custody_pda = pda::get_custody_pda(pool_pda, collateral_custody_token_mint).0;
    let principal_custody_pda = pda::get_custody_pda(pool_pda, principal_custody_token_mint).0;

    let receiving_custody_account =
        utils::get_account::<Custody>(program_test_ctx, receiving_custody_pda).await;
    let receiving_custody_oracle_account_address = receiving_custody_account.oracle.oracle_account;

    let collateral_custody_account =
        utils::get_account::<Custody>(program_test_ctx, collateral_custody_pda).await;
    let collateral_custody_oracle_account_address =
        collateral_custody_account.oracle.oracle_account;

    let principal_custody_account =
        utils::get_account::<Custody>(program_test_ctx, principal_custody_pda).await;
    let principal_custody_oracle_account_address = principal_custody_account.oracle.oracle_account;

    let result: OpenPositionWithSwapAmountAndFees = utils::create_and_simulate_perpetuals_view_ix(
        program_test_ctx,
        perpetuals::accounts::GetOpenPositionWithSwapAmountAndFees {
            perpetuals: perpetuals_pda,
            pool: *pool_pda,
            receiving_custody: receiving_custody_pda,
            receiving_custody_oracle_account: receiving_custody_oracle_account_address,
            collateral_custody: collateral_custody_pda,
            collateral_custody_oracle_account: collateral_custody_oracle_account_address,
            principal_custody: principal_custody_pda,
            principal_custody_oracle_account: principal_custody_oracle_account_address,
            perpetuals_program: perpetuals::ID,
        }
        .to_account_metas(None),
        perpetuals::instruction::GetOpenPositionWithSwapAmountAndFees { params },
        payer,
    )
    .await?;

    // ==== THEN ==============================================================
    Ok(result)
}

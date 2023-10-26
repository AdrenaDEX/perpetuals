use {
    super::get_update_pool_ix,
    crate::utils::{self, pda},
    anchor_lang::{prelude::Pubkey, ToAccountMetas},
    perpetuals::{
        instructions::OpenPositionWithSwapParams,
        state::{custody::Custody, position::Side, staking::Staking},
    },
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
    tokio::sync::RwLock,
};

pub async fn open_position_with_swap(
    program_test_ctx: &RwLock<ProgramTestContext>,
    owner: &Keypair,
    payer: &Keypair,
    pool_pda: &Pubkey,
    // Token provided from the user as collateral for the position
    // (not the ultimate collateral, but the one provided by the user initially)
    user_collateral_token_mint: &Pubkey,
    // Token targeted for the position
    principal_token_mint: &Pubkey,
    params: OpenPositionWithSwapParams,

    // In case the position is a short, provide the stable token to serve as collateral
    // can be the same as collateral_token_mint
    stable_token_mint: Option<&Pubkey>,
) -> std::result::Result<(Pubkey, u8), BanksClientError> {
    // ==== WHEN ==============================================================

    // Prepare PDA and addresses
    let transfer_authority_pda = pda::get_transfer_authority_pda().0;
    let perpetuals_pda = pda::get_perpetuals_pda().0;

    //
    // Infos about receiving custody (the token received by the ix from the user initially)
    //
    // i.e User short ETH with BTC. receiving custody is ETH.
    //
    let receiving_custody_pda = pda::get_custody_pda(pool_pda, user_collateral_token_mint).0;
    let receiving_custody_token_account_pda =
        pda::get_custody_token_account_pda(pool_pda, user_collateral_token_mint).0;

    let receiving_custody_account =
        utils::get_account::<Custody>(program_test_ctx, receiving_custody_pda).await;
    let receiving_custody_oracle_account_address = receiving_custody_account.oracle.oracle_account;

    //
    // Infos about principal custody (the token targetted by the position)
    //
    // i.e User short ETH with BTC. principal custody is BTC.
    //
    let principal_custody_pda = pda::get_custody_pda(pool_pda, principal_token_mint).0;
    let principal_custody_token_account_pda =
        pda::get_custody_token_account_pda(pool_pda, principal_token_mint).0;

    let principal_custody_account =
        utils::get_account::<Custody>(program_test_ctx, principal_custody_pda).await;
    let principal_custody_oracle_account_address = principal_custody_account.oracle.oracle_account;

    //
    // Infos about collateral custody (the token to swap receiving token for to provide as collateral for the position)
    //
    // i.e User short ETH with BTC. receiving custody is USDC (can only short with stable)
    //
    let (
        collateral_custody_pda,
        collateral_custody_token_account_pda,
        collateral_custody_oracle_account_address,
        collateral_account_address,
    ) = if params.side == Side::Short {
        // Must be provided when short
        let stable_token_mint = stable_token_mint.unwrap();

        let collateral_custody_pda = pda::get_custody_pda(pool_pda, stable_token_mint).0;
        let collateral_custody_token_account_pda =
            pda::get_custody_token_account_pda(pool_pda, stable_token_mint).0;

        let collateral_custody_account =
            utils::get_account::<Custody>(program_test_ctx, collateral_custody_pda).await;
        let collateral_custody_oracle_account_address =
            collateral_custody_account.oracle.oracle_account;

        let collateral_account_address =
            utils::find_associated_token_account(&owner.pubkey(), stable_token_mint).0;

        (
            collateral_custody_pda,
            collateral_custody_token_account_pda,
            collateral_custody_oracle_account_address,
            collateral_account_address,
        )
    } else {
        // When longing, the collateral and principal must be the same token

        let collateral_account_address =
            utils::find_associated_token_account(&owner.pubkey(), principal_token_mint).0;
        (
            principal_custody_pda,
            principal_custody_token_account_pda,
            principal_custody_oracle_account_address,
            collateral_account_address,
        )
    };
    //
    //

    let (position_pda, position_bump) = pda::get_position_pda(
        &owner.pubkey(),
        pool_pda,
        &principal_custody_pda,
        params.side,
    );

    let cortex_pda = pda::get_cortex_pda().0;
    let lm_token_mint_pda = pda::get_lm_token_mint_pda().0;
    let lp_token_mint_pda = pda::get_lp_token_mint_pda(pool_pda).0;
    let lm_staking_pda = pda::get_staking_pda(&lm_token_mint_pda).0;
    let lp_staking_pda = pda::get_staking_pda(&lp_token_mint_pda).0;

    let funding_account_address =
        utils::find_associated_token_account(&owner.pubkey(), user_collateral_token_mint).0;
    let lm_token_account_address =
        utils::find_associated_token_account(&owner.pubkey(), &lm_token_mint_pda).0;

    let lm_staking_reward_token_vault_pda =
        pda::get_staking_reward_token_vault_pda(&lm_staking_pda).0;

    let lp_staking_reward_token_vault_pda =
        pda::get_staking_reward_token_vault_pda(&lp_staking_pda).0;

    let lm_staking_account = utils::get_account::<Staking>(program_test_ctx, lm_staking_pda).await;

    let srt_custody_pda = pda::get_custody_pda(pool_pda, &lm_staking_account.reward_token_mint).0;
    let srt_custody_token_account_pda =
        pda::get_custody_token_account_pda(pool_pda, &lm_staking_account.reward_token_mint).0;
    let srt_custody_account =
        utils::get_account::<Custody>(program_test_ctx, srt_custody_pda).await;
    let srt_custody_oracle_account_address = srt_custody_account.oracle.oracle_account;

    utils::create_and_execute_perpetuals_ix(
        program_test_ctx,
        perpetuals::accounts::OpenPositionWithSwap {
            owner: owner.pubkey(),
            payer: payer.pubkey(),
            funding_account: funding_account_address,
            collateral_account: collateral_account_address,
            lm_token_account: lm_token_account_address,

            //
            receiving_custody: receiving_custody_pda,
            receiving_custody_oracle_account: receiving_custody_oracle_account_address,
            receiving_custody_token_account: receiving_custody_token_account_pda,
            //
            collateral_custody: collateral_custody_pda,
            collateral_custody_oracle_account: collateral_custody_oracle_account_address,
            collateral_custody_token_account: collateral_custody_token_account_pda,
            //
            principal_custody: principal_custody_pda,
            principal_custody_oracle_account: principal_custody_oracle_account_address,
            principal_custody_token_account: principal_custody_token_account_pda,
            //
            transfer_authority: transfer_authority_pda,
            cortex: cortex_pda,
            perpetuals: perpetuals_pda,
            lm_staking: lm_staking_pda,
            lp_staking: lp_staking_pda,
            pool: *pool_pda,
            position: position_pda,
            staking_reward_token_custody: srt_custody_pda,
            staking_reward_token_custody_oracle_account: srt_custody_oracle_account_address,
            staking_reward_token_custody_token_account: srt_custody_token_account_pda,
            lm_staking_reward_token_vault: lm_staking_reward_token_vault_pda, // the stake reward vault
            lp_staking_reward_token_vault: lp_staking_reward_token_vault_pda,
            lm_token_mint: lm_token_mint_pda,
            lp_token_mint: lp_token_mint_pda,
            staking_reward_token_mint: lm_staking_account.reward_token_mint,
            system_program: anchor_lang::system_program::ID,
            token_program: anchor_spl::token::ID,
            perpetuals_program: perpetuals::ID,
        }
        .to_account_metas(None),
        perpetuals::instruction::OpenPositionWithSwap { params },
        Some(&payer.pubkey()),
        &[owner, payer],
        Some(get_update_pool_ix(program_test_ctx, payer, pool_pda).await?),
        None,
    )
    .await?;

    // ==== THEN ==============================================================

    Ok((position_pda, position_bump))
}

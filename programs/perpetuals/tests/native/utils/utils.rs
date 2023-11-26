use {
    crate::test_instructions,
    anchor_lang::{prelude::*, InstructionData},
    anchor_spl::token::spl_token,
    bonfida_test_utils::ProgramTestContextExt,
    borsh::BorshDeserialize,
    perpetuals::{
        adapters::SplGovernanceV3Adapter,
        instructions::SetCustodyConfigParams,
        math,
        state::{
            custody::Custody,
            oracle::CustomOracle,
            perpetuals::Perpetuals,
            pool::{AumCalcMode, Pool, TokenRatios},
        },
    },
    solana_program::{
        borsh0_10::try_from_slice_unchecked, clock::DEFAULT_MS_PER_SLOT,
        epoch_schedule::DEFAULT_SLOTS_PER_EPOCH, program_pack::Pack, stake_history::Epoch,
    },
    solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext},
    solana_sdk::{
        account, compute_budget::ComputeBudgetInstruction, signature::Keypair, signer::Signer,
        signers::Signers,
    },
    std::ops::{Div, Mul},
    tokio::sync::RwLock,
};

pub const ANCHOR_DISCRIMINATOR_SIZE: usize = 8;

#[macro_export]
macro_rules! assert_unchanged {
    ($before:expr, $after:expr) => {
        assert_eq!(
            $before, $after,
            "Values are not the same: {:?} != {:?}",
            $before, $after
        );
    };
}

pub fn create_and_fund_account(address: &Pubkey, program_test: &mut ProgramTest) {
    program_test.add_account(
        *address,
        account::Account {
            lamports: 1_000_000_000,
            ..account::Account::default()
        },
    );
}

pub fn find_associated_token_account(owner: &Pubkey, mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            owner.as_ref(),
            anchor_spl::token::ID.as_ref(),
            mint.as_ref(),
        ],
        &anchor_spl::associated_token::ID,
    )
}

pub fn copy_keypair(keypair: &Keypair) -> Keypair {
    Keypair::from_bytes(&keypair.to_bytes()).unwrap()
}

pub fn days_in_seconds(nb_days: u32) -> i64 {
    (nb_days as i64) * (3_600 * 24)
}

pub async fn get_token_account(
    program_test_ctx: &RwLock<ProgramTestContext>,
    key: Pubkey,
) -> spl_token::state::Account {
    let mut ctx = program_test_ctx.write().await;
    let banks_client = &mut ctx.banks_client;

    let raw_account = banks_client.get_account(key).await.unwrap().unwrap();

    spl_token::state::Account::unpack(&raw_account.data).unwrap()
}

pub async fn get_token_account_balance(
    program_test_ctx: &RwLock<ProgramTestContext>,
    key: Pubkey,
) -> u64 {
    get_token_account(program_test_ctx, key).await.amount
}

pub async fn get_borsh_account<T: BorshDeserialize>(
    program_test_ctx: &RwLock<ProgramTestContext>,
    address: &Pubkey,
) -> T {
    let mut ctx = program_test_ctx.write().await;
    let banks_client = &mut ctx.banks_client;

    banks_client
        .get_account(*address)
        .await
        .unwrap()
        .map(|a| try_from_slice_unchecked(&a.data).unwrap())
        .unwrap_or_else(|| panic!("GET-TEST-ACCOUNT-ERROR: Account {} not found", address))
}

pub async fn try_get_account<T: anchor_lang::AccountDeserialize>(
    program_test_ctx: &RwLock<ProgramTestContext>,
    key: Pubkey,
) -> Option<T> {
    let mut ctx = program_test_ctx.write().await;
    let banks_client = &mut ctx.banks_client;

    let account = banks_client.get_account(key).await.unwrap();

    // an account with 0 lamport can be considered inexistant in the context of our tests
    // on mainnet, someone might just send lamports to the right place but doesn't matter here
    return if let Some(a) = account {
        Some(T::try_deserialize(&mut a.data.as_slice()).unwrap())
    } else {
        None
    };
}

pub async fn get_account<T: anchor_lang::AccountDeserialize>(
    program_test_ctx: &RwLock<ProgramTestContext>,
    key: Pubkey,
) -> T {
    let mut ctx = program_test_ctx.write().await;
    let banks_client = &mut ctx.banks_client;

    let account = banks_client.get_account(key).await.unwrap().unwrap();

    T::try_deserialize(&mut account.data.as_slice()).unwrap()
}

pub async fn get_current_unix_timestamp(program_test_ctx: &RwLock<ProgramTestContext>) -> i64 {
    let mut ctx = program_test_ctx.write().await;
    let banks_client = &mut ctx.banks_client;

    banks_client
        .get_sysvar::<solana_program::sysvar::clock::Clock>()
        .await
        .unwrap()
        .unix_timestamp
}

pub async fn initialize_token_account(
    program_test_ctx: &RwLock<ProgramTestContext>,
    mint: &Pubkey,
    owner: &Pubkey,
) -> Pubkey {
    let mut ctx = program_test_ctx.write().await;

    ctx.initialize_token_accounts(*mint, &[*owner])
        .await
        .unwrap()[0]
}

pub async fn initialize_and_fund_token_account(
    program_test_ctx: &RwLock<ProgramTestContext>,
    mint: &Pubkey,
    owner: &Pubkey,
    mint_authority: &Keypair,
    amount: u64,
) -> Pubkey {
    let token_account_address = initialize_token_account(program_test_ctx, mint, owner).await;

    mint_tokens(
        program_test_ctx,
        mint_authority,
        mint,
        &token_account_address,
        amount,
    )
    .await;

    token_account_address
}

pub async fn mint_tokens(
    program_test_ctx: &RwLock<ProgramTestContext>,
    mint_authority: &Keypair,
    mint: &Pubkey,
    token_account: &Pubkey,
    amount: u64,
) {
    let mut ctx = program_test_ctx.write().await;

    ctx.mint_tokens(mint_authority, mint, token_account, amount)
        .await
        .unwrap();
}

// Doesn't check if you go before epoch 0 when passing negative amounts, be wary
pub async fn warp_forward(ctx: &RwLock<ProgramTestContext>, seconds: i64) {
    let mut ctx = ctx.write().await;

    let clock_sysvar: Clock = ctx.banks_client.get_sysvar().await.unwrap();
    println!(
        "Original Time: epoch = {}, timestamp = {}",
        clock_sysvar.epoch, clock_sysvar.unix_timestamp
    );
    let mut new_clock = clock_sysvar.clone();
    new_clock.unix_timestamp += seconds;

    let seconds_since_epoch_start = new_clock.unix_timestamp - clock_sysvar.epoch_start_timestamp;
    let ms_since_epoch_start = seconds_since_epoch_start * 1_000;
    let slots_since_epoch_start = ms_since_epoch_start / DEFAULT_MS_PER_SLOT as i64;
    let epochs_since_epoch_start = slots_since_epoch_start / DEFAULT_SLOTS_PER_EPOCH as i64;
    new_clock.epoch = (new_clock.epoch as i64 + epochs_since_epoch_start) as u64;

    ctx.set_sysvar(&new_clock);
    let clock_sysvar: Clock = ctx.banks_client.get_sysvar().await.unwrap();
    println!(
        "New Time: epoch = {}, timestamp = {}",
        clock_sysvar.epoch, clock_sysvar.unix_timestamp
    );

    let blockhash = ctx.banks_client.get_latest_blockhash().await.unwrap();

    ctx.last_blockhash = blockhash;
}

pub async fn create_and_fund_multiple_accounts(
    program_test: &mut ProgramTest,
    number: usize,
) -> Vec<Keypair> {
    let mut keypairs = Vec::new();

    for _ in 0..number {
        keypairs.push(Keypair::new());
    }

    keypairs
        .iter()
        .for_each(|k| create_and_fund_account(&k.pubkey(), program_test));

    keypairs
}

pub async fn create_and_simulate_perpetuals_view_ix<T: InstructionData, U: BorshDeserialize>(
    program_test_ctx: &RwLock<ProgramTestContext>,
    accounts_meta: Vec<AccountMeta>,
    args: T,
    payer: &Keypair,
) -> std::result::Result<U, BanksClientError> {
    let ix = solana_sdk::instruction::Instruction {
        program_id: perpetuals::id(),
        accounts: accounts_meta,
        data: args.data(),
    };

    let payer_pubkey = payer.pubkey();

    let mut ctx = program_test_ctx.write().await;
    let last_blockhash = ctx.last_blockhash;
    let banks_client = &mut ctx.banks_client;

    let tx = solana_sdk::transaction::Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer_pubkey),
        &[payer],
        last_blockhash,
    );

    let result = banks_client.simulate_transaction(tx).await;

    if result.is_err() {
        return Err(result.err().unwrap());
    }

    // Extract the returned data
    let mut return_data: Vec<u8> = result
        .unwrap()
        .simulation_details
        .unwrap()
        .return_data
        .unwrap()
        .data;

    let result_expected_len = std::mem::size_of::<U>();

    // Returned data doesn't contains leading zeros, need to re-add them before deserialization
    while return_data.len() < result_expected_len {
        return_data.push(0u8);
    }

    Ok(U::try_from_slice(return_data.as_slice()).unwrap())
}

pub async fn create_and_execute_perpetuals_ix<T: InstructionData, U: Signers>(
    program_test_ctx: &RwLock<ProgramTestContext>,
    accounts_meta: Vec<AccountMeta>,
    args: T,
    payer: Option<&Pubkey>,
    signing_keypairs: &U,
    pre_ix: Option<solana_sdk::instruction::Instruction>,
    post_ix: Option<solana_sdk::instruction::Instruction>,
) -> std::result::Result<(), BanksClientError> {
    let ix = solana_sdk::instruction::Instruction {
        program_id: perpetuals::id(),
        accounts: accounts_meta,
        data: args.data(),
    };

    let mut ctx = program_test_ctx.write().await;
    let last_blockhash = ctx.last_blockhash;
    let banks_client = &mut ctx.banks_client;

    let mut instructions: Vec<solana_sdk::instruction::Instruction> = Vec::new();

    instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(
        2_000_000u32,
    ));

    if pre_ix.is_some() {
        instructions.push(pre_ix.unwrap());
    }

    instructions.push(ix);

    if post_ix.is_some() {
        instructions.push(post_ix.unwrap());
    }

    let tx = solana_sdk::transaction::Transaction::new_signed_with_payer(
        instructions.as_slice(),
        payer,
        signing_keypairs,
        last_blockhash,
    );

    let result = banks_client.process_transaction(tx).await;

    if result.is_err() {
        return Err(result.err().unwrap());
    }

    Ok(())
}

pub async fn create_and_execute_spl_governance_ix<U: Signers>(
    program_test_ctx: &RwLock<ProgramTestContext>,
    accounts_meta: Vec<AccountMeta>,
    data: Vec<u8>,
    payer: Option<&Pubkey>,
    signing_keypairs: &U,
) -> std::result::Result<(), BanksClientError> {
    let ix = solana_sdk::instruction::Instruction {
        program_id: SplGovernanceV3Adapter::id(),
        accounts: accounts_meta,
        data,
    };

    let mut ctx = program_test_ctx.write().await;
    let last_blockhash = ctx.last_blockhash;
    let banks_client = &mut ctx.banks_client;

    let tx = solana_sdk::transaction::Transaction::new_signed_with_payer(
        &[ix],
        payer,
        signing_keypairs,
        last_blockhash,
    );

    let result = banks_client.process_transaction(tx).await;

    if result.is_err() {
        return Err(result.err().unwrap());
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn set_custody_ratios(
    program_test_ctx: &RwLock<ProgramTestContext>,
    custody_admin: &Keypair,
    payer: &Keypair,
    custody_pda: &Pubkey,
    ratios: Vec<TokenRatios>,
    multisig_signers: &[&Keypair],
) {
    let custody_account = get_account::<Custody>(program_test_ctx, *custody_pda).await;

    test_instructions::set_custody_config(
        program_test_ctx,
        custody_admin,
        payer,
        &custody_account.pool,
        custody_pda,
        SetCustodyConfigParams {
            is_stable: custody_account.is_stable,
            oracle: custody_account.oracle,
            pricing: custody_account.pricing,
            permissions: custody_account.permissions,
            fees: custody_account.fees,
            borrow_rate: custody_account.borrow_rate,
            ratios,
        },
        multisig_signers,
    )
    .await
    .unwrap();
}

#[derive(Clone, Copy)]
pub struct SetupCustodyInfo {
    pub custom_oracle_pda: Pubkey,
    pub custody_pda: Pubkey,
}

pub fn scale(amount: u64, decimals: u8) -> u64 {
    math::checked_mul(amount, 10u64.pow(decimals as u32)).unwrap()
}

pub fn scale_f64(amount: f64, decimals: u8) -> u64 {
    math::checked_as_u64(
        math::checked_float_mul(amount, 10u64.pow(decimals as u32) as f64).unwrap(),
    )
    .unwrap()
}

pub fn ratio_from_percentage(percentage: f64) -> u64 {
    (Perpetuals::BPS_POWER as f64)
        .mul(percentage)
        .div(100_f64)
        .floor() as u64
}

pub async fn initialize_users_token_accounts(
    program_test_ctx: &RwLock<ProgramTestContext>,
    mints: Vec<Pubkey>,
    users: Vec<Pubkey>,
) {
    for mint in mints {
        let mut ctx = program_test_ctx.write().await;

        ctx.initialize_token_accounts(mint, users.as_slice())
            .await
            .unwrap();
    }
}

async fn get_account_info<'a, T: anchor_lang::AccountDeserialize + AnchorSerialize>(
    program_test_ctx: &'a RwLock<ProgramTestContext>,
    pda: &'a Pubkey,
    discriminator: [u8; 8],
) -> std::result::Result<AccountInfo<'a>, tokio::io::Error> {
    let lamports = Box::new(1_000_000);
    let owner = Box::new(Perpetuals::id());

    let acc: T = get_account::<T>(program_test_ctx, *pda).await;

    let mut data: Vec<u8> = acc.try_to_vec().unwrap();
    data.splice(0..0, discriminator.iter().cloned());

    let data_box = Box::new(data);

    Ok(AccountInfo::new(
        pda,
        false,
        true,
        Box::leak(lamports),
        // Serialize `CustomOracle` struct to Vec<u8>
        Box::leak(data_box).as_mut(),
        Box::leak(owner),
        false,
        Epoch::default(),
    ))
}

async fn get_custody_account_info<'a>(
    program_test_ctx: &'a RwLock<ProgramTestContext>,
    pda: &'a Pubkey,
) -> std::result::Result<AccountInfo<'a>, tokio::io::Error> {
    let custody_discriminator = [1, 184, 48, 81, 93, 131, 63, 145];

    get_account_info::<Custody>(program_test_ctx, pda, custody_discriminator).await
}

async fn get_custom_oracle_account_info<'a>(
    program_test_ctx: &'a RwLock<ProgramTestContext>,
    pda: &'a Pubkey,
) -> std::result::Result<AccountInfo<'a>, tokio::io::Error> {
    let custom_oracle_discriminator = [227, 170, 164, 218, 127, 16, 35, 223];

    get_account_info::<CustomOracle>(program_test_ctx, pda, custom_oracle_discriminator).await
}

pub async fn get_assets_under_management_usd(
    program_test_ctx: &RwLock<ProgramTestContext>,
    pool_pda: Pubkey,
) -> std::result::Result<u128, anchor_lang::error::Error> {
    let pool_account = get_account::<Pool>(program_test_ctx, pool_pda).await;

    let mut account_infos: Vec<AccountInfo> = vec![];

    // Add all custodies accounts
    for custody_pda in &pool_account.custodies {
        account_infos.push(
            get_custody_account_info(program_test_ctx, &custody_pda)
                .await
                .unwrap(),
        );
    }

    // Add all custodies accounts
    for custody_pda in &pool_account.custodies {
        let custody_acc: Custody = get_account::<Custody>(program_test_ctx, *custody_pda).await;

        let oracle_pda = Box::new(custody_acc.oracle.oracle_account);

        account_infos.push(
            get_custom_oracle_account_info(program_test_ctx, Box::leak(oracle_pda))
                .await
                .unwrap(),
        );
    }

    let curtime = get_current_unix_timestamp(program_test_ctx).await;

    pool_account.get_assets_under_management_usd(
        AumCalcMode::Max,
        account_infos.as_slice(),
        curtime,
    )
}

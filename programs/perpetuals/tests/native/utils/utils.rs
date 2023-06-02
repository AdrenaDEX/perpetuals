use {
    super::{fixtures, get_program_data_pda, get_test_oracle_account},
    crate::instructions,
    anchor_lang::{prelude::*, InstructionData},
    anchor_spl::token::spl_token,
    bonfida_test_utils::ProgramTestContextExt,
    borsh::BorshDeserialize,
    perpetuals::{
        adapters::SplGovernanceV3Adapter,
        instructions::{
            AddCustodyParams, AddLiquidityParams, SetCustodyConfigParams, SetTestOraclePriceParams,
        },
        math,
        state::{
            custody::{BorrowRateParams, Custody, Fees, PricingParams},
            perpetuals::{Permissions, Perpetuals},
            pool::TokenRatios,
        },
    },
    solana_program::{
        borsh::try_from_slice_unchecked, bpf_loader_upgradeable, clock::SLOT_MS,
        epoch_schedule::DEFAULT_SLOTS_PER_EPOCH, program_pack::Pack, stake_history::Epoch,
    },
    solana_program_test::{read_file, BanksClientError, ProgramTest, ProgramTestContext},
    solana_sdk::{
        account, compute_budget::ComputeBudgetInstruction, signature::Keypair, signer::Signer,
        signers::Signers,
    },
    std::{
        ops::{Div, Mul},
        path::Path,
    },
};

pub const ANCHOR_DISCRIMINATOR_SIZE: usize = 8;

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
    return (nb_days as i64) * (3_600 * 24);
}

pub async fn get_token_account(
    program_test_ctx: &mut ProgramTestContext,
    key: Pubkey,
) -> spl_token::state::Account {
    let raw_account = program_test_ctx
        .banks_client
        .get_account(key)
        .await
        .unwrap()
        .unwrap();

    spl_token::state::Account::unpack(&raw_account.data).unwrap()
}

pub async fn get_token_account_balance(
    program_test_ctx: &mut ProgramTestContext,
    key: Pubkey,
) -> u64 {
    get_token_account(program_test_ctx, key).await.amount
}

pub async fn get_borsh_account<T: BorshDeserialize>(
    program_test_ctx: &mut ProgramTestContext,
    address: &Pubkey,
) -> T {
    program_test_ctx
        .banks_client
        .get_account(*address)
        .await
        .unwrap()
        .map(|a| try_from_slice_unchecked(&a.data).unwrap())
        .unwrap_or_else(|| panic!("GET-TEST-ACCOUNT-ERROR: Account {} not found", address))
}

pub async fn try_get_account<T: anchor_lang::AccountDeserialize>(
    program_test_ctx: &mut ProgramTestContext,
    key: Pubkey,
) -> Option<T> {
    let account = program_test_ctx
        .banks_client
        .get_account(key)
        .await
        .unwrap();

    // an account with 0 lamport can be considered inexistant in the context of our tests
    // on mainnet, someone might just send lamports to the right place but doesn't matter here
    return if let Some(a) = account {
        Some(T::try_deserialize(&mut a.data.as_slice()).unwrap())
    } else {
        None
    };
}

pub async fn get_account<T: anchor_lang::AccountDeserialize>(
    program_test_ctx: &mut ProgramTestContext,
    key: Pubkey,
) -> T {
    let account = program_test_ctx
        .banks_client
        .get_account(key)
        .await
        .unwrap()
        .unwrap();

    T::try_deserialize(&mut account.data.as_slice()).unwrap()
}

pub async fn get_current_unix_timestamp(program_test_ctx: &mut ProgramTestContext) -> i64 {
    program_test_ctx
        .banks_client
        .get_sysvar::<solana_program::sysvar::clock::Clock>()
        .await
        .unwrap()
        .unix_timestamp
}

pub async fn initialize_users_token_accounts(
    program_test_ctx: &mut ProgramTestContext,
    mints: Vec<Pubkey>,
    users: Vec<Pubkey>,
) {
    for mint in mints {
        program_test_ctx
            .initialize_token_accounts(mint, users.as_slice())
            .await
            .unwrap();
    }
}

pub async fn initialize_token_account(
    program_test_ctx: &mut ProgramTestContext,
    mint: &Pubkey,
    owner: &Pubkey,
) -> Pubkey {
    program_test_ctx
        .initialize_token_accounts(*mint, &[*owner])
        .await
        .unwrap()[0]
}

pub async fn initialize_and_fund_token_account(
    program_test_ctx: &mut ProgramTestContext,
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
    program_test_ctx: &mut ProgramTestContext,
    mint_authority: &Keypair,
    mint: &Pubkey,
    token_account: &Pubkey,
    amount: u64,
) {
    program_test_ctx
        .mint_tokens(mint_authority, mint, token_account, amount)
        .await
        .unwrap();
}

// Doesn't check if you go before epoch 0 when passing negative amounts, be wary
pub async fn warp_forward(ctx: &mut ProgramTestContext, seconds: i64) {
    let clock_sysvar: Clock = ctx.banks_client.get_sysvar().await.unwrap();
    println!(
        "Original Time: epoch = {}, timestamp = {}",
        clock_sysvar.epoch, clock_sysvar.unix_timestamp
    );
    let mut new_clock = clock_sysvar.clone();
    new_clock.unix_timestamp += seconds;

    let seconds_since_epoch_start = new_clock.unix_timestamp - clock_sysvar.epoch_start_timestamp;
    let ms_since_epoch_start = seconds_since_epoch_start * 1_000;
    let slots_since_epoch_start = ms_since_epoch_start / SLOT_MS as i64;
    let epochs_since_epoch_start = slots_since_epoch_start / DEFAULT_SLOTS_PER_EPOCH as i64;
    new_clock.epoch = (new_clock.epoch as i64 + epochs_since_epoch_start) as u64;

    ctx.set_sysvar(&new_clock);
    let clock_sysvar: Clock = ctx.banks_client.get_sysvar().await.unwrap();
    println!(
        "New Time: epoch = {}, timestamp = {}",
        clock_sysvar.epoch, clock_sysvar.unix_timestamp
    );

    ctx.last_blockhash = ctx.banks_client.get_latest_blockhash().await.unwrap();
}

pub async fn add_spl_governance_program(
    program_test: &mut ProgramTest,
    upgrade_authority: &Keypair,
) {
    let mut program_bytes = read_file(std::env::current_dir().unwrap().join(Path::new(
        "tests/native/external_programs_binaries/spl_governance_3_1_0.so",
    )));

    let program_data_pda = Pubkey::find_program_address(
        &[SplGovernanceV3Adapter::id().as_ref()],
        &solana_program::bpf_loader_upgradeable::id(),
    )
    .0;

    let program = UpgradeableLoaderState::Program {
        programdata_address: program_data_pda,
    };
    let program_data = UpgradeableLoaderState::ProgramData {
        slot: 1,
        upgrade_authority_address: Some(upgrade_authority.pubkey()),
    };

    let serialized_program = bincode::serialize(&program).unwrap();

    let mut serialzed_program_data = bincode::serialize(&program_data).unwrap();
    serialzed_program_data.append(&mut program_bytes);

    let program_account = account::Account {
        lamports: Rent::default().minimum_balance(serialized_program.len()),
        data: serialized_program,
        owner: bpf_loader_upgradeable::ID,
        executable: true,
        rent_epoch: Epoch::default(),
    };
    let program_data_account = account::Account {
        lamports: Rent::default().minimum_balance(serialzed_program_data.len()),
        data: serialzed_program_data,
        owner: bpf_loader_upgradeable::ID,
        executable: false,
        rent_epoch: Epoch::default(),
    };

    program_test.add_account(SplGovernanceV3Adapter::id(), program_account);
    program_test.add_account(program_data_pda, program_data_account);
}

// Deploy the perpetuals program onchain as upgradeable program
pub async fn add_perpetuals_program(program_test: &mut ProgramTest, upgrade_authority: &Keypair) {
    // Deploy two accounts, one describing the program
    // and a second one holding the program's binary bytes
    let mut program_bytes = read_file(
        std::env::current_dir()
            .unwrap()
            .join(Path::new("../../target/deploy/perpetuals.so")),
    );

    let program_data_pda = get_program_data_pda().0;

    let program = UpgradeableLoaderState::Program {
        programdata_address: program_data_pda,
    };
    let program_data = UpgradeableLoaderState::ProgramData {
        slot: 1,
        upgrade_authority_address: Some(upgrade_authority.pubkey()),
    };

    let serialized_program = bincode::serialize(&program).unwrap();

    let mut serialzed_program_data = bincode::serialize(&program_data).unwrap();
    serialzed_program_data.append(&mut program_bytes);

    let program_account = account::Account {
        lamports: Rent::default().minimum_balance(serialized_program.len()),
        data: serialized_program,
        owner: bpf_loader_upgradeable::ID,
        executable: true,
        rent_epoch: Epoch::default(),
    };
    let program_data_account = account::Account {
        lamports: Rent::default().minimum_balance(serialzed_program_data.len()),
        data: serialzed_program_data,
        owner: bpf_loader_upgradeable::ID,
        executable: false,
        rent_epoch: Epoch::default(),
    };

    program_test.add_account(perpetuals::id(), program_account);
    program_test.add_account(program_data_pda, program_data_account);
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

pub async fn create_and_execute_perpetuals_ix<T: InstructionData, U: Signers>(
    program_test_ctx: &mut ProgramTestContext,
    accounts_meta: Vec<AccountMeta>,
    args: T,
    payer: Option<&Pubkey>,
    signing_keypairs: &U,
) -> std::result::Result<(), BanksClientError> {
    let ix = solana_sdk::instruction::Instruction {
        program_id: perpetuals::id(),
        accounts: accounts_meta,
        data: args.data(),
    };

    let tx = solana_sdk::transaction::Transaction::new_signed_with_payer(
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(400_000u32),
            ix,
        ],
        payer,
        signing_keypairs,
        program_test_ctx.last_blockhash,
    );

    let result = program_test_ctx.banks_client.process_transaction(tx).await;

    if result.is_err() {
        return Err(result.err().unwrap());
    }

    Ok(())
}

pub async fn create_and_execute_spl_governance_ix<U: Signers>(
    program_test_ctx: &mut ProgramTestContext,
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

    let tx = solana_sdk::transaction::Transaction::new_signed_with_payer(
        &[ix],
        payer,
        signing_keypairs,
        program_test_ctx.last_blockhash,
    );

    let result = program_test_ctx.banks_client.process_transaction(tx).await;

    if result.is_err() {
        return Err(result.err().unwrap());
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn set_custody_ratios(
    program_test_ctx: &mut ProgramTestContext,
    custody_admin: &Keypair,
    payer: &Keypair,
    custody_pda: &Pubkey,
    ratios: Vec<TokenRatios>,
    multisig_signers: &[&Keypair],
) {
    let custody_account = get_account::<Custody>(program_test_ctx, *custody_pda).await;

    instructions::test_set_custody_config(
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

pub struct SetupCustodyWithLiquidityParams {
    pub setup_custody_params: SetupCustodyParams,
    pub liquidity_amount: u64,
    pub payer: Keypair,
}

// @deprecated
// Setup the pool, add custodies then add liquidity
pub async fn setup_pool_with_custodies_and_liquidity(
    program_test_ctx: &mut ProgramTestContext,
    admin: &Keypair,
    pool_name: &str,
    payer: &Keypair,
    stake_reward_mint: &Pubkey,
    multisig_signers: &[&Keypair],
    custodies_params: Vec<SetupCustodyWithLiquidityParams>,
) -> (
    solana_sdk::pubkey::Pubkey,
    u8,
    solana_sdk::pubkey::Pubkey,
    u8,
    Vec<SetupCustodyInfo>,
) {
    // Setup the pool without ratio bound so we can provide liquidity without ratio limit error
    let (pool_pda, pool_bump, lp_token_mint_pda, lp_token_mint_bump, custodies_info) =
        setup_pool_with_custodies(
            program_test_ctx,
            admin,
            pool_name,
            payer,
            multisig_signers,
            custodies_params
                .iter()
                .map(|e| {
                    let mut params = e.setup_custody_params;

                    params.max_ratio = 10_000;
                    params.min_ratio = 0;

                    params
                })
                .collect(),
        )
        .await;

    // warp to avoid expired blockhash
    warp_forward(program_test_ctx, 1).await;

    // Add liquidity
    for params in custodies_params.as_slice() {
        println!(
            "adding liquidity for mint {}",
            params.setup_custody_params.mint
        );

        if params.liquidity_amount > 0 {
            instructions::test_add_liquidity(
                program_test_ctx,
                &params.payer,
                payer,
                &pool_pda,
                &params.setup_custody_params.mint,
                stake_reward_mint,
                AddLiquidityParams {
                    amount_in: params.liquidity_amount,
                    min_lp_amount_out: 1,
                },
            )
            .await
            .unwrap();
        }

        // warp to avoid expired blockhash
        warp_forward(program_test_ctx, 1).await;
    }

    // Set proper ratios
    let target_ratio = 10000 / custodies_params.len() as u64;
    let mut ratios: Vec<TokenRatios> = custodies_params
        .iter()
        .map(|x| TokenRatios {
            target: target_ratio,
            min: x.setup_custody_params.min_ratio,
            max: x.setup_custody_params.max_ratio,
        })
        .collect();
    if 10000 % custodies_params.len() != 0 {
        let len = ratios.len();
        ratios[len - 1].target += 10000 % custodies_params.len() as u64;
    }
    for (idx, _params) in custodies_params.as_slice().iter().enumerate() {
        set_custody_ratios(
            program_test_ctx,
            admin,
            payer,
            &custodies_info[idx].custody_pda,
            ratios.clone(),
            multisig_signers,
        )
        .await;
    }

    (
        pool_pda,
        pool_bump,
        lp_token_mint_pda,
        lp_token_mint_bump,
        custodies_info,
    )
}

#[derive(Clone, Copy)]
pub struct SetupCustodyParams {
    pub mint: Pubkey,
    pub decimals: u8,
    pub is_stable: bool,
    pub target_ratio: u64,
    pub min_ratio: u64,
    pub max_ratio: u64,
    pub initial_price: u64,
    pub initial_conf: u64,
    pub pricing_params: Option<PricingParams>,
    pub permissions: Option<Permissions>,
    pub fees: Option<Fees>,
    pub borrow_rate: Option<BorrowRateParams>,
}

#[derive(Clone, Copy)]
pub struct SetupCustodyInfo {
    pub test_oracle_pda: Pubkey,
    pub custody_pda: Pubkey,
}

pub async fn setup_pool_with_custodies(
    program_test_ctx: &mut ProgramTestContext,
    admin: &Keypair,
    pool_name: &str,
    payer: &Keypair,
    multisig_signers: &[&Keypair],
    custodies_params: Vec<SetupCustodyParams>,
) -> (
    solana_sdk::pubkey::Pubkey,
    u8,
    solana_sdk::pubkey::Pubkey,
    u8,
    Vec<SetupCustodyInfo>,
) {
    let (pool_pda, pool_bump, lp_token_mint_pda, lp_token_mint_bump) =
        instructions::test_add_pool(program_test_ctx, admin, payer, pool_name, multisig_signers)
            .await
            .unwrap();

    let mut custodies_info: Vec<SetupCustodyInfo> = Vec::new();

    let mut ratios = vec![];

    for (idx, custody_param) in custodies_params.iter().enumerate() {
        let test_oracle_pda = get_test_oracle_account(&pool_pda, &custody_param.mint).0;

        let target_ratio = 10000 / (idx + 1) as u64;
        ratios.push(TokenRatios {
            target: target_ratio,
            min: custody_param.min_ratio,
            max: custody_param.max_ratio,
        });
        ratios.iter_mut().for_each(|x| x.target = target_ratio);

        if 10000 % (idx + 1) != 0 {
            let len = ratios.len();
            ratios[len - 1].target += 10000 % (idx + 1) as u64;
        }

        let custody_pda = {
            let add_custody_params = AddCustodyParams {
                is_stable: custody_param.is_stable,
                oracle: fixtures::oracle_params_regular(test_oracle_pda),
                pricing: custody_param
                    .pricing_params
                    .unwrap_or_else(|| fixtures::pricing_params_regular(false)),
                permissions: custody_param
                    .permissions
                    .unwrap_or_else(fixtures::permissions_full),
                fees: custody_param
                    .fees
                    .unwrap_or_else(fixtures::fees_linear_regular),
                borrow_rate: custody_param
                    .borrow_rate
                    .unwrap_or_else(fixtures::borrow_rate_regular),

                // in BPS, 10_000 = 100%
                ratios: ratios.clone(),
            };

            instructions::test_add_custody(
                program_test_ctx,
                admin,
                payer,
                &pool_pda,
                &custody_param.mint,
                custody_param.decimals,
                add_custody_params,
                multisig_signers,
            )
            .await
            .unwrap()
            .0
        };

        let publish_time = get_current_unix_timestamp(program_test_ctx).await;

        instructions::test_set_test_oracle_price(
            program_test_ctx,
            admin,
            payer,
            &pool_pda,
            &custody_pda,
            &test_oracle_pda,
            SetTestOraclePriceParams {
                price: custody_param.initial_price,
                expo: -(custody_param.decimals as i32),
                conf: custody_param.initial_conf,
                publish_time,
            },
            multisig_signers,
        )
        .await
        .unwrap();

        custodies_info.push(SetupCustodyInfo {
            test_oracle_pda,
            custody_pda,
        });
    }

    (
        pool_pda,
        pool_bump,
        lp_token_mint_pda,
        lp_token_mint_bump,
        custodies_info,
    )
}

// @deprecated
pub async fn refresh_test_oracle_initial_prices(
    program_test_ctx: &mut ProgramTestContext,
    admin: &Keypair,
    pool_pda: &Pubkey,
    payer: &Keypair,
    multisig_signers: &[&Keypair],
    custodies_params: &Vec<SetupCustodyParams>,
    custodies_infos: &Vec<SetupCustodyInfo>,
) -> Result<()> {
    let publish_time = get_current_unix_timestamp(program_test_ctx).await;

    for i in 0..custodies_params.len() {
        let params = custodies_params[i];
        let info = custodies_infos[i];
        let test_oracle_pda = info.test_oracle_pda;
        let custody_pda = info.custody_pda;

        instructions::test_set_test_oracle_price(
            program_test_ctx,
            admin,
            payer,
            &pool_pda,
            &custody_pda,
            &test_oracle_pda,
            SetTestOraclePriceParams {
                price: params.initial_price,
                expo: -(params.decimals as i32),
                conf: scale(0, params.decimals),
                publish_time,
            },
            multisig_signers,
        )
        .await
        .unwrap();
    }
    Ok(())
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

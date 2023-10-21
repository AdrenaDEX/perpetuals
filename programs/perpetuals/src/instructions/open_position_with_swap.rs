//! OpenPositionWithSwap instruction handler

use {
    super::{open_position::OpenPositionParams, SwapParams},
    crate::{
        error::PerpetualsError,
        instructions::{BucketName, MintLmTokensFromBucketParams},
        math::{self, checked_sub},
        perpetuals,
        state::{
            cortex::Cortex,
            custody::Custody,
            oracle::OraclePrice,
            perpetuals::Perpetuals,
            pool::Pool,
            position::{Position, Side},
            staking::Staking,
        },
    },
    anchor_lang::prelude::*,
    anchor_spl::token::{Mint, Token, TokenAccount},
};

#[derive(Accounts)]
#[instruction(params: OpenPositionWithSwapParams)]
pub struct OpenPositionWithSwap<'info> {
    #[account()]
    pub owner: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    // i.e in case of open short position on ETH with BTC, receiving custody is ETH
    #[account(
        mut,
        constraint = funding_account.mint == receiving_custody.mint,
        has_one = owner
    )]
    pub funding_account: Box<Account<'info, TokenAccount>>,

    // used as temporary location to store collateral between the swap and the open position
    #[account(
        mut,
        constraint = collateral_account.mint == collateral_custody.mint,
        has_one = owner
    )]
    pub collateral_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = lm_token_account.mint == lm_token_mint.key(),
        has_one = owner
    )]
    pub lm_token_account: Box<Account<'info, TokenAccount>>,

    //
    // Receiving custody
    //
    // i.e in case of open short position on ETH with BTC, receiving custody is ETH
    //
    #[account(
        mut,
        seeds = [b"custody",
                 pool.key().as_ref(),
                 receiving_custody.mint.as_ref()],
        bump = receiving_custody.bump
    )]
    pub receiving_custody: Box<Account<'info, Custody>>,

    /// CHECK: oracle account for the received token
    #[account(
        constraint = receiving_custody_oracle_account.key() == receiving_custody.oracle.oracle_account
    )]
    pub receiving_custody_oracle_account: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [b"custody_token_account",
                 pool.key().as_ref(),
                 receiving_custody.mint.as_ref()],
        bump = receiving_custody.token_account_bump
    )]
    pub receiving_custody_token_account: Box<Account<'info, TokenAccount>>,
    //
    // Collateral Custody
    //
    // i.e in case of open short position on ETH with BTC, collateral custody is USDC
    //
    #[account(
        mut,
        seeds = [b"custody",
                 pool.key().as_ref(),
                 collateral_custody.mint.as_ref()],
        bump = collateral_custody.bump
    )]
    pub collateral_custody: Box<Account<'info, Custody>>,

    /// CHECK: oracle account for the received token
    #[account(
        constraint = collateral_custody_oracle_account.key() == collateral_custody.oracle.oracle_account
    )]
    pub collateral_custody_oracle_account: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [b"custody_token_account",
                 pool.key().as_ref(),
                 collateral_custody.mint.as_ref()],
        bump = collateral_custody.token_account_bump
    )]
    pub collateral_custody_token_account: Box<Account<'info, TokenAccount>>,
    //
    // Principal Custody
    //
    // i.e in case of open short position on ETH with BTC, principal custody is BTC
    //
    #[account(
        mut,
        seeds = [b"custody",
                 pool.key().as_ref(),
                 principal_custody.mint.as_ref()],
        bump = principal_custody.bump
    )]
    pub principal_custody: Box<Account<'info, Custody>>,

    /// CHECK: oracle account for the received token
    #[account(
        constraint = principal_custody_oracle_account.key() == principal_custody.oracle.oracle_account
    )]
    pub principal_custody_oracle_account: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [b"custody_token_account",
                 pool.key().as_ref(),
                 principal_custody.mint.as_ref()],
        bump = principal_custody.token_account_bump
    )]
    pub principal_custody_token_account: Box<Account<'info, TokenAccount>>,
    //
    //
    //
    /// CHECK: empty PDA, authority for token accounts
    #[account(
        seeds = [b"transfer_authority"],
        bump = perpetuals.transfer_authority_bump
    )]
    pub transfer_authority: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [b"cortex"],
        bump = cortex.bump
    )]
    pub cortex: Box<Account<'info, Cortex>>,

    #[account(
        seeds = [b"perpetuals"],
        bump = perpetuals.perpetuals_bump
    )]
    pub perpetuals: Box<Account<'info, Perpetuals>>,

    #[account(
        mut,
        seeds = [b"staking", lm_staking.staked_token_mint.as_ref()],
        bump = lm_staking.bump,
        constraint = lm_staking.reward_token_mint.key() == staking_reward_token_mint.key()
    )]
    pub lm_staking: Box<Account<'info, Staking>>,

    #[account(
        mut,
        seeds = [b"staking", lp_token_mint.key().as_ref()],
        bump = lp_staking.bump,
        constraint = lp_staking.reward_token_mint.key() == staking_reward_token_mint.key()
    )]
    pub lp_staking: Box<Account<'info, Staking>>,

    #[account(
        mut,
        seeds = [b"pool",
                 pool.name.as_bytes()],
        bump = pool.bump
    )]
    pub pool: Box<Account<'info, Pool>>,

    /// CHECK: initialized by CPI
    #[account(mut)]
    pub position: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [b"custody",
                 pool.key().as_ref(),
                 staking_reward_token_custody.mint.as_ref()],
        bump = staking_reward_token_custody.bump,
        constraint = staking_reward_token_custody.mint == staking_reward_token_mint.key(),
    )]
    pub staking_reward_token_custody: Box<Account<'info, Custody>>,

    /// CHECK:
    #[account(
        constraint = staking_reward_token_custody_oracle_account.key() == staking_reward_token_custody.oracle.oracle_account
    )]
    pub staking_reward_token_custody_oracle_account: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [b"custody_token_account",
                 pool.key().as_ref(),
                 staking_reward_token_custody.mint.as_ref()],
        bump = staking_reward_token_custody.token_account_bump,
    )]
    pub staking_reward_token_custody_token_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        token::mint = lm_staking.reward_token_mint,
        seeds = [b"staking_reward_token_vault", lm_staking.key().as_ref()],
        bump = lm_staking.reward_token_vault_bump
    )]
    pub lm_staking_reward_token_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        token::mint = lp_staking.reward_token_mint,
        seeds = [b"staking_reward_token_vault", lp_staking.key().as_ref()],
        bump = lp_staking.reward_token_vault_bump
    )]
    pub lp_staking_reward_token_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [b"lm_token_mint"],
        bump = cortex.lm_token_bump
    )]
    pub lm_token_mint: Box<Account<'info, Mint>>,

    #[account(
        mut,
        seeds = [b"lp_token_mint",
                 pool.key().as_ref()],
        bump = pool.lp_token_bump
    )]
    pub lp_token_mint: Box<Account<'info, Mint>>,

    #[account()]
    pub staking_reward_token_mint: Box<Account<'info, Mint>>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    perpetuals_program: Program<'info, Perpetuals>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct OpenPositionWithSwapParams {
    pub price: u64,
    pub collateral: u64,
    pub size: u64,
    pub side: Side,
}

pub fn open_position_with_swap(
    ctx: Context<OpenPositionWithSwap>,
    params: &OpenPositionWithSwapParams,
) -> Result<()> {
    let cortex = ctx.accounts.cortex.as_mut();
    let perpetuals = ctx.accounts.perpetuals.as_mut();

    let swap_required = ctx
        .accounts
        .receiving_custody
        .key()
        .ne(&ctx.accounts.collateral_custody.key());

    // transfer tokens
    /*msg!("Transfer tokens");
    perpetuals.transfer_tokens_from_user(
        ctx.accounts.funding_account.to_account_info(),
        ctx.accounts
            .receiving_custody_token_account
            .to_account_info(),
        ctx.accounts.owner.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        params.collateral,
    )?;*/

    let collateral_amount = if swap_required {
        let collateral_amount_before = ctx.accounts.collateral_account.amount;

        perpetuals.internal_swap(
            ctx.accounts.owner.to_account_info(),
            ctx.accounts.transfer_authority.to_account_info(),
            ctx.accounts.funding_account.to_account_info(),
            ctx.accounts.collateral_account.to_account_info(),
            ctx.accounts.lm_token_account.to_account_info(),
            cortex.to_account_info(),
            perpetuals.to_account_info(),
            ctx.accounts.pool.to_account_info(),
            ctx.accounts.receiving_custody.to_account_info(),
            ctx.accounts
                .receiving_custody_oracle_account
                .to_account_info(),
            ctx.accounts
                .receiving_custody_token_account
                .to_account_info(),
            ctx.accounts.collateral_custody.to_account_info(),
            ctx.accounts
                .collateral_custody_oracle_account
                .to_account_info(),
            ctx.accounts
                .collateral_custody_token_account
                .to_account_info(),
            ctx.accounts.staking_reward_token_custody.to_account_info(),
            ctx.accounts
                .staking_reward_token_custody_oracle_account
                .to_account_info(),
            ctx.accounts
                .staking_reward_token_custody_token_account
                .to_account_info(),
            ctx.accounts.lm_staking_reward_token_vault.to_account_info(),
            ctx.accounts.lp_staking_reward_token_vault.to_account_info(),
            ctx.accounts.staking_reward_token_mint.to_account_info(),
            ctx.accounts.lm_staking.to_account_info(),
            ctx.accounts.lp_staking.to_account_info(),
            ctx.accounts.lm_token_mint.to_account_info(),
            ctx.accounts.lp_token_mint.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            ctx.accounts.perpetuals_program.to_account_info(),
            SwapParams {
                amount_in: params.collateral,
                min_amount_out: 0,
            },
        )?;

        // Reload accounts so they are up to date after what happened in the cpi
        {
            ctx.accounts.receiving_custody.reload()?;
            ctx.accounts.collateral_account.reload()?;
            ctx.accounts.lm_token_account.reload()?;
            cortex.reload()?;
            perpetuals.reload()?;
            ctx.accounts.pool.reload()?;
            ctx.accounts.receiving_custody_token_account.reload()?;
            ctx.accounts.collateral_custody.reload()?;
            ctx.accounts.staking_reward_token_custody.reload()?;
            ctx.accounts
                .staking_reward_token_custody_token_account
                .reload()?;
            ctx.accounts.lm_staking_reward_token_vault.reload()?;
            ctx.accounts.lp_staking_reward_token_vault.reload()?;
            ctx.accounts.staking_reward_token_mint.reload()?;
            ctx.accounts.lm_staking.reload()?;
            ctx.accounts.lp_staking.reload()?;
            ctx.accounts.lm_token_mint.reload()?;
            ctx.accounts.lp_token_mint.reload()?;
        }

        let collateral_amount_after = ctx.accounts.collateral_account.amount;
        let collateral_amount = checked_sub(collateral_amount_after, collateral_amount_before)?;

        msg!("Swapped for {} tokens", collateral_amount);

        collateral_amount
    } else {
        msg!("No swap required");

        params.collateral
    };

    perpetuals.internal_open_position(
        //TODO have two authority, one is the user, one is the authority beind the funds
        // when called with client, both are the same
        // when called in CPI, user is the user and authority is the transfer_authority
        ctx.accounts.owner.to_account_info(),
        ctx.accounts.transfer_authority.to_account_info(),
        ctx.accounts.payer.to_account_info(),
        ctx.accounts.collateral_account.to_account_info(),
        ctx.accounts.lm_token_account.to_account_info(),
        ctx.accounts.lm_staking.to_account_info(),
        ctx.accounts.lp_staking.to_account_info(),
        cortex.to_account_info(),
        perpetuals.to_account_info(),
        ctx.accounts.pool.to_account_info(),
        ctx.accounts.position.to_account_info(),
        ctx.accounts.staking_reward_token_custody.to_account_info(),
        ctx.accounts
            .staking_reward_token_custody_oracle_account
            .to_account_info(),
        ctx.accounts
            .staking_reward_token_custody_token_account
            .to_account_info(),
        ctx.accounts.principal_custody.to_account_info(),
        ctx.accounts
            .principal_custody_oracle_account
            .to_account_info(),
        ctx.accounts.collateral_custody.to_account_info(),
        ctx.accounts
            .collateral_custody_oracle_account
            .to_account_info(),
        ctx.accounts
            .collateral_custody_token_account
            .to_account_info(),
        ctx.accounts.lm_staking_reward_token_vault.to_account_info(),
        ctx.accounts.lp_staking_reward_token_vault.to_account_info(),
        ctx.accounts.lm_token_mint.to_account_info(),
        ctx.accounts.lp_token_mint.to_account_info(),
        ctx.accounts.staking_reward_token_mint.to_account_info(),
        ctx.accounts.system_program.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        ctx.accounts.perpetuals_program.to_account_info(),
        OpenPositionParams {
            price: params.price,
            collateral: collateral_amount,
            size: params.size,
            side: params.side,
        },
    )?;

    Ok(())
}

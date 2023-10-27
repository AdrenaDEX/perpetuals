//! GetOpenPositionWithSwapAmountAndFees instruction handler

use {
    super::{get_swap_amount_and_fees::GetSwapAmountAndFeesParams, GetEntryPriceAndFeeParams},
    crate::state::{
        custody::Custody,
        perpetuals::{OpenPositionWithSwapAmountAndFees, Perpetuals},
        pool::Pool,
        position::Side,
    },
    anchor_lang::prelude::*,
};

#[derive(Accounts)]
pub struct GetOpenPositionWithSwapAmountAndFees<'info> {
    #[account(
        seeds = [b"perpetuals"],
        bump = perpetuals.perpetuals_bump
    )]
    pub perpetuals: Box<Account<'info, Perpetuals>>,

    #[account(
        seeds = [b"pool",
                 pool.name.as_bytes()],
        bump = pool.bump
    )]
    pub pool: Box<Account<'info, Pool>>,

    #[account(
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
        seeds = [b"custody",
                 pool.key().as_ref(),
                 collateral_custody.mint.as_ref()],
        bump = collateral_custody.bump
    )]
    pub collateral_custody: Box<Account<'info, Custody>>,

    /// CHECK:
    #[account(
        constraint = collateral_custody_oracle_account.key() == collateral_custody.oracle.oracle_account
    )]
    pub collateral_custody_oracle_account: AccountInfo<'info>,

    #[account(
        seeds = [b"custody",
                 pool.key().as_ref(),
                 principal_custody.mint.as_ref()],
        bump = principal_custody.bump
    )]
    pub principal_custody: Box<Account<'info, Custody>>,

    /// CHECK:
    #[account(
        constraint = principal_custody_oracle_account.key() == principal_custody.oracle.oracle_account
    )]
    pub principal_custody_oracle_account: AccountInfo<'info>,
    perpetuals_program: Program<'info, Perpetuals>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct GetOpenPositionWithSwapAmountAndFeesParams {
    pub collateral_amount: u64,
    pub size: u64,
    pub side: Side,
}

pub fn get_open_position_with_swap_amount_and_fees(
    ctx: Context<GetOpenPositionWithSwapAmountAndFees>,
    params: &GetOpenPositionWithSwapAmountAndFeesParams,
) -> Result<OpenPositionWithSwapAmountAndFees> {
    let perpetuals = ctx.accounts.perpetuals.as_ref();

    let swap_required = ctx
        .accounts
        .receiving_custody
        .key()
        .ne(&ctx.accounts.collateral_custody.key());

    // calculate swap fee
    let (collateral_amount, swap_fee_in, swap_fee_out) = if swap_required {
        let swap_amount_and_fee = perpetuals.internal_get_swap_amount_and_fee(
            perpetuals.to_account_info(),
            ctx.accounts.pool.to_account_info(),
            ctx.accounts.receiving_custody.to_account_info(),
            ctx.accounts
                .receiving_custody_oracle_account
                .to_account_info(),
            ctx.accounts.collateral_custody.to_account_info(),
            ctx.accounts
                .collateral_custody_oracle_account
                .to_account_info(),
            ctx.accounts.perpetuals_program.to_account_info(),
            GetSwapAmountAndFeesParams {
                amount_in: params.collateral_amount,
            },
        )?;

        (
            swap_amount_and_fee.amount_out,
            swap_amount_and_fee.fee_in,
            swap_amount_and_fee.fee_out,
        )
    } else {
        (params.collateral_amount, 0u64, 0u64)
    };

    let entry_price_and_fee = perpetuals.internal_get_entry_price_and_fee(
        perpetuals.to_account_info(),
        ctx.accounts.pool.to_account_info(),
        ctx.accounts.principal_custody.to_account_info(),
        ctx.accounts
            .principal_custody_oracle_account
            .to_account_info(),
        ctx.accounts.collateral_custody.to_account_info(),
        ctx.accounts
            .collateral_custody_oracle_account
            .to_account_info(),
        ctx.accounts.perpetuals_program.to_account_info(),
        GetEntryPriceAndFeeParams {
            collateral: collateral_amount,
            size: params.size,
            side: params.side,
        },
    )?;

    // Informations are not entirely correct, as the swap impacts are not taken into account in the
    // calculations of entry_price_and_fee (custody utilization, ratios target etc.)
    // still it's a very close estimation
    Ok(OpenPositionWithSwapAmountAndFees {
        entry_price: entry_price_and_fee.entry_price,
        liquidation_price: entry_price_and_fee.liquidation_price,
        swap_fee_in,
        swap_fee_out,
        open_position_fee: entry_price_and_fee.fee,
    })
}

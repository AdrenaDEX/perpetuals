//! ClaimStake instruction handler

use {
    crate::{
        math,
        state::{cortex::Cortex, perpetuals::Perpetuals, staking::Staking},
    },
    anchor_lang::prelude::*,
    anchor_spl::token::{Mint, Token, TokenAccount},
    num::Zero,
    solana_program::log::sol_log_compute_units,
};

#[derive(Accounts)]
pub struct ClaimStakes<'info> {
    // TODO:
    // Caller should be either the program iself, or the owner, cannot be third party
    #[account(mut)]
    pub caller: Signer<'info>,

    /// CHECK: verified through the `stake` account seed derivation
    #[account(mut)]
    pub owner: AccountInfo<'info>,

    // reward token account for the caller if elligible
    #[account(
            mut,
            token::mint = stake_reward_token_mint,
            constraint = caller_reward_token_account.owner == caller.key()
        )]
    pub caller_reward_token_account: Box<Account<'info, TokenAccount>>,

    // reward token account of the stake owner
    #[account(
        mut,
        token::mint = stake_reward_token_mint,
        has_one = owner
    )]
    pub owner_reward_token_account: Box<Account<'info, TokenAccount>>,

    // staking reward token vault
    #[account(
        mut,
        token::mint = stake_reward_token_mint,
        seeds = [b"stake_reward_token_account"],
        bump = cortex.stake_reward_token_account_bump
    )]
    pub stake_reward_token_account: Box<Account<'info, TokenAccount>>,

    /// CHECK: empty PDA, authority for token accounts
    #[account(
        seeds = [b"transfer_authority"],
        bump = perpetuals.transfer_authority_bump
    )]
    pub transfer_authority: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [b"staking",
                 owner.key().as_ref()],
        bump = staking.bump
    )]
    pub staking: Box<Account<'info, Staking>>,

    #[account(
        mut,
        seeds = [b"cortex"],
        bump = cortex.bump,
        has_one = stake_reward_token_mint
    )]
    pub cortex: Box<Account<'info, Cortex>>,

    #[account(
        seeds = [b"perpetuals"],
        bump = perpetuals.perpetuals_bump
    )]
    pub perpetuals: Box<Account<'info, Perpetuals>>,

    #[account()]
    pub stake_reward_token_mint: Box<Account<'info, Mint>>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

pub fn claim_stakes(ctx: Context<ClaimStakes>) -> Result<()> {
    let staking = ctx.accounts.staking.as_mut();
    let cortex = ctx.accounts.cortex.as_mut();

    msg!("Process resolved rounds & rewards calculation");

    // Process resolved rounds and:
    // 1. Calculate total rewards amount
    // 2. Drop fully claimed rounds
    let rewards_token_amount = {
        // prints compute budget before
        sol_log_compute_units();

        let resolved_staking_rounds_len_before = cortex.resolved_staking_rounds.len();

        msg!(
            ">>> resolved_staking_rounds_len_before: {}",
            resolved_staking_rounds_len_before
        );

        let mut rewards_token_amount: u64 = 0;

        let stake_token_decimals = cortex.stake_token_decimals as i32;
        let stake_reward_token_decimals = cortex.stake_reward_token_decimals as i32;

        // For each resolved staking rounds
        cortex.resolved_staking_rounds.retain_mut(|round| {
            // Locked staking
            {
                // For each user locked stakes
                for locked_stake in staking.locked_stakes.iter_mut() {
                    // Stake is elligible for rewards
                    if locked_stake.qualifies_for_rewards_from(round) {
                        let locked_stake_rewards_token_amount = math::checked_decimal_mul(
                            locked_stake.amount_with_multiplier,
                            -stake_token_decimals,
                            round.rate,
                            -(Perpetuals::RATE_DECIMALS as i32),
                            -stake_reward_token_decimals,
                        )
                        .unwrap();

                        rewards_token_amount = math::checked_add(
                            rewards_token_amount,
                            locked_stake_rewards_token_amount,
                        )
                        .unwrap();

                        round.total_claim =
                            math::checked_add(round.total_claim, locked_stake_rewards_token_amount)
                                .unwrap();
                    }
                }
            }

            // Liquid staking
            {
                msg!(">>> Check if liquid stake is elligbile for rewards");

                // Stake is elligible for rewards
                if staking.liquid_stake.qualifies_for_rewards_from(round) {
                    msg!(">>> YES ELLIGIBLE");

                    let liquid_stake_rewards_token_amount = math::checked_decimal_mul(
                        staking.liquid_stake.amount_with_multiplier,
                        -stake_token_decimals,
                        round.rate,
                        -(Perpetuals::RATE_DECIMALS as i32),
                        -stake_reward_token_decimals,
                    )
                    .unwrap();

                    rewards_token_amount =
                        math::checked_add(rewards_token_amount, liquid_stake_rewards_token_amount)
                            .unwrap();

                    round.total_claim =
                        math::checked_add(round.total_claim, liquid_stake_rewards_token_amount)
                            .unwrap();
                } else {
                    msg!(">>> NOT ELLIGIBLE");
                }
            }

            // retain element if there is stake that has not been claimed yet by other participants
            let round_fully_claimed = round.total_claim == round.total_stake;
            // note: some dust of rewards will build up in the token account due to rate precision of 9 units
            !round_fully_claimed
        });

        // Realloc Cortex to account for dropped staking rounds if needed
        {
            let staking_rounds_delta = math::checked_sub(
                cortex.resolved_staking_rounds.len() as i32,
                resolved_staking_rounds_len_before as i32,
            )?;

            if !staking_rounds_delta.is_zero() {
                msg!("Realloc Cortex");
                Perpetuals::realloc(
                    ctx.accounts.caller.to_account_info(),
                    cortex.clone().to_account_info(),
                    ctx.accounts.system_program.to_account_info(),
                    cortex.new_size(staking_rounds_delta)?,
                    true,
                )?;
            }
        }

        // prints compute budget after
        sol_log_compute_units();

        rewards_token_amount
    };

    msg!("Distribute rewards");

    {
        if rewards_token_amount.is_zero() {
            msg!("No reward tokens to claim at this time");
            return Ok(());
        }

        msg!("Transfer rewards_token_amount: {}", rewards_token_amount);
        let perpetuals = ctx.accounts.perpetuals.as_mut();

        let (owner_rewards_token_amount, caller_reward_token_amount) = {
            if !ctx.accounts.caller.key().eq(&ctx.accounts.owner.key()) {
                //
                // TODO: Apply fees to rewards because the claimor is the program
                //
            }

            (rewards_token_amount, 0)
        };

        perpetuals.transfer_tokens(
            ctx.accounts.stake_reward_token_account.to_account_info(),
            ctx.accounts.owner_reward_token_account.to_account_info(),
            ctx.accounts.transfer_authority.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            owner_rewards_token_amount,
        )?;

        if !caller_reward_token_amount.is_zero() {
            perpetuals.transfer_tokens(
                ctx.accounts.stake_reward_token_account.to_account_info(),
                ctx.accounts.caller_reward_token_account.to_account_info(),
                ctx.accounts.transfer_authority.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
                caller_reward_token_amount,
            )?;
        }
    }

    // Update stakings claim time
    {
        // refresh claim time while keeping the claim time out of the current round
        // so that the user stay eligible for current round rewards
        let claim_time = math::checked_sub(cortex.current_staking_round.start_time, 1)?;

        // Locked staking
        for mut locked_stake in staking.locked_stakes.iter_mut() {
            locked_stake.claim_time = claim_time;
        }

        // Liquid staking
        staking.liquid_stake.claim_time = claim_time;
    }

    // Adapt current/next round
    {
        for locked_stake in staking.locked_stakes.iter_mut() {
            // remove stake from current staking round
            cortex.current_staking_round.total_stake = math::checked_sub(
                cortex.current_staking_round.total_stake,
                locked_stake.amount_with_multiplier,
            )?;

            // update resolved stake token amount left, by removing the previously staked amount
            cortex.resolved_stake_token_amount = math::checked_sub(
                cortex.resolved_stake_token_amount,
                locked_stake.amount_with_multiplier,
            )?;

            // update resolved reward token amount left
            cortex.resolved_reward_token_amount =
                math::checked_sub(cortex.resolved_reward_token_amount, rewards_token_amount)?;

            // add stake to next staking round
            cortex.next_staking_round.total_stake = math::checked_add(
                cortex.next_staking_round.total_stake,
                locked_stake.amount_with_multiplier,
            )?;
        }

        msg!(
            "Cortex.resolved_staking_rounds after claim stake {:?}",
            cortex.resolved_staking_rounds
        );
        msg!(
            "Cortex.current_staking_round after claim stake {:?}",
            cortex.current_staking_round
        );
        msg!(
            "Cortex.next_staking_round after claim stake {:?}",
            cortex.next_staking_round
        );
    }

    Ok(())
}

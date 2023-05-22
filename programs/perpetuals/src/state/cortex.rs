//! Cortex state and routines

use {
    super::{perpetuals::Perpetuals, stake::Stake, vest::Vest},
    crate::math,
    anchor_lang::prelude::*,
};

pub const DAYS_PER_YEAR: i64 = 365;
pub const HOURS_PER_DAY: i64 = 24;
pub const SECONDS_PER_HOURS: i64 = 3600;

#[account]
#[derive(Default, Debug)]
pub struct Cortex {
    pub vests: Vec<Pubkey>,
    pub bump: u8,
    pub lm_token_bump: u8,
    pub governance_token_bump: u8,
    pub stake_token_account_bump: u8,
    pub stake_reward_token_account_bump: u8,
    pub inception_epoch: u64,
    pub governance_program: Pubkey,
    pub governance_realm: Pubkey,
    pub stake_reward_token_mint: Pubkey,
    pub stake_token_decimals: u8,
    pub stake_reward_token_decimals: u8,
    // these two values are used to resolve staking rounds
    // `resolved_reward_token_amount` represents the amount of rewards allocated to resolved rounds, claimable (excluding current/next round)
    pub resolved_reward_token_amount: u64,
    // `resolved_stake_token_amount`represents the amount of staked token locked in resolved rounds, claimable (excluding current/next round)
    pub resolved_stake_token_amount: u64,
    pub current_staking_round: StakingRound,
    pub next_staking_round: StakingRound,
    // must be the last element of the struct for reallocs
    pub resolved_staking_rounds: Vec<StakingRound>,
}

#[derive(Default, Debug, Clone, AnchorSerialize, AnchorDeserialize, PartialEq)]
pub struct StakingRound {
    pub start_time: i64,
    pub rate: u64, // the amount of reward you get per staked stake-token for that round - set at Round's resolution
    pub total_stake: u64, // - set at Round's resolution
    pub total_claim: u64, // - set at Round's resolution
}

impl StakingRound {
    const LEN: usize = std::mem::size_of::<StakingRound>();
    // a staking round can be resolved after at least 6 hours
    const ROUND_MIN_DURATION_HOURS: i64 = 6;
    pub const ROUND_MIN_DURATION_SECONDS: i64 = Self::ROUND_MIN_DURATION_HOURS * SECONDS_PER_HOURS;
    // A Stake account max age is 365, this is due to computing limit in the claim instruction.
    // This is also arbitrarily used as the max theoretical amount of staking rounds
    // stored if all were persisting (rounds get cleaned up once their rewards are fully claimed by their participants).
    // This is done to ensure the Cortex.resolved_staking_rounds doesn't grow out of proportion, primarily to facilitate
    // the fetching from front end.
    pub const MAX_RESOLVED_ROUNDS: usize =
        ((Stake::MAX_AGE_SECONDS / SECONDS_PER_HOURS) / Self::ROUND_MIN_DURATION_HOURS) as usize;

    pub fn new(start_time: i64) -> Self {
        Self {
            start_time,
            rate: u64::MIN,
            total_stake: u64::MIN,
            total_claim: u64::MIN,
        }
    }
}

/// Cortex
impl Cortex {
    pub const LEN: usize = 8 + std::mem::size_of::<Cortex>();
    const INCEPTION_EMISSION_RATE: u64 = Perpetuals::RATE_POWER as u64; // 100%
    pub const FEE_TO_REWARD_RATIO_BPS: u8 = 10; //  0.10% of fees paid become rewards
    pub const LM_DECIMALS: u8 = Perpetuals::USD_DECIMALS;
    pub const GOVERNANCE_DECIMALS: u8 = Perpetuals::USD_DECIMALS;
    // a limit is needed to keep the Cortex size deterministic
    pub const MAX_ONGOING_VESTS: usize = 64;
    // lenght of our epoch relative to Solana epochs (1 Solana epoch is ~2-3 days)
    const ADRENA_EPOCH: u8 = 10;

    pub fn get_swap_lm_rewards_amounts(&self, (fee_in, fee_out): (u64, u64)) -> Result<(u64, u64)> {
        Ok((
            self.get_lm_rewards_amount(fee_in)?,
            self.get_lm_rewards_amount(fee_out)?,
        ))
    }

    // lm rewards amount is a portion of fees paid, scaled down by the current epoch emission rate
    pub fn get_lm_rewards_amount(&self, fee_amount: u64) -> Result<u64> {
        let base_reward_amount = math::checked_as_u64(math::checked_div(
            math::checked_mul(fee_amount as u128, Self::FEE_TO_REWARD_RATIO_BPS as u128)?,
            Perpetuals::BPS_POWER,
        )?)?;
        let emission_rate = Self::get_emission_rate(self.inception_epoch, self.get_epoch()?)?;
        let epoch_adjusted_reward_amount = math::checked_as_u64(math::checked_div(
            math::checked_mul(base_reward_amount as u128, emission_rate as u128)?,
            Perpetuals::RATE_POWER,
        )?)?;
        Ok(epoch_adjusted_reward_amount)
    }

    fn get_emission_rate(inception_epoch: u64, current_epoch: u64) -> Result<u64> {
        let elapsed_epochs = std::cmp::max(math::checked_sub(current_epoch, inception_epoch)?, 1);

        math::checked_div(
            Self::INCEPTION_EMISSION_RATE,
            std::cmp::max(elapsed_epochs / Cortex::ADRENA_EPOCH as u64, 1),
        )
    }

    pub fn get_epoch(&self) -> Result<u64> {
        let epoch = solana_program::sysvar::clock::Clock::get()?.epoch;
        Ok(epoch)
    }

    // returns the current size of the Cortex
    pub fn size(&self) -> usize {
        let size = Cortex::LEN
            + self.vests.len() * Vest::LEN
            + self.resolved_staking_rounds.len() * StakingRound::LEN;
        return size;
    }

    // returns the new size of the structure after adding/removing some staking rounds
    pub fn new_size(&self, staking_rounds_delta: i32) -> Result<usize> {
        math::checked_as_usize(math::checked_add(
            self.size() as i32,
            math::checked_mul(staking_rounds_delta, StakingRound::LEN as i32)?,
        )?)
    }

    pub fn current_staking_round_is_resolvable(&self, current_time: i64) -> Result<bool> {
        Ok(current_time
            >= math::checked_add(
                self.current_staking_round.start_time,
                StakingRound::ROUND_MIN_DURATION_SECONDS,
            )?)
    }
}

#[cfg(test)]
mod test {
    use {super::*, num_traits::Zero, proptest::prelude::*};

    fn get_fixture_staking_round() -> StakingRound {
        StakingRound {
            start_time: 0,
            rate: 0,
            total_stake: 0,
            total_claim: 0,
        }
    }

    fn get_fixture_cortex(resolved_staking_rounds_count: usize) -> Cortex {
        Cortex {
            vests: Vec::new(),
            bump: 255,
            lm_token_bump: 255,
            governance_token_bump: 255,
            stake_token_account_bump: 255,
            stake_reward_token_account_bump: 255,
            inception_epoch: 0,
            governance_program: Pubkey::default(),
            governance_realm: Pubkey::default(),
            stake_reward_token_mint: Pubkey::default(),
            stake_token_decimals: 0,
            stake_reward_token_decimals: 0,
            resolved_reward_token_amount: 0,
            resolved_stake_token_amount: 0,
            current_staking_round: get_fixture_staking_round(),
            next_staking_round: get_fixture_staking_round(),
            resolved_staking_rounds: vec![
                get_fixture_staking_round();
                resolved_staking_rounds_count
            ],
        }
    }

    #[test]
    fn test_new_size() {
        proptest!(|(staking_rounds_count in usize::MIN..StakingRound::MAX_RESOLVED_ROUNDS, staking_rounds_delta in -(StakingRound::MAX_RESOLVED_ROUNDS as i32)..(StakingRound::MAX_RESOLVED_ROUNDS as i32))| {
            prop_assume!(staking_rounds_delta.abs() as usize <= staking_rounds_count);
            let cortex = get_fixture_cortex(staking_rounds_count);
            let size = cortex.size();
           let new_size = cortex.new_size(staking_rounds_delta).unwrap();

            if staking_rounds_delta.is_negative() {
            assert_eq!(
                new_size, size - StakingRound::LEN * staking_rounds_delta.abs() as usize
            );
            } else if staking_rounds_delta.is_positive() {
                            assert_eq!(
                new_size, size + StakingRound::LEN * staking_rounds_delta.abs() as usize
            );
            } else if staking_rounds_delta.is_zero() {
                            assert_eq!(
                new_size, size
            );
            }

        });
    }

    #[test]
    fn test_get_emission_rate() {
        proptest!(|(inception_epoch: u32, epoches_elapsed: u32)| {
            let current_epoch = inception_epoch as u64 + epoches_elapsed as u64;
            let divider = match current_epoch {
                0 => 1,
                _ => epoches_elapsed as u64 / 10
            };
            assert_eq!(
                Cortex::get_emission_rate(inception_epoch as u64, current_epoch).unwrap(),
                Cortex::INCEPTION_EMISSION_RATE / divider
            );
        });
    }
}

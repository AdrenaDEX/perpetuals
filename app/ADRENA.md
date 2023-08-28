# CLI setup

## Keys

admin: 5vAooJKJxWXVPNb13dBq1jPsuE3RTbMCfYuounMJcAvb

alice: ArJJRUcbSrqqMuTrMdQNvBsXNokQBh3VCt2EDrg2aqnc
martin: 7KQ7YgFmP4mz4p1UQnwrESZ58APxDZUoWyN5GXeXm4cz
paul: 37dThGRY77tkRrnzgbfAkZaNnCQzGdQWnLCE9UD4MuPy

usdc: 4ZY3ZH8bStniqdCZdR14xsWW6vrMsCJrusobTdy4JipC
eth: 3AHAG1ZSUnPz43XBFKRqnLwhdyz29WhHvYQgVrcheCwr
btc: HRvpfs8bKiUbLzSgT4LmKKugafZ8ePi5Vq7icJBC9dnM

program: 6XwYJWNVgrmZuhYxPFy139P1MuXpVktpdHSLCKgmJQDR

governance program: GovER5Lthms3bLBqWub97yVrMmEogzX7xNjdXpPPCVZw

## Upload program

./scripts/change_program_id.sh
anchor deploy --provider.cluster devnet --program-keypair ./target/deploy/perpetuals-keypair.json

## Setup the Cortex

npx ts-node app/src/cli.ts -k adrena-keypairs/admin.json init\
 --min-signatures 1 \
 --lm-staking-reward-token-mint 4ZY3ZH8bStniqdCZdR14xsWW6vrMsCJrusobTdy4JipC \
 --governance-program GovER5Lthms3bLBqWub97yVrMmEogzX7xNjdXpPPCVZw \
 --governance-realm <> \
 --coreContributorBucketAllocation 100 \
 --daoTreasuryBucketAllocation 100 \
 --polBucketAllocation 100 \
 --ecosystemBucketAllocation 100 \
 6XwYJWNVgrmZuhYxPFy139P1MuXpVktpdHSLCKgmJQDR

## Setup the Pool

## Add Custodies

## Init LP Staking

##

---

## Not up to date

npx ts-node ./src/cli.ts -k ~/adrena-keypairs/admin.json get-multisig
npx ts-node ./src/cli.ts -k ~/adrena-keypairs/admin.json get-perpetuals

npx ts-node ./src/cli.ts -k ~/adrena-keypairs/admin.json add-pool pool3

npx ts-node src/cli.ts -k ~/adrena-keypairs/admin.json add-custody pool3 -t pyth -s 4ZY3ZH8bStniqdCZdR14xsWW6vrMsCJrusobTdy4JipC 5SSkXsEKQepHHAewytPVwdej4epN1nxgLVM84L4KXgy7

# ETH

npx ts-node src/cli.ts -k ~/adrena-keypairs/admin.json add-custody pool3 -t pyth 3AHAG1ZSUnPz43XBFKRqnLwhdyz29WhHvYQgVrcheCwr EdVCmQ9FSPcVe5YySXDPCRmc8aDQLKJ9xvYBMZPie1Vw

# BTC

npx ts-node src/cli.ts -k ~/adrena-keypairs/admin.json add-custody pool3 -t pyth HRvpfs8bKiUbLzSgT4LmKKugafZ8ePi5Vq7icJBC9dnM HovQMDrbAgAYPCmHVSrezcSmkMtXSSUsLDFANExrZh2J

# SOL

npx ts-node src/cli.ts -k ~/adrena-keypairs/admin.json add-custody pool3 -t pyth So11111111111111111111111111111111111111112 J83w4HKfqxwcq3BEMMkPFSppX3gqekLyLJBexebFVkix

npx ts-node src/cli.ts -k ~/adrena-keypairs/admin.json get-pool pool3
npx ts-node src/cli.ts -k ~/adrena-keypairs/admin.json get-custodies pool3

# Initialize lm token ATA

npx ts-node src/cli.ts -k ~/adrena-keypairs/admin.json get-lp-token-mint pool3
spl-token create-account 2fHi57To3JsfoJP967rH6e1cv6AvtWQarxy4yiv4FuYU --owner 5vAooJKJxWXVPNb13dBq1jPsuE3RTbMCfYuounMJcAvb --fee-payer ~/adrena-keypairs/admin.json

# Add USDC liquidity

npx ts-node src/cli.ts -k ~/adrena-keypairs/admin.json add-liquidity pool3 4ZY3ZH8bStniqdCZdR14xsWW6vrMsCJrusobTdy4JipC --amount-in 10000000 --min-amount-out 0

# Add ETH liquidity

npx ts-node src/cli.ts -k ~/adrena-keypairs/admin.json add-liquidity pool3 3AHAG1ZSUnPz43XBFKRqnLwhdyz29WhHvYQgVrcheCwr --amount-in 5000 --min-amount-out 0

# Add BTC liquidity

npx ts-node src/cli.ts -k ~/adrena-keypairs/admin.json add-liquidity pool3 HRvpfs8bKiUbLzSgT4LmKKugafZ8ePi5Vq7icJBC9dnM --amount-in 250 --min-amount-out 0

# Add SOL liquidity

npx ts-node src/cli.ts -k ~/adrena-keypairs/admin.json add-liquidity pool3 So11111111111111111111111111111111111111112 --amount-in 500000000 --min-amount-out 0

# How to use `setup_devnet` scripts

IF NO PROGRAM DEPLOYED ON DEVNET{
1/ execute `change_program_id.sh` to generate a new program id
2/ execute `anchor build` to update IDL
3/ verify you are on devnet, execute: `solana config get`
4/ execute `anchor deploy`
}

5/ change `ADRENA_PROGRAM_ID` variable in `setup_devnet.ts` and `LOCAL_WALLET_PATH`
6/ execute `ts-node ./script/setup_devnet.ts`

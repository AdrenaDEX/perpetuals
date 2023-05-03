import * as fs from "fs";
import { Keypair, PublicKey } from "@solana/web3.js";
import { BorshAccountsCoder, Wallet } from "@coral-xyz/anchor";
import bs58 from "bs58";
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";

export function loadWalletFromKeypair(path: string) {
  const secretKey = new Uint8Array(
    JSON.parse(fs.readFileSync(path).toString())
  );
  const walletKeypair = Keypair.fromSecretKey(secretKey);

  return new Wallet(walletKeypair);
}

export function findAccountDiscrimator(accountName: string) {
  return bs58.encode(BorshAccountsCoder.accountDiscriminator(accountName));
}

export function findAssociatedTokenAddress(
  owner: PublicKey,
  mint: PublicKey
): PublicKey {
  return PublicKey.findProgramAddressSync(
    [owner.toBuffer(), TOKEN_PROGRAM_ID.toBuffer(), mint.toBuffer()],
    ASSOCIATED_TOKEN_PROGRAM_ID
  )[0];
}

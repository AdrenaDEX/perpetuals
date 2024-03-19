import {
  AnchorProvider,
  BN,
  Program,
  ProgramAccount,
  Wallet,
} from "@coral-xyz/anchor";
import {
  Connection,
  PublicKey,
  ConfirmOptions,
  Commitment,
  KeyedAccountInfo,
  Context,
} from "@solana/web3.js";
import { Perpetuals, IDL } from "../target/types/perpetuals";
import { Position, Custody } from "./types";
import { findAccountDiscrimator, findAssociatedTokenAddress } from "./utils";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";

export default class Client {
  protected positionChangeSubscriptionId: number | null = null;

  constructor(
    public readonly programId: PublicKey,
    public readonly anchorProvider: AnchorProvider,
    public readonly program: Program<Perpetuals>
  ) {}

  public static initialize({
    programId,
    connection,
    wallet,
    confirmOptions,
  }: {
    programId: PublicKey;
    connection: Connection;
    wallet: Wallet;
    confirmOptions: ConfirmOptions;
  }) {
    const anchorProvider = new AnchorProvider(
      connection,
      wallet,
      confirmOptions
    );

    const program: Program<Perpetuals> = new Program<Perpetuals>(
      IDL,
      programId,
      anchorProvider
    );

    return new Client(programId, anchorProvider, program);
  }

  public async loadAllPositions(): Promise<ProgramAccount<Position>[]> {
    return this.program.account.position.all() as unknown as Promise<
      ProgramAccount<Position>[]
    >;
  }

  public async loadAllCustodies(): Promise<ProgramAccount<Custody>[]> {
    return this.program.account.custody.all() as unknown as Promise<
      ProgramAccount<Custody>[]
    >;
  }

  public unListenToPositionChange(): void {
    if (this.positionChangeSubscriptionId === null) {
      return;
    }

    this.anchorProvider.connection.removeProgramAccountChangeListener(
      this.positionChangeSubscriptionId
    );

    this.positionChangeSubscriptionId = null;
  }

  public listenToPositionChange(
    callbackFn: (pubkey: PublicKey, position: Position) => void,
    commitment?: Commitment
  ): void {
    if (this.positionChangeSubscriptionId) {
      throw new Error("Already subscribed to position change");
    }

    this.positionChangeSubscriptionId =
      this.anchorProvider.connection.onProgramAccountChange(
        this.programId,
        (keyedAccountInfo: KeyedAccountInfo, _: Context) => {
          const position = this.program.coder.accounts.decode<Position>(
            "position",
            keyedAccountInfo.accountInfo.data
          );

          callbackFn(keyedAccountInfo.accountId, position);
        },
        commitment,
        [
          {
            memcmp: {
              offset: 0,
              bytes: findAccountDiscrimator("Position"),
            },
          },
        ]
      );
  }

  public async getLiquidationPrice(
    pubkey: PublicKey,
    position: Position,
    custody: Custody
  ): Promise<number | null> {
    if (!this.program.views) {
      throw new Error("Cannot get liquidation price");
    }

    const liquidationPrice = await this.program.views.getLiquidationPrice(
      {
        addCollateral: new BN(0),
        removeCollateral: new BN(0),
      },
      {
        accounts: {
          perpetuals: this.getPerpetualsPda(),
          pool: position.pool,
          position: pubkey,
          custody: position.custody,
          custodyOracleAccount: custody.oracle.oracleAccount,
        },
      }
    );

    if (liquidationPrice === null) {
      return null;
    }

    return liquidationPrice.toNumber() / 10 ** custody.decimals;
  }

  // Return true if position got liquidated, false otherwise
  public async tryAndLiquidatePosition({
    pubkey,
    position,
    custody,
  }: {
    pubkey: PublicKey;
    position: Position;
    custody: Custody;
  }): Promise<boolean> {
    // Position owner receive the funds
    const receivingAccount = findAssociatedTokenAddress(
      position.owner,
      custody.mint
    );

    // Liquidator receives rewards for calling liquidation ix
    const rewardsReceivingAccount = findAssociatedTokenAddress(
      this.anchorProvider.wallet.publicKey,
      custody.mint
    );

    const accounts = {
      signer: this.anchorProvider.wallet.publicKey,
      receivingAccount,
      rewardsReceivingAccount,
      transferAuthority: this.getTransferAuthorityPda(),
      perpetuals: this.getPerpetualsPda(),
      pool: position.pool,
      position: pubkey,
      custody: position.custody,
      custodyOracleAccount: custody.oracle.oracleAccount,
      custodyTokenAccount: custody.tokenAccount,
      tokenProgram: TOKEN_PROGRAM_ID,
    };

    try {
      await this.program.methods.liquidate({}).accounts(accounts).simulate();

      // Simulation worked, do liquidate now
      await this.program.methods.liquidate({}).accounts(accounts).rpc();

      return true;
    } catch {
      // Position is not ready to be liquidated or already did
      // Ignore
      return false;
    }
  }

  protected getTransferAuthorityPda(): PublicKey {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("transfer_authority")],
      this.programId
    )[0];
  }

  protected getPerpetualsPda(): PublicKey {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("perpetuals")],
      this.programId
    )[0];
  }
}

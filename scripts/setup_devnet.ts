import { AnchorProvider, BN, Program } from "@project-serum/anchor";
import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
} from "@solana/web3.js";
import NodeWallet from "@project-serum/anchor/dist/cjs/nodewallet";
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  createAssociatedTokenAccountIdempotent,
  createMint,
  mintTo,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import fs from "fs";
import {
  IDL as adrenaIDL,
  Perpetuals as Adrena,
} from "../target/types/perpetuals";

//
// Configuration
//

const ADRENA_PROGRAM_ID = new PublicKey(
  "E62QGi9PKWn1FHyRHxo4LwuKyQN3UNJjNZqrDoVmYaKK"
);

const LOCAL_WALLET_PATH = "/Users/orex/.config/solana/id.json";

//
// Setup spl-tokens and plug them on devnet pyth oracles
//

function explorer(tx: string): string {
  return `https://explorer.solana.com/tx/${tx}?cluster=devnet`;
}

export function findATAAddressSync(
  wallet: PublicKey,
  mint: PublicKey
): PublicKey {
  return PublicKey.findProgramAddressSync(
    [wallet.toBuffer(), TOKEN_PROGRAM_ID.toBuffer(), mint.toBuffer()],
    ASSOCIATED_TOKEN_PROGRAM_ID
  )[0];
}

const RPC = "https://api.devnet.solana.com";

type Token = "ETH" | "BTC" | "SOL" | "USDC";
type DummyUser = "alice" | "martin" | "kevin" | "anna";

async function main() {
  const connection = new Connection(RPC, "processed");

  // Local wallet
  const adminKeypair = Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(fs.readFileSync(LOCAL_WALLET_PATH).toString()))
  );

  console.log("adminKeypair", adminKeypair.publicKey.toBase58());

  const dummyUsers: Record<DummyUser, Keypair> = {
    // ArJJRUcbSrqqMuTrMdQNvBsXNokQBh3VCt2EDrg2aqnc
    alice: Keypair.fromSecretKey(
      new Uint8Array([
        167, 32, 131, 64, 97, 100, 87, 105, 209, 118, 29, 178, 217, 123, 114,
        40, 196, 193, 180, 105, 185, 80, 124, 84, 168, 202, 23, 49, 112, 69, 30,
        91, 146, 90, 52, 30, 6, 41, 130, 214, 186, 233, 152, 141, 84, 171, 172,
        247, 1, 141, 245, 222, 95, 40, 216, 110, 108, 55, 234, 25, 254, 119,
        243, 237,
      ])
    ),
    // 7KQ7YgFmP4mz4p1UQnwrESZ58APxDZUoWyN5GXeXm4cz
    martin: Keypair.fromSecretKey(
      new Uint8Array([
        4, 134, 122, 99, 223, 171, 228, 210, 126, 150, 104, 187, 157, 190, 253,
        66, 224, 76, 34, 133, 25, 209, 222, 28, 34, 29, 194, 127, 104, 33, 176,
        134, 93, 220, 181, 189, 95, 247, 41, 246, 30, 246, 187, 226, 134, 42,
        31, 83, 154, 77, 109, 198, 117, 236, 214, 193, 44, 175, 205, 137, 59,
        122, 93, 115,
      ])
    ),
    // 37dThGRY77tkRrnzgbfAkZaNnCQzGdQWnLCE9UD4MuPy
    kevin: Keypair.fromSecretKey(
      new Uint8Array([
        51, 48, 148, 183, 116, 191, 169, 151, 38, 187, 128, 187, 66, 233, 244,
        167, 205, 83, 12, 116, 63, 199, 105, 161, 23, 160, 55, 215, 168, 144,
        44, 176, 31, 106, 21, 157, 189, 189, 112, 40, 146, 15, 20, 15, 24, 136,
        131, 62, 117, 24, 58, 166, 163, 130, 49, 221, 231, 170, 32, 183, 194,
        20, 114, 20,
      ])
    ),
    // 4kNKao2wpEawMJa1yp1WDnbKTFHsuVaCevrDdF8M3R2Y
    anna: Keypair.fromSecretKey(
      new Uint8Array([
        203, 10, 6, 239, 82, 22, 190, 22, 228, 58, 196, 241, 176, 203, 59, 80,
        157, 85, 73, 39, 183, 140, 156, 162, 53, 172, 237, 239, 180, 105, 89,
        228, 55, 175, 36, 130, 51, 32, 231, 33, 198, 129, 66, 106, 132, 112, 86,
        251, 45, 192, 11, 34, 202, 44, 192, 249, 57, 21, 51, 130, 185, 160, 48,
        41,
      ])
    ),
  };

  const mints: Record<Token, Keypair> = {
    // 4ZY3ZH8bStniqdCZdR14xsWW6vrMsCJrusobTdy4JipC
    USDC: Keypair.fromSecretKey(
      new Uint8Array([
        67, 205, 106, 120, 43, 207, 109, 34, 35, 89, 82, 105, 249, 202, 97, 207,
        143, 178, 84, 131, 111, 247, 173, 108, 137, 194, 232, 32, 160, 155, 42,
        212, 52, 232, 191, 145, 234, 195, 31, 75, 183, 114, 247, 19, 71, 30,
        111, 17, 248, 206, 76, 240, 203, 240, 249, 225, 49, 152, 107, 39, 51,
        88, 28, 77,
      ])
    ),
    // 3AHAG1ZSUnPz43XBFKRqnLwhdyz29WhHvYQgVrcheCwr
    ETH: Keypair.fromSecretKey(
      new Uint8Array([
        74, 245, 161, 250, 191, 117, 125, 105, 226, 27, 85, 37, 12, 20, 69, 231,
        89, 0, 128, 213, 240, 136, 124, 136, 54, 197, 174, 120, 145, 16, 255,
        209, 32, 23, 223, 117, 202, 78, 58, 90, 195, 163, 130, 84, 254, 151,
        173, 116, 79, 133, 193, 99, 250, 108, 17, 153, 213, 127, 110, 182, 247,
        157, 253, 97,
      ])
    ),
    // HRvpfs8bKiUbLzSgT4LmKKugafZ8ePi5Vq7icJBC9dnM
    BTC: Keypair.fromSecretKey(
      new Uint8Array([
        207, 136, 187, 145, 213, 240, 93, 106, 219, 102, 25, 13, 64, 224, 197,
        222, 174, 230, 139, 190, 87, 151, 162, 116, 146, 186, 112, 16, 99, 29,
        67, 145, 244, 29, 201, 111, 56, 105, 171, 8, 11, 114, 140, 1, 17, 171,
        126, 109, 39, 235, 119, 114, 131, 69, 123, 67, 158, 232, 189, 253, 12,
        202, 187, 70,
      ])
    ),
    // EtX1Uagb44Yp5p4hsqjwAwF3mKaQTMizCyvC1CsyHAQN
    SOL: Keypair.fromSecretKey(
      new Uint8Array([
        33, 138, 133, 160, 211, 125, 21, 178, 4, 64, 232, 0, 199, 223, 74, 1,
        209, 246, 43, 168, 114, 206, 215, 5, 19, 214, 138, 191, 151, 239, 182,
        208, 206, 90, 123, 79, 163, 184, 219, 6, 77, 255, 144, 195, 173, 205,
        124, 106, 94, 180, 3, 70, 32, 91, 93, 230, 86, 0, 113, 79, 139, 99, 12,
        143,
      ])
    ),
  };

  const adrenaProgram = new Program<Adrena>(
    adrenaIDL,
    ADRENA_PROGRAM_ID,
    new AnchorProvider(connection, new NodeWallet(adminKeypair), {
      commitment: "confirmed",
    })
  );

  const [multisig] = PublicKey.findProgramAddressSync(
    [Buffer.from("multisig")],
    ADRENA_PROGRAM_ID
  );

  const [transferAuthority] = PublicKey.findProgramAddressSync(
    [Buffer.from("transfer_authority")],
    ADRENA_PROGRAM_ID
  );

  const [perpetuals] = PublicKey.findProgramAddressSync(
    [Buffer.from("perpetuals")],
    ADRENA_PROGRAM_ID
  );

  const [adrenaProgramData] = PublicKey.findProgramAddressSync(
    [ADRENA_PROGRAM_ID.toBuffer()],
    new PublicKey("BPFLoaderUpgradeab1e11111111111111111111111")
  );

  //
  // Setup `Perpetuals` account
  //

  let perpetualsAccount = await adrenaProgram.account.perpetuals.fetchNullable(
    perpetuals
  );

  if (!perpetualsAccount) {
    const tx = await adrenaProgram.methods
      .init({
        minSignatures: 1,
        allowSwap: true,
        allowAddLiquidity: true,
        allowRemoveLiquidity: true,
        allowOpenPosition: true,
        allowClosePosition: true,
        allowPnlWithdrawal: true,
        allowCollateralWithdrawal: true,
        allowSizeChange: true,
      })
      .accounts({
        upgradeAuthority: adminKeypair.publicKey,
        multisig,
        transferAuthority,
        perpetuals,
        perpetualsProgram: ADRENA_PROGRAM_ID,
        perpetualsProgramData: adrenaProgramData,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .remainingAccounts([
        {
          pubkey: adminKeypair.publicKey,
          isSigner: true,
          isWritable: false,
        },
      ])
      .signers([adminKeypair])
      .rpc();

    console.log("Perpetuals account initialization tx:", explorer(tx));

    perpetualsAccount = await adrenaProgram.account.perpetuals.fetch(
      perpetuals
    );
  } else {
    console.log(
      "Perpetuals account already initialized:",
      perpetuals.toBase58()
    );
  }

  //
  // Setup `Pool` account with newly created spl-tokens as collateral
  //

  const mainPoolName = "adrena";

  const [mainPool] = PublicKey.findProgramAddressSync(
    [Buffer.from("pool"), Buffer.from(mainPoolName)],
    ADRENA_PROGRAM_ID
  );

  const [lpTokenMint] = PublicKey.findProgramAddressSync(
    [Buffer.from("lp_token_mint"), mainPool.toBuffer()],
    ADRENA_PROGRAM_ID
  );

  let mainPoolAccount = await adrenaProgram.account.pool.fetchNullable(
    mainPool
  );

  //
  // Setup spl-tokens accounts
  //

  if (!(await connection.getAccountInfo(mints.USDC.publicKey))) {
    await Promise.all(
      Object.values(mints).map((mintKeypair) =>
        createMint(
          connection,
          adminKeypair,
          adminKeypair.publicKey,
          adminKeypair.publicKey,
          6,
          mintKeypair
        )
      )
    );

    console.log(
      Object.entries(mints).map(
        ([key, value]) =>
          `${key} mint initialized: [${value.secretKey.toString()}] ${value.publicKey.toBase58()}`
      )
    );

    // Mint 500k of each tokens to dummy users
    await Promise.all(
      Object.entries(dummyUsers).reduce(
        (promises, [dummyUserName, { publicKey: dummyUserWallet }]) => {
          console.log(
            "Mint 500k of each tokens to",
            dummyUserName,
            "wallet",
            dummyUserWallet.toBase58()
          );

          return [
            ...promises,
            ...Object.values(mints).map(async (mintKeypair) => {
              await createAssociatedTokenAccountIdempotent(
                connection,
                adminKeypair,
                mintKeypair.publicKey,
                dummyUserWallet
              );

              const ata = findATAAddressSync(
                dummyUserWallet,
                mintKeypair.publicKey
              );

              return mintTo(
                connection,
                adminKeypair,
                mintKeypair.publicKey,
                ata,
                adminKeypair,
                500_000 * 10 ** 6
              );
            }),
          ];
        },
        [] as Promise<void>[]
      )
    );
  } else {
    console.log(
      Object.entries(mints).map(
        ([key, value]) => `${key} mint: ${value.publicKey.toBase58()}`
      )
    );
  }

  // You can uncomment the following, replace the pubkey and execute to mint tokens to dev account
  /*
  const devWallet = new PublicKey("6hqz24NfaMwEvUna95p7haPqrh2urVwyVo1gLHEqUVXY");
  console.log(
    "Mint 500k of each tokens to developer wallet",
    devWallet.toBase58(),
  );
  await Promise.all(
    Object.values(mints).map(async (mintKeypair) => {
      await createAssociatedTokenAccountIdempotent(
        connection,
        adminKeypair,
        mintKeypair.publicKey,
        devWallet,
      );

      const ata = findATAAddressSync(
        devWallet,
        mintKeypair.publicKey
      );

      return mintTo(
        connection,
        adminKeypair,
        mintKeypair.publicKey,
        ata,
        adminKeypair,
        500_000 * 10 ** 6
      );
    })
  );
  */

  if (!mainPoolAccount) {
    //
    // Setup pool
    //

    const tx = await adrenaProgram.methods
      .addPool({
        name: mainPoolName,
      })
      .accounts({
        admin: adminKeypair.publicKey,
        multisig,
        transferAuthority,
        perpetuals,
        pool: mainPool,
        lpTokenMint,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .signers([adminKeypair])
      .rpc();

    console.log("Main pool account initialization tx:", explorer(tx));

    mainPoolAccount = await adrenaProgram.account.pool.fetch(mainPool);
  } else {
    console.log("Main pool account already initialized:", mainPool.toBase58());
  }

  //
  // Add pool custodies
  //

  const tokenInfos = {
    USDC: {
      isStable: true,
      pythOracle: new PublicKey("5SSkXsEKQepHHAewytPVwdej4epN1nxgLVM84L4KXgy7"),
    },
    ETH: {
      isStable: false,
      pythOracle: new PublicKey("EdVCmQ9FSPcVe5YySXDPCRmc8aDQLKJ9xvYBMZPie1Vw"),
    },
    BTC: {
      isStable: false,
      pythOracle: new PublicKey("HovQMDrbAgAYPCmHVSrezcSmkMtXSSUsLDFANExrZh2J"),
    },
    SOL: {
      isStable: false,
      pythOracle: new PublicKey("J83w4HKfqxwcq3BEMMkPFSppX3gqekLyLJBexebFVkix"),
    },
  } as const;

  // Due to how addCustody works, ratio targets must always worth 1
  // You gotta have to adapt ratios each time you add a new custody
  const ratiosHistory = [
    [
      {
        // USDC
        target: new BN(10_000),
        min: new BN(0), // 0%
        max: new BN(10_000), // 100%
      },
    ],
    [
      {
        // USDC
        target: new BN(10_000),
        min: new BN(0), // 0%
        max: new BN(10_000), // 100%
      },
      {
        // ETH
        target: new BN(0),
        min: new BN(0), // 0%
        max: new BN(10_000), // 100%
      },
    ],
    [
      {
        // USDC
        target: new BN(10_000),
        min: new BN(0), // 0%
        max: new BN(10_000), // 100%
      },
      {
        // ETH
        target: new BN(0),
        min: new BN(0), // 0%
        max: new BN(10_000), // 100%
      },
      {
        // BTC
        target: new BN(0),
        min: new BN(0), // 0%
        max: new BN(10_000), // 100%
      },
    ],
    [
      // LAST ONE
      {
        // USDC
        target: new BN(3_000), // 30%
        min: new BN(0), // 0%
        max: new BN(10_000), // 100%
      },
      {
        // ETH
        target: new BN(2_000), // 20%
        min: new BN(0), // 0%
        max: new BN(10_000), // 100%
      },
      {
        // BTC
        target: new BN(2_000), // 20%
        min: new BN(0), // 0%
        max: new BN(10_000), // 100%
      },
      {
        // SOL
        target: new BN(3_000), // 30%
        min: new BN(0), // 0%
        max: new BN(10_000), // 100%
      },
    ],
  ];

  let i = 0;
  for await (const [tokenName, mintKeypair] of Object.entries(mints)) {
    const [custody] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("custody"),
        mainPool.toBuffer(),
        mintKeypair.publicKey.toBuffer(),
      ],
      ADRENA_PROGRAM_ID
    );

    const [custodyTokenAccount] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("custody_token_account"),
        mainPool.toBuffer(),
        mintKeypair.publicKey.toBuffer(),
      ],
      ADRENA_PROGRAM_ID
    );

    let custodyAccount = await adrenaProgram.account.custody.fetchNullable(
      custody
    );

    if (!custodyAccount) {
      const { isStable, pythOracle } = tokenInfos[tokenName];

      // Setup custody account
      const tx = await adrenaProgram.methods
        .addCustody({
          isStable,
          oracle: {
            oracleAccount: pythOracle as PublicKey,
            oracleType: { pyth: {} },
            maxPriceError: new BN(1_000_000),
            maxPriceAgeSec: 30,
          },
          pricing: {
            useEma: false,
            useUnrealizedPnlInAum: true,
            tradeSpreadLong: new BN(100),
            tradeSpreadShort: new BN(100),
            swapSpread: new BN(300),
            minInitialLeverage: new BN(10_000),
            maxInitialLeverage: new BN(500_000),
            maxLeverage: new BN(500_000),
            maxPayoffMult: new BN(10_000),
            maxUtilization: new BN(0),
            maxPositionLockedUsd: new BN(0),
            maxTotalLockedUsd: new BN(0),
          },
          permissions: {
            allowSwap: true,
            allowAddLiquidity: true,
            allowRemoveLiquidity: true,
            allowOpenPosition: true,
            allowClosePosition: true,
            allowPnlWithdrawal: true,
            allowCollateralWithdrawal: true,
            allowSizeChange: true,
          },
          fees: {
            mode: { linear: {} },
            ratioMult: new BN(20_000),
            utilizationMult: new BN(20_000),
            swapIn: new BN(100),
            swapOut: new BN(100),
            stableSwapIn: new BN(100),
            stableSwapOut: new BN(100),
            addLiquidity: new BN(100),
            removeLiquidity: new BN(100),
            openPosition: new BN(100),
            closePosition: new BN(100),
            liquidation: new BN(100),
            protocolShare: new BN(10),
          },
          borrowRate: {
            baseRate: new BN(0),
            slope1: new BN(80_000),
            slope2: new BN(120_000),
            optimalUtilization: new BN(800_000_000),
          },
          ratios: ratiosHistory[i++],
        } as any)
        .accounts({
          admin: adminKeypair.publicKey,
          multisig,
          transferAuthority,
          perpetuals,
          pool: mainPool,
          custody,
          custodyTokenAccount,
          custodyTokenMint: mintKeypair.publicKey,
          systemProgram: SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          rent: SYSVAR_RENT_PUBKEY,
        })
        .signers([adminKeypair])
        .rpc();

      console.log(`Add ${tokenName} custody tx:`, explorer(tx));
    } else {
      console.log(tokenName, "Custody already setup", custody.toBase58());
    }
  }

  //
  // Add liquidity from dummy wallets
  //

  const addLiquidityIfNotAlready = async (
    dummyUserName: DummyUser,
    token: Token,
    amount: BN
  ) => {
    const userKeypair = dummyUsers[dummyUserName];
    const mintPubkey = mints[token].publicKey;

    const fundingAccount = findATAAddressSync(
      userKeypair.publicKey,
      mintPubkey
    );
    const lpTokenAccount = findATAAddressSync(
      userKeypair.publicKey,
      lpTokenMint
    );

    const [custody] = PublicKey.findProgramAddressSync(
      [Buffer.from("custody"), mainPool.toBuffer(), mintPubkey.toBuffer()],
      ADRENA_PROGRAM_ID
    );

    const [custodyTokenAccount] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("custody_token_account"),
        mainPool.toBuffer(),
        mintPubkey.toBuffer(),
      ],
      ADRENA_PROGRAM_ID
    );

    await createAssociatedTokenAccountIdempotent(
      connection,
      adminKeypair,
      lpTokenMint,
      userKeypair.publicKey
    );

    const currentLpTokenBalance = await connection.getTokenAccountBalance(
      lpTokenAccount
    );

    if (currentLpTokenBalance.value.uiAmount > 0) {
      console.log(
        dummyUserName,
        `(${dummyUsers[
          dummyUserName
        ].publicKey.toBase58()}) already have LP tokens, thus have already provided liquidity`
      );

      return;
    }

    const custodiesAccounts = await adrenaProgram.account.custody.fetchMultiple(
      mainPoolAccount.custodies
    );

    const tx = await adrenaProgram.methods
      .addLiquidity({
        amountIn: amount,
        minLpAmountOut: new BN(0),
      })
      .accounts({
        owner: userKeypair.publicKey,
        fundingAccount,
        lpTokenAccount,
        transferAuthority,
        perpetuals,
        pool: mainPool,
        custody,
        custodyOracleAccount: tokenInfos[token].pythOracle,
        custodyTokenAccount,
        lpTokenMint,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .remainingAccounts([
        // needs to provide all custodies and theirs oracles
        ...mainPoolAccount.custodies.map((custody) => ({
          pubkey: custody,
          isSigner: false,
          isWritable: false,
        })),

        ...mainPoolAccount.custodies.map((_, index) => ({
          pubkey: (custodiesAccounts[index] as any).oracle.oracleAccount,
          isSigner: false,
          isWritable: false,
        })),
      ])
      .signers([userKeypair])
      .rpc();

    console.log(
      dummyUserName,
      " adds ",
      amount.toString(),
      "liquidity, tx:",
      explorer(tx)
    );
  };

  // Get the latest version of pool data
  //
  mainPoolAccount = await adrenaProgram.account.pool.fetch(mainPool);

  // 10 BTC
  await addLiquidityIfNotAlready("alice", "BTC", new BN(10 * 10 ** 6));

  // 150 ETH
  await addLiquidityIfNotAlready("martin", "ETH", new BN(150 * 10 ** 6));

  // 10_000 SOL
  await addLiquidityIfNotAlready("kevin", "SOL", new BN(10_000 * 10 ** 6));

  // 200_000 USDC
  await addLiquidityIfNotAlready("anna", "USDC", new BN(200_000 * 10 ** 6));
}

main();

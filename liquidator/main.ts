import { Connection, PublicKey } from "@solana/web3.js";
import Client from "./Client";
import { loadWalletFromKeypair } from "./utils";
import LiquidatorBot from "./LiquidatorBot";
import PriceFeed from "./PriceFeed";
import PositionsBank from "./PositionsBank";

const RPC = "https://api.devnet.solana.com";

// @TODO
//
// Makes the following as params:
// - RPC
// - Wallet keypair path
// - Program ID
// - Cluster
// - Price Refresh Time in Ms
// - Symbol + Pyth Price Accounts
//
// Catch CTRL+C signal to stop the script

async function main() {
  const connection = new Connection(RPC, "processed");

  const wallet = loadWalletFromKeypair("/Users/orex/.config/solana/id.json");

  const programId = new PublicKey(
    "H1byJyMjQ3gUrtrrav5FV7yV3Jo6pT8YePtNkdGgoa1P"
  );

  const client = Client.initialize({
    programId,
    connection,
    wallet,
    confirmOptions: {},
  });

  const priceFeed = PriceFeed.initialize({
    connection,
    cluster: "devnet",
    priceAccounts: {
      // SOL_USD
      // Mint here is the mint the price is about (fake SOL here)
      EtX1Uagb44Yp5p4hsqjwAwF3mKaQTMizCyvC1CsyHAQN: new PublicKey(
        "J83w4HKfqxwcq3BEMMkPFSppX3gqekLyLJBexebFVkix"
      ),

      // ETH_USD
      "3AHAG1ZSUnPz43XBFKRqnLwhdyz29WhHvYQgVrcheCwr": new PublicKey(
        "EdVCmQ9FSPcVe5YySXDPCRmc8aDQLKJ9xvYBMZPie1Vw"
      ),

      // BTC_USD
      HRvpfs8bKiUbLzSgT4LmKKugafZ8ePi5Vq7icJBC9dnM: new PublicKey(
        "HovQMDrbAgAYPCmHVSrezcSmkMtXSSUsLDFANExrZh2J"
      ),
    },
    refreshTimeInMs: 2_000,
    commitment: "processed",
  });

  const positionsBank = PositionsBank.initialize({ client });

  const liquidator = await LiquidatorBot.initialize({
    client,
    priceFeed,
    positionsBank,
  });

  await liquidator.start();
}

main();

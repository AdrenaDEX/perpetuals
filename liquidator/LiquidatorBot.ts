import { PublicKey } from "@solana/web3.js";
import Client from "./Client";
import PositionsBank from "./PositionsBank";
import PriceFeed, { Symbol } from "./PriceFeed";
import { Position } from "./types";
import { BN } from "@coral-xyz/anchor";

export default class LiquidatorBot {
  constructor(
    public readonly client: Client,
    public readonly priceFeed: PriceFeed,
    public readonly positionsBank: PositionsBank
  ) {}

  // Keep liquidation price for each positions
  public positionsLiquidationPriceBank: Record<string, BN> = {};

  public static initialize({
    client,
    priceFeed,
    positionsBank,
  }: {
    client: Client;
    priceFeed: PriceFeed;
    positionsBank: PositionsBank;
  }) {
    return new LiquidatorBot(client, priceFeed, positionsBank);
  }

  // Get triggered every time a price changes
  protected priceChangeFn(prices: Record<Symbol, number>) {
    // Check the price of every positions
    // ...
  }

  // Get triggered every time a position account change
  protected positionChangeFn(pubkey: PublicKey, position: Position) {
    // Update the liquidation price and re-evaluate
    // ...
  }

  public async start(): Promise<void> {
    await Promise.all([
      this.priceFeed.startListening(this.priceChangeFn),
      this.positionsBank.start(this.positionChangeFn),
    ]);

    // Load positions liquidation prices for each positions in the bank
    // ...
  }

  public async stop(): Promise<void> {
    await this.positionsBank.stop();
    this.priceFeed.stopListening();
  }
}

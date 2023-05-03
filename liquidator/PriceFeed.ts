import {
  PriceData,
  PythCluster,
  PythHttpClient,
  getPythProgramKeyForCluster,
} from "@pythnetwork/client";
import { Commitment, Connection, PublicKey } from "@solana/web3.js";

export type Symbol = string;
export type PriceChangeTriggerFn = (
  pricesCache: Record<Symbol, number | null>
) => void;

// Load prices periodically and trigger callback function
// when price change is detected
//
// Pros:
// - Limit RPC usage (using PythConnection uses multiple request/seconds/token)
// - Triggers only when price change to limit checks
export default class PriceFeed {
  protected loading: boolean = false;

  public pricesCache: Record<Symbol, number | null> = {};

  constructor(
    public readonly pythHttpClient: PythHttpClient,
    public readonly priceAccounts: Record<Symbol, PublicKey>,
    public readonly refreshTimeInMs: number
  ) {
    this.pricesCache = Object.keys(priceAccounts).reduce(
      (acc, symbol) => ({
        ...acc,
        [symbol]: null,
      }),
      {}
    );
  }

  public static initialize({
    connection,
    cluster,
    priceAccounts,
    refreshTimeInMs,
    commitment,
  }: {
    connection: Connection;
    cluster: PythCluster;
    priceAccounts: Record<Symbol, PublicKey>;
    refreshTimeInMs: number;
    commitment?: Commitment;
  }) {
    const pythHttpClient = new PythHttpClient(
      connection,
      getPythProgramKeyForCluster(cluster),
      commitment
    );

    return new PriceFeed(pythHttpClient, priceAccounts, refreshTimeInMs);
  }

  protected async loadPriceIndefinitely(
    priceChangeFn: PriceChangeTriggerFn
  ): Promise<void> {
    this.loading = true;

    const priceAccountsArray = Object.entries(this.priceAccounts);

    const priceDataArray = await this.pythHttpClient.getAssetPricesFromAccounts(
      priceAccountsArray.map(([_, priceAccount]) => priceAccount)
    );

    if (!this.loading) return;

    // Adapt price cache
    const priceChanged = priceDataArray.reduce(
      (priceChanged, priceData: PriceData, index: number) => {
        const lowestPrice = priceData.price - priceData.confidence;

        const [symbol, _] = priceAccountsArray[index];

        // price is the same, nothing to do
        if (this.pricesCache[symbol] == lowestPrice) {
          return priceChanged;
        }

        this.pricesCache[symbol] = lowestPrice;

        return true;
      },
      false
    );

    if (priceChanged) {
      priceChangeFn(this.pricesCache);
    }

    setTimeout(() => {
      this.loadPriceIndefinitely(priceChangeFn);
    }, this.refreshTimeInMs);
  }

  // Triggers the callback function everytime a price change
  public startListening(priceChangeFn: PriceChangeTriggerFn): Promise<void> {
    return this.loadPriceIndefinitely(priceChangeFn);
  }

  public stopListening() {
    this.loading = false;
  }
}

import { PublicKey } from "@solana/web3.js";
import Client from "./Client";
import PositionsBank, { PublicKeyString } from "./PositionsBank";
import PriceFeed, { Mint } from "./PriceFeed";
import { Custody, Position } from "./types";
import { ProgramAccount } from "@coral-xyz/anchor";

export default class LiquidatorBot {
  // Keep liquidation price for each positions
  public positionsLiquidationPriceBank: Record<
    PublicKeyString,
    {
      price: number | null;

      // Use an index to handle multiple async loading for same position
      loadingIndex: number | null;
    }
  > = {};

  constructor(
    public readonly client: Client,
    public readonly priceFeed: PriceFeed,
    public readonly positionsBank: PositionsBank,
    public readonly custodies: ProgramAccount<Custody>[]
  ) {}

  public static async initialize({
    client,
    priceFeed,
    positionsBank,
  }: {
    client: Client;
    priceFeed: PriceFeed;
    positionsBank: PositionsBank;
  }) {
    const custodies = await client.loadAllCustodies();

    console.log(
      `Custodies: [${custodies
        .map(({ publicKey }) => publicKey.toBase58())
        .join(", ")}]`
    );

    return new LiquidatorBot(client, priceFeed, positionsBank, custodies);
  }

  // Get triggered every time a price changes
  protected priceChangeFn(pricesCache: Record<Mint, number | null>) {
    // Position bank not ready
    if (!this.positionsBank) {
      return;
    }

    // Check the price of every positions
    const elligibleToLiquation = Object.entries(this.positionsBank.bank).reduce(
      (list, [pubkey, position]) => {
        // Check price vs liquidationPrice

        // Liquidation price not available yet
        if (
          !this.positionsLiquidationPriceBank[pubkey] ||
          !this.positionsLiquidationPriceBank[pubkey].price === null
        ) {
          return list;
        }

        const liquidationPrice =
          this.positionsLiquidationPriceBank[pubkey].price;

        // Liquidation price not available yet
        if (liquidationPrice === null) {
          return list;
        }

        const custody = this.custodies.find(({ publicKey }) =>
          publicKey.equals(position.custody)
        );

        // Cannot found custody
        if (!custody) {
          return list;
        }

        const markPrice = pricesCache[custody.account.mint.toBase58()];

        // Mark price not available
        if (markPrice === null || typeof markPrice === "undefined") {
          return list;
        }

        if (position.side.long) {
          if (liquidationPrice >= markPrice) {
            // Position may be liquidable
            console.log("Position may be liquidable", {
              side: position.side,
              liquidationPrice,
              markPrice,
            });
          }
          return list;
        }

        if (position.side.short) {
          if (liquidationPrice <= markPrice) {
            // Position may be liquidable
            console.log("Position may be liquidable", {
              side: position.side,
              liquidationPrice,
              markPrice,
            });
          }
          return list;
        }

        return list;
      },
      []
    );
  }

  // Get triggered every time a position account change
  protected async positionChangeFn(pubkey: PublicKey, position: Position) {
    // Update the liquidation price
    await this.loadPositionLiquidationPrice(pubkey, position);

    // Re-evaluate the liquidation with actual oracle price
    // ...
  }

  protected async loadPositionLiquidationPrice(
    pubkey: PublicKey,
    position: Position
  ) {
    // Generate a random loading index to identify this load in particual
    const loadingIndex = Math.random();

    if (!this.positionsLiquidationPriceBank[pubkey.toBase58()]) {
      this.positionsLiquidationPriceBank[pubkey.toBase58()] = {
        price: null,
        loadingIndex,
      };
    } else {
      this.positionsLiquidationPriceBank[pubkey.toBase58()].loadingIndex =
        loadingIndex;
    }

    let custody = this.custodies.find(({ publicKey }) =>
      publicKey.equals(position.custody)
    );

    if (!custody) {
      console.log("Unidentified custody", position.custody.toBase58());

      // Custody must have been created after script got launched
      const newCustody =
        await this.client.program.account.custody.fetchNullable(
          position.custody
        );

      if (!newCustody) {
        console.log("Position with unknown custody", {
          position: pubkey.toBase58(),
          custody: position.custody.toBase58(),
        });
        return;
      }

      this.custodies.push({
        publicKey: position.custody,
        account: newCustody,
      });

      custody = {
        publicKey: position.custody,
        account: newCustody,
      };
    }

    try {
      const price = await this.client.getLiquidationPrice(
        new PublicKey(pubkey),
        position,
        custody.account
      );

      if (price === null) {
        throw new Error("Cannot load liquidation error");
      }

      // Loading is not up-to-date anymore, discard
      if (
        this.positionsLiquidationPriceBank[pubkey.toBase58()].loadingIndex !==
        loadingIndex
      ) {
        console.log("Discard due loadingIndex");
        return;
      }

      console.log(
        "Set position liquidation price",
        pubkey.toBase58(),
        price.toLocaleString()
      );

      this.positionsLiquidationPriceBank[pubkey.toBase58()] = {
        price,
        loadingIndex: null,
      };
    } catch {
      // Loading is not up-to-date anymore, discard
      if (
        this.positionsLiquidationPriceBank[pubkey.toBase58()].loadingIndex !==
        loadingIndex
      ) {
        return;
      }

      this.positionsLiquidationPriceBank[pubkey.toBase58()].loadingIndex = null;
    }
  }

  public async start(): Promise<void> {
    await Promise.all([
      this.priceFeed.startListening(this.priceChangeFn.bind(this)),
      this.positionsBank.start(this.positionChangeFn.bind(this)),
    ]);

    // Load positions liquidation prices for each positions in the bank
    await Promise.all(
      Object.entries(this.positionsBank.bank).map(([pubkey, position]) =>
        this.loadPositionLiquidationPrice(new PublicKey(pubkey), position)
      )
    );
  }

  public async stop(): Promise<void> {
    await this.positionsBank.stop();
    this.priceFeed.stopListening();
  }
}

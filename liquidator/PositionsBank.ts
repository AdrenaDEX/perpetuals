import { PublicKey } from "@solana/web3.js";
import Client from "./Client";
import { Position } from "./types";

export type PositionChangeTriggerFn = (
  pubkey: PublicKey,
  position: Position
) => void;

// Keep up to date list of Positions
export default class PositionsBank {
  public bank: Record<string, Position> = {};

  constructor(public readonly client: Client) {}

  public static initialize({ client }: { client: Client }) {
    return new PositionsBank(client);
  }

  public async start(positionChangeTriggerFn: PositionChangeTriggerFn) {
    this.client.listenToPositionChange(
      (pubkey: PublicKey, position: Position) => {
        console.log("Change happened to", pubkey.toBase58(), position);

        // Update bank
        this.bank[pubkey.toBase58()] = position;

        // Trigger change callback
        positionChangeTriggerFn(pubkey, position);
      }
    );

    const positions = await this.client.loadAllPositions();

    // Push positions to bank
    this.bank = positions.reduce((bank, { publicKey, account: position }) => {
      bank[publicKey.toBase58()] = position;
      return bank;
    }, {});
  }

  public async stop() {
    this.client.unListenToPositionChange();
    this.bank = {};
  }
}

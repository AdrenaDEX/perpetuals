import { IdlAccounts } from "@coral-xyz/anchor";
import { Perpetuals } from "../target/types/perpetuals";

type Accounts = IdlAccounts<Perpetuals>;

export type Position = Accounts["position"];
export type Custody = Accounts["custody"];

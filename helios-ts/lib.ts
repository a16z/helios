import initWasm, { EthereumClient, OpStackClient } from "./pkg/index";

export async function init() {
  const wasmData = require("./pkg/index_bg.wasm");
  await initWasm(wasmData);
}

/// An EIP-1193 compliant Ethereum provider. Treat this the same as you
/// would window.ethereum when constructing an ethers or web3 provider.
export class HeliosProvider {
  #client;
  #chainId;

  /// Do not use this constructor. Instead use the createHeliosProvider function.
  constructor(config: Config, kind: "ethereum" | "opstack") {
    if (kind === "ethereum") {
      const executionRpc = config.executionRpc;
      const consensusRpc = config.consensusRpc;
      const checkpoint = config.checkpoint;
      const network = config.network ?? Network.MAINNET;
      const dbType = config.dbType ?? "localstorage";

      this.#client = new EthereumClient(
        executionRpc,
        consensusRpc,
        network,
        checkpoint,
        dbType
      );
    } else if (kind === "opstack") {
      const executionRpc = config.executionRpc;
      const network = config.network;

      this.#client = new OpStackClient(executionRpc, network);
    } else {
      throw "invalid kind: must be ethereum or opstack";
    }
    this.#chainId = this.#client.chain_id();
  }

  async sync() {
    await this.#client.sync();
  }

  async waitSynced() {
    await this.#client.wait_synced();
  }

  async request(req: Request): Promise<any> {
    try {
      return await this.#req(req);
    } catch (err) {
      throw new Error(err.toString());
    }
  }

  async #req(req: Request): Promise<any> {
    switch (req.method) {
      case "eth_getBalance": {
        return this.#client.get_balance(req.params[0], req.params[1]);
      }
      case "eth_chainId": {
        return this.#chainId;
      }
      case "eth_blockNumber": {
        return this.#client.get_block_number();
      }
      case "eth_getTransactionByHash": {
        let tx = await this.#client.get_transaction_by_hash(req.params[0]);
        return mapToObj(tx);
      }
      case "eth_getTransactionCount": {
        return this.#client.get_transaction_count(req.params[0], req.params[1]);
      }
      case "eth_getBlockTransactionCountByHash": {
        return this.#client.get_block_transaction_count_by_hash(req.params[0]);
      }
      case "eth_getBlockTransactionCountByNumber": {
        return this.#client.get_block_transaction_count_by_number(
          req.params[0]
        );
      }
      case "eth_getCode": {
        return this.#client.get_code(req.params[0], req.params[1]);
      }
      case "eth_call": {
        return this.#client.call(req.params[0], req.params[1]);
      }
      case "eth_estimateGas": {
        return this.#client.estimate_gas(req.params[0]);
      }
      case "eth_gasPrice": {
        return this.#client.gas_price();
      }
      case "eth_maxPriorityFeePerGas": {
        return this.#client.max_priority_fee_per_gas();
      }
      case "eth_sendRawTransaction": {
        return this.#client.send_raw_transaction(req.params[0]);
      }
      case "eth_getTransactionReceipt": {
        return this.#client.get_transaction_receipt(req.params[0]);
      }
      case "eth_getTransactionByHash": {
        return this.#client.get_transaction_by_hash(req.params[0]);
      }
      case "eth_getTransactionByBlockHashAndIndex": {
        return this.#client.get_transaction_by_block_hash_and_index(
          req.params[0],
          req.params[1]
        );
      }
      case "eth_getBlockReceipts": {
        return this.#client.get_block_receipts(req.params[0]);
      }
      case "eth_getLogs": {
        return this.#client.get_logs(req.params[0]);
      }
      case "net_version": {
        return this.#chainId;
      }
      case "eth_getBlockByNumber": {
        return this.#client.get_block_by_number(req.params[0], req.params[1]);
      }
      case "web3_clientVersion": {
        return this.#client.client_version();
      }
      default: {
        throw `method not implemented: ${req.method}`;
      }
    }
  }
}

export type Config = {
  executionRpc: string;
  consensusRpc?: string;
  checkpoint?: string;
  network?: Network;
  /** Where to cache checkpoints, default to "localstorage" */
  dbType?: "localstorage" | "config";
};

export enum Network {
  MAINNET = "mainnet",
  GOERLI = "goerli",
}

type Request = {
  method: string;
  params: any[];
};

function mapToObj(map: Map<any, any> | undefined): Object | undefined {
  if (!map) return undefined;

  return Array.from(map).reduce((obj: any, [key, value]) => {
    if (value !== undefined) {
      obj[key] = value;
    }

    return obj;
  }, {});
}

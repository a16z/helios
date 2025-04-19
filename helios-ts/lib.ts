import { EventEmitter } from "events";
import { v4 as uuidv4 } from "uuid";
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
  #eventEmitter;

  /// Do not use this constructor. Instead use the createHeliosProvider function.
  constructor(config: Config, kind: "ethereum" | "opstack") {
    const executionRpc = config.executionRpc;
    const executionVerifiableApi = config.executionVerifiableApi;

    if (kind === "ethereum") {
      const consensusRpc = config.consensusRpc;
      const checkpoint = config.checkpoint;
      const network = config.network ?? Network.MAINNET;
      const dbType = config.dbType ?? "localstorage";

      this.#client = new EthereumClient(
        executionRpc,
        executionVerifiableApi,
        consensusRpc,
        network,
        checkpoint,
        dbType
      );
    } else if (kind === "opstack") {
      const network = config.network;

      this.#client = new OpStackClient(executionRpc, executionVerifiableApi, network);
    } else {
      throw new Error("Invalid kind: must be 'ethereum' or 'opstack'");
    }

    this.#chainId = this.#client.chain_id();
    this.#eventEmitter = new EventEmitter();
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
      case "eth_getStorageAt": {
        return this.#client.get_storage_at(req.params[0], req.params[1], req.params[2]);
      }
      case "eth_getProof": {
        return this.#client.get_proof(req.params[0], req.params[1], req.params[2]);
      }
      case "eth_call": {
        return this.#client.call(req.params[0], req.params[1]);
      }
      case "eth_estimateGas": {
        return this.#client.estimate_gas(req.params[0], req.params[1]);
      }
      case "eth_createAccessList": {
        return this.#client.create_access_list(req.params[0], req.params[1]);
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
        const receipt = await this.#client.get_transaction_receipt(req.params[0]);
        return mapToObj(receipt);
      }
      case "eth_getTransactionByBlockHashAndIndex": {
        const tx = await this.#client.get_transaction_by_block_hash_and_index(
          req.params[0],
          req.params[1]
        );
        return mapToObj(tx);
      }
      case "eth_getTransactionByBlockNumberAndIndex": {
        const tx = await this.#client.get_transaction_by_block_number_and_index(
          req.params[0],
          req.params[1]
        );
        return mapToObj(tx);
      }
      case "eth_getBlockReceipts": {
        const receipts = await this.#client.get_block_receipts(req.params[0]);
        return receipts.map(mapToObj);
      }
      case "eth_getLogs": {
        const logs = await this.#client.get_logs(req.params[0]);
        return logs.map(mapToObj);
      }
      case "eth_getFilterChanges": {
        const changes = await this.#client.get_filter_changes(req.params[0]);
        if (changes.length > 0 && typeof changes[0] === "object") {
          return changes.map(mapToObj);
        }
        return changes;
      }
      case "eth_getFilterLogs": {
        const logs = await this.#client.get_filter_logs(req.params[0]);
        return logs.map(mapToObj);
      }
      case "eth_uninstallFilter": {
        return this.#client.uninstall_filter(req.params[0]);
      }
      case "eth_newFilter": {
        return this.#client.new_filter(req.params[0]);
      }
      case "eth_newBlockFilter": {
        return this.#client.new_block_filter();
      }
      case "eth_newPendingTransactionFilter": {
        return this.#client.new_pending_transaction_filter();
      }
      case "net_version": {
        return this.#chainId;
      }
      case "eth_getBlockByNumber": {
        const block = await this.#client.get_block_by_number(req.params[0], req.params[1]);
        return mapToObj(block);
      }
      case "eth_getBlockByHash": {
        const block = await this.#client.get_block_by_hash(req.params[0], req.params[1]);
        return mapToObj(block);
      }
      case "web3_clientVersion": {
        return this.#client.client_version();
      }
      case "eth_subscribe": {
        return this.#handleSubscribe(req);
      }
      case "eth_unsubscribe": {
        return this.#client.unsubscribe(req.params[0]);
      }
      default: {
        throw `method not implemented: ${req.method}`;
      }
    }
  }

  async #handleSubscribe(req: Request) {
    try {
      let id = uuidv4();
      await this.#client.subscribe(req.params[0], id, (data: any, id: string) => {
        let result = data instanceof Map ? mapToObj(data) : data;
        let payload = {
          type: 'eth_subscription',
          data: {
            subscription: id,
            result,
          },
        };
        this.#eventEmitter.emit("message", payload);
      });
      return id;
    } catch (err) {
      throw new Error(err.toString());
    }
  }

  on(
    eventName: string,
    handler: (data: any) => void
  ): void {
    this.#eventEmitter.on(eventName, handler);
  }

  removeListener(
    eventName: string,
    handler: (data: any) => void
  ): void {
    this.#eventEmitter.off(eventName, handler);
  }
}

export type Config = {
  executionRpc?: string;
  executionVerifiableApi?: string;
  consensusRpc?: string;
  checkpoint?: string;
  network?: Network;
  /** Where to cache checkpoints, default to "localstorage" */
  dbType?: "localstorage" | "config";
};

export enum Network {
  MAINNET = "mainnet",
  SEPOLIA = "sepolia",
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

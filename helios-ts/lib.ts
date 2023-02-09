import { Client } from "./pkg";

/// An EIP-1193 compliant Ethereum provider. Treat this the same as you
/// would window.ethereum when constructing an ethers or web3 provider.
export class HeliosProvider {
  #client;
  #chainId;

  constructor(config: Config) {
    const executionRpc = config.executionRpc;
    const consensusRpc = config.consensusRpc;
    const checkpoint = config.checkpoint;
    const network = config.network ?? Network.MAINNET;

    this.#client = new Client(executionRpc, consensusRpc, network, checkpoint);
    this.#chainId = this.#client.chain_id();
  }

  async sync() {
    await this.#client.sync();
  }
  
  async request(req: Request): Promise<any> {
    switch(req.method) {
      case "eth_getBalance": {
        return this.#client.get_balance(req.params[0], req.params[1]);
      };
      case "eth_chainId": {
        return this.#chainId;
      };
      case "eth_blockNumber": {
        return this.#client.get_block_number();
      };
      case "eth_getTransactionByHash": {
        let tx = await this.#client.get_transaction_by_hash(req.params[0]);
        return mapToObj(tx);
      };
      case "eth_getTransactionCount": {
        return this.#client.get_transaction_count(req.params[0], req.params[1]);
      };
      case "eth_getBlockTransactionCountByHash": {
        return this.#client.get_block_transaction_count_by_hash(req.params[0]);
      };
      case "eth_getBlockTransactionCountByNumber": {
        return this.#client.get_block_transaction_count_by_number(req.params[0]);
      };
      case "eth_getCode": {
        return this.#client.get_code(req.params[0], req.params[1]);
      };
      case "eth_call": {
        return this.#client.call(req.params[0], req.params[1]);
      };
      case "eth_estimateGas": {
        return this.#client.estimate_gas(req.params[0]);
      };
      case "eth_gasPrice": {
        return this.#client.gas_price();
      };
      case "eth_maxPriorityFeePerGas": {
        return this.#client.max_priority_fee_per_gas();
      };
      case "eth_sendRawTransaction": {
        return this.#client.send_raw_transaction(req.params[0]);
      };
      case "eth_getTransactionReceipt": {
        return this.#client.get_transaction_receipt(req.params[0]);
      };
      case "eth_getLogs": {
        return this.#client.get_logs(req.params[0]);
      };
      case "net_version": {
        return this.#chainId;
      };
    }
  }
}

export type Config = {
  executionRpc: string,
  consensusRpc?: string,
  checkpoint?: string,
  network?: Network,
}

export enum Network {
  MAINNET = "mainnet",
  GOERLI = "goerli",
}

type Request = {
  method: string,
  params: any[],
}

function mapToObj(map: Map<any, any> | undefined): Object | undefined {
  if(!map) return undefined;

  return Array.from(map).reduce((obj: any, [key, value]) => {
    obj[key] = value;
    return obj;
  }, {});
}


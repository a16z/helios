import { providers } from "ethers";
const { Web3Provider } = providers;

import { Client } from "./pkg";


export class EthersProvider extends Web3Provider {
  constructor(executionRpc: string, consensusRpc: string) {
    let client = new HeliosExternalProvider(executionRpc, consensusRpc);
    super(client);
  }

  async sync() {
    await (this.provider as HeliosExternalProvider).sync();
  }
}

class HeliosExternalProvider {
  #client;
  #chainId;

  constructor(executionRpc: string, consensusRpc: string) {
    this.#client = new Client(executionRpc, consensusRpc);
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
    }
  }
}

interface Request {
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


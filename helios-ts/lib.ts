import { BigNumber, Transaction, providers } from "ethers";
const { Web3Provider } = providers;

import { Client, RpcRequest } from "./pkg";

export type BlockTag = "latest" | number;

export class HeliosProvider extends Web3Provider {
  constructor(executionRpc: string, consensusRpc: string) {
    let client = new HeliosExternal(executionRpc, consensusRpc);
    super(client);
  }

  async sync() {
    await (this.provider as HeliosExternal).sync();
  }
}

class HeliosExternal {
  #client;

  constructor(executionRpc: string, consensusRpc: string) {
    this.#client = new Client(executionRpc, consensusRpc);
  }

  async sync() {
    await this.#client.sync();
    setInterval(async () => await this.#client.advance(), 12_000);
  }

  async request(req: Request): Promise<any> {
    console.log(req);

    
    // this.#client.request(req)

    // switch(req.method) {
    //   case "eth_getBalance": {
    //     return "0x123";
    //   };
    //   case "eth_chainId": {
    //     return "0x1";
    //   };
    //   case "eth_getBalance": {
    //     return "0x500";
    //   };
    //   case "eth_blockNumber": {
    //     return this.#client.get_block_number();
    //   }
    // }
  }
}

interface Request {
  method: string,
  params?: any[],
}

// export class Client {
//   #node
// 
//   constructor(execution_rpc: string, consensus_rpc: string) {
//     this.#node = new Node(execution_rpc, consensus_rpc);
//   }
// 
//   async sync() {
//     await this.#node.sync();
//     setInterval(async () => await this.#node.advance(), 12_000);
//   }
// 
//   async getBlockNumber(): Promise<number> {
//     return await this.#node.get_block_number();
//   }
// 
//   async getBalance(addr: string, block: BlockTag = "latest"): Promise<BigNumber> {
//     let balance = await this.#node.get_balance(addr, block.toString());
//     return BigNumber.from(balance);
//   }
// 
//   async getCode(addr: string, block: BlockTag = "latest"): Promise<string> {
//     return await this.#node.get_code(addr, block.toString());
//   }
// 
//   async getNonce(addr: string, block: BlockTag = "latest"): Promise<number> {
//     return await this.#node.get_nonce(addr, block.toString());
//   }
// 
//   async getTransaction(hash: string): Promise<Transaction>  {
//     let tx = await this.#node.get_transaction_by_hash(hash);
//     return {
//       hash: tx.hash,
//       to: tx.to,
//       from: tx.from,
//       nonce: tx.nonce,
//       gasLimit: BigNumber.from(tx.gas_limit),
//       data: tx.data,
//       value: BigNumber.from(tx.value),
//       chainId: tx.chain_id,
//       gasPrice: tx.gas_price ? BigNumber.from(tx.gas_price) : null, 
//       maxFeePerGas: tx.max_fee_per_gas ? BigNumber.from(tx.max_fee_per_gas) : null,
//       maxPriorityFeePerGas: tx.max_priority_fee_per_gas ? BigNumber.from(tx.max_priority_fee_per_gas): null,
//       r: tx.r,
//       s: tx.s,
//       v: parseInt(tx.v),
//     }
//   }
// }


import { BigNumber } from "ethers";
import { Node } from "./pkg";


export class Client {
  #node

  constructor(execution_rpc: string, consensus_rpc: string) {
    this.#node = new Node(execution_rpc, consensus_rpc);
  }

  async sync() {
    await this.#node.sync();
    setInterval(async () => await this.#node.advance(), 12_000);
  }

  async getBalance(addr: string): Promise<BigNumber> {
    let balance = await this.#node.get_balance(addr);
    return BigNumber.from(balance);
  }

  async getBlockNumber(): Promise<number> {
    return await this.#node.get_block_number();
  }

  async getCode(addr: string): Promise<string> {
    return await this.#node.get_code(addr);
  }
}


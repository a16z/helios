import { BigNumber } from "ethers";
import { Node } from "./pkg";

export type BlockTag = "latest" | number;

export class Client {
  #node

  constructor(execution_rpc: string, consensus_rpc: string) {
    this.#node = new Node(execution_rpc, consensus_rpc);
  }

  async sync() {
    await this.#node.sync();
    setInterval(async () => await this.#node.advance(), 12_000);
  }

  async getBlockNumber(): Promise<number> {
    return await this.#node.get_block_number();
  }

  async getBalance(addr: string, block: BlockTag = "latest"): Promise<BigNumber> {
    let balance = await this.#node.get_balance(addr, block.toString());
    return BigNumber.from(balance);
  }

  async getCode(addr: string, block: BlockTag = "latest"): Promise<string> {
    return await this.#node.get_code(addr, block.toString());
  }

  async getNonce(addr: string, block: BlockTag = "latest"): Promise<number> {
    return await this.#node.get_nonce(addr, block.toString());
  }
}


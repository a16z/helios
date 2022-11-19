import { Node } from "./pkg";


export class Client {
  #node

  constructor(execution_rpc: string, consensus_rpc: string) {
    this.#node = new Node(execution_rpc, consensus_rpc);
  }

  async sync() {
    await this.#node.sync();
    setInterval(async () => {
      await this.#node.advance();
      let num = await this.#node.block_number();
      console.log(`block number: ${num}`);
    }, 12_000)
  }

  async getBalance(addr: string): Promise<string> {
    return await this.#node.get_balance(addr);
  }

  async blockNumber(): Promise<number> {
    return await this.#node.block_number();
  }
}


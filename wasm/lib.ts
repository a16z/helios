import { Node } from "./pkg";


export class Client {
  #node

  constructor(execution_rpc: string, consensus_rpc: string) {
    this.#node = new Node(execution_rpc, consensus_rpc);
  }

  async sync() {
    await this.#node.sync();
  }

  async getBalance(addr: string): Promise<string> {
    return await this.#node.get_balance(addr);
  }
}


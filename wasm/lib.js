import { Node } from "./pkg";


export class Client {
  #node

  constructor(execution_rpc, consensus_rpc) {
    this.#node = new Node(execution_rpc, consensus_rpc);
  }

  async sync() {
    await this.#node.sync();
  }

  async getBalance(addr) {
    return await this.#node.get_balance(addr);
  }
}


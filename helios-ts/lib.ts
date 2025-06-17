import { EventEmitter } from "eventemitter3";
import { v4 as uuidv4 } from "uuid";
import initWasm, { EthereumClient, OpStackClient, LineaClient } from "./pkg";

let initPromise: Promise<any> | null = null;

async function ensureInitialized() {
  if (!initPromise) {
    initPromise = initWasm();
  }
  await initPromise;
}

/**
 * Supported network kinds for the Helios light client.
 * 
 * @remarks
 * - `ethereum` - Standard Ethereum networks (mainnet, testnets)
 * - `opstack` - Optimism Stack based L2 networks
 * - `linea` - Linea L2 network
 */
export type NetworkKind = "ethereum" | "opstack" | "linea";

/**
 * Creates a new HeliosProvider instance.
 * 
 * @param config - Configuration object for the provider
 * @param kind - The type of network to connect to
 * @returns A promise that resolves to an initialized HeliosProvider instance
 * 
 * @remarks
 * This function creates an EIP-1193 compliant Ethereum provider that can be used
 * with popular web3 libraries like ethers.js or web3.js. Treat this the same as
 * you would `window.ethereum` when constructing a provider.
 * 
 * @example
 * ```typescript
 * const provider = await createHeliosProvider({
 *   executionRpc: "https://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY",
 *   consensusRpc: "https://www.lightclientdata.org",
 *   network: Network.MAINNET
 * }, "ethereum");
 * ```
 */
export async function createHeliosProvider(config: Config, kind: NetworkKind): Promise<HeliosProvider> {
  await ensureInitialized();
  return HeliosProvider.createInternal(config, kind);
}

/**
 * An EIP-1193 compliant Ethereum provider powered by the Helios light client.
 * 
 * @remarks
 * HeliosProvider implements the Ethereum Provider API (EIP-1193) and can be used
 * as a drop-in replacement for `window.ethereum` in web3 applications. It provides
 * trustless access to Ethereum without relying on centralized RPC providers.
 * 
 * The provider supports all standard Ethereum JSON-RPC methods and maintains
 * compatibility with popular libraries like ethers.js, web3.js, and viem.
 * 
 * @example
 * ```typescript
 * // Using with ethers.js
 * import { BrowserProvider } from 'ethers';
 * 
 * const heliosProvider = await createHeliosProvider(config, "ethereum");
 * const ethersProvider = new BrowserProvider(heliosProvider);
 * 
 * // Using with web3.js
 * import Web3 from 'web3';
 * 
 * const heliosProvider = await createHeliosProvider(config, "ethereum");
 * const web3 = new Web3(heliosProvider);
 * ```
 */
export class HeliosProvider {
  #client;
  #chainId;
  #eventEmitter;

  private constructor(config: Config, kind: NetworkKind) {
    const executionRpc = config.executionRpc;
    const verifiableApi = config.verifiableApi;

    if (kind === "ethereum") {
      const consensusRpc = config.consensusRpc;
      const checkpoint = config.checkpoint;
      const network = config.network ?? Network.MAINNET;
      const dbType = config.dbType ?? "localstorage";

      this.#client = new EthereumClient(
        executionRpc,
        verifiableApi,
        consensusRpc,
        network,
        checkpoint,
        dbType
      );
    } else if (kind === "opstack") {
      const network = config.network;
      this.#client = new OpStackClient(executionRpc, verifiableApi, network);
    } else if (kind === "linea") {
      const network = config.network;
      this.#client = new LineaClient(executionRpc, network);
    } else {
      throw new Error("Invalid kind: must be 'ethereum', 'opstack', or `linea`");
    }

    this.#chainId = this.#client.chain_id();
    this.#eventEmitter = new EventEmitter();
  }

  /** @internal */
  static createInternal(config: Config, kind: NetworkKind): HeliosProvider {
    return new HeliosProvider(config, kind);
  }


  /**
   * Waits for the light client to sync with the network.
   * 
   * @returns A promise that resolves when the client is fully synced
   * 
   * @remarks
   * This method blocks until the light client has synchronized with the network
   * and is ready to process requests. It's recommended to call this before
   * making any RPC requests to ensure accurate data.
   * 
   * @example
   * ```typescript
   * const provider = await createHeliosProvider(config, "ethereum");
   * await provider.waitSynced();
   * console.log("Provider is ready!");
   * ```
   */
  async waitSynced() {
    await this.#client.wait_synced();
  }

  /**
   * Sends an RPC request to the provider.
   * 
   * @param req - The RPC request object containing method and params
   * @returns A promise that resolves with the RPC response
   * @throws {Error} If the RPC method is not supported or the request fails
   * 
   * @remarks
   * This is the main entry point for all Ethereum JSON-RPC requests. It implements
   * the EIP-1193 provider interface and supports all standard Ethereum RPC methods.
   * 
   * @example
   * ```typescript
   * // Get the latest block number
   * const blockNumber = await provider.request({
   *   method: "eth_blockNumber",
   *   params: []
   * });
   * 
   * // Get account balance
   * const balance = await provider.request({
   *   method: "eth_getBalance",
   *   params: [address, "latest"]
   * });
   * ```
   */
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
        let res = this.#client.call(req.params[0], req.params[1]);
        console.log(res);
        return res;
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
        throw `method not supported: ${req.method}`;
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

  /**
   * Registers an event listener for provider events.
   * 
   * @param eventName - The name of the event to listen for
   * @param handler - The callback function to handle the event
   * 
   * @remarks
   * Supports standard EIP-1193 provider events including:
   * - `message` - For subscription updates
   * - `connect` - When the provider connects
   * - `disconnect` - When the provider disconnects
   * - `chainChanged` - When the chain ID changes
   * - `accountsChanged` - When accounts change (if applicable)
   * 
   * @example
   * ```typescript
   * provider.on("message", (message) => {
   *   console.log("Received message:", message);
   * });
   * 
   * // Subscribe to new blocks
   * const subId = await provider.request({
   *   method: "eth_subscribe",
   *   params: ["newHeads"]
   * });
   * ```
   */
  on(
    eventName: string,
    handler: (data: any) => void
  ): void {
    this.#eventEmitter.on(eventName, handler);
  }

  /**
   * Removes an event listener from the provider.
   * 
   * @param eventName - The name of the event to stop listening for
   * @param handler - The callback function to remove
   * 
   * @remarks
   * Removes a previously registered event listener. The handler must be
   * the same function reference that was passed to `on()`.
   * 
   * @example
   * ```typescript
   * const handler = (data) => console.log(data);
   * 
   * // Add listener
   * provider.on("message", handler);
   * 
   * // Remove listener
   * provider.removeListener("message", handler);
   * ```
   */
  removeListener(
    eventName: string,
    handler: (data: any) => void
  ): void {
    this.#eventEmitter.off(eventName, handler);
  }
}

/**
 * Configuration options for creating a Helios provider.
 * 
 * @remarks
 * Different network kinds require different configuration options:
 * - For Ethereum networks: executionRpc, consensusRpc, and optionally checkpoint are required
 * - For OpStack networks: executionRpc and verifiableApi are required
 * - For Linea networks: executionRpc is required
 */
export type Config = {
  /**
   * The RPC endpoint for execution layer requests.
   * This is required for all network types.
   * @example "https://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY"
   */
  executionRpc?: string;
  
  /**
   * The verifiable API endpoint for any networks.
   * Not recommended for use currently.
   * @example "https://verifiable-api-ethereum.operationsolarstorm.org
   */
  verifiableApi?: string;
  
  /**
   * The consensus layer RPC endpoint for Ethereum and OP Stack networks.
   * Required for Ethereum and OP Stack networks to sync.
   * @example "https://www.lightclientdata.org"
   */
  consensusRpc?: string;
  
  /**
   * A trusted checkpoint for faster initial sync on Ethereum networks.
   * Optional but recommended for better performance.
   * @example "0x1234567890abcdef..."
   */
  checkpoint?: string;
  
  /**
   * The network to connect to.
   * Defaults to Network.MAINNET for Ethereum networks.
   */
  network?: Network;
  
  /**
   * Where to cache checkpoints for persistence.
   * @defaultValue "localstorage"
   * @remarks
   * - `localstorage` - Store in browser's localStorage (web environments)
   * - `config` - Store in configuration (node environments)
   */
  dbType?: "localstorage" | "config";
};

/**
 * Supported networks across all network kinds for the Helios provider.
 * 
 * @remarks
 * Networks are organized by their network kind:
 * - Ethereum networks: "mainnet", "sepolia", "holesky", "hoodi"
 * - OP Stack networks: "op-mainnet", "base", "worldchain", "zora", "unichain"
 * - Linea networks: "linea", "linea-sepolia"
 * 
 * @example
 * ```typescript
 * // For Ethereum mainnet
 * const config: Config = {
 *   executionRpc: "https://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY",
 *   consensusRpc: "https://www.lightclientdata.org",
 *   network: "mainnet"
 * };
 * 
 * // For Optimism
 * const config: Config = {
 *   executionRpc: "https://mainnet.optimism.io",
 *   consensusRpc: "https://op-mainnet.operationsolarstorm.org",
 *   network: "op-mainnet"
 * };
 * ```
 */
export type Network = 
  // Ethereum networks
  | "mainnet"      // Ethereum mainnet (chain ID: 1)
  | "goerli"       // Goerli testnet (deprecated)
  | "sepolia"      // Sepolia testnet (chain ID: 11155111)
  | "holesky"      // Holesky testnet (chain ID: 17000)
  | "hoodi"        // Hoodi testnet (chain ID: 560048)
  // OP Stack networks
  | "op-mainnet"   // OP Mainnet (chain ID: 10)
  | "base"         // Base mainnet (chain ID: 8453)
  | "worldchain"   // Worldchain mainnet (chain ID: 480)
  | "zora"         // Zora mainnet (chain ID: 7777777)
  | "unichain"     // Unichain mainnet (chain ID: 130)
  // Linea networks
  | "linea"        // Linea mainnet (chain ID: 59144)
  | "linea-sepolia"; // Linea Sepolia testnet (chain ID: 59141)

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

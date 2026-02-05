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
 * with popular web3 libraries like viem, ethers.js or web3.js. Treat this the same as
 * you would `window.ethereum` when constructing a provider.
 * 
 * @example
 * ```typescript
 * const provider = await createHeliosProvider({
 *   executionRpc: "https://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY",
 *   consensusRpc: "https://www.lightclientdata.org",
 *   network: "mainnet"
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
 * compatibility with popular libraries like viem, ethers.js, and web3.js.
 * 
 * @example
 * ```typescript
 * // Using with viem
 * import { createPublicClient, custom } from 'viem';
 * import { mainnet } from 'viem/chains';
 * 
 * const heliosProvider = await createHeliosProvider(config, "ethereum");
 * const client = createPublicClient({
 *   chain: mainnet,
 *   transport: custom(heliosProvider)
 * });
 * 
 * // Using with ethers.js
 * import { BrowserProvider } from 'ethers';
 * 
 * const heliosProvider = await createHeliosProvider(config, "ethereum");
 * const ethersProvider = new BrowserProvider(heliosProvider);
 * ```
 */
export class HeliosProvider {
  #client;
  #chainId;
  #eventEmitter;
  #closed = false;
  #subscriptionIds: Set<string> = new Set();

  private constructor(config: Config, kind: NetworkKind) {
    const executionRpc = config.executionRpc;
    const verifiableApi = config.verifiableApi;

    if (kind === "ethereum") {
      const consensusRpc = config.consensusRpc;
      const checkpoint = config.checkpoint;
      const network = config.network ?? "mainnet";
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
    this.#setHeliosEvents();
  }

  #setHeliosEvents() {
    // Capture only the emitter reference, not `this`
    const emitter = this.#eventEmitter;
    this.#client.set_helios_events((event: string, data: any) => {
      emitter.emit(event, data);
    });
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
    if (this.#closed) {
      throw new Error("Provider has been shut down");
    }
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
        return this.#client.call(req.params[0], req.params[1], req.params[2]);
      }
      case "eth_estimateGas": {
        return this.#client.estimate_gas(req.params[0], req.params[1], req.params[2]);
      }
      case "eth_createAccessList": {
        return this.#client.create_access_list(req.params[0], req.params[1], req.params[2]);
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
        const id = req.params[0];
        this.#subscriptionIds.delete(id);
        return this.#client.unsubscribe(id);
      }
      case "helios_getCurrentCheckpoint": {
        return this.#client.get_current_checkpoint();
      }
      default: {
        throw new Error(`method not supported: ${req.method}`);
      }
    }
  }

  async #handleSubscribe(req: Request) {
    try {
      const id = uuidv4();
      // Capture only the emitter reference, not `this`
      const emitter = this.#eventEmitter;
      await this.#client.subscribe(req.params[0], id, (data: any, subId: string) => {
        const result = data instanceof Map ? mapToObj(data) : data;
        const payload = {
          type: 'eth_subscription',
          data: {
            subscription: subId,
            result,
          },
        };
        emitter.emit("message", payload);
      });
      this.#subscriptionIds.add(id);
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
  ): this {
    this.#eventEmitter.on(eventName, handler);
    return this;
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
  ): this {
    this.#eventEmitter.off(eventName, handler);
    return this;
  }

  /**
   * Shuts down the provider and releases all resources.
   * 
   * @returns A promise that resolves when the provider has been shut down
   * 
   * @remarks
   * After shutdown:
   * - All future `request()` calls will reject with an error
   * - All active subscriptions are unsubscribed
   * - All event listeners are removed
   * - Background tasks are stopped
   * 
   * The provider instance will be garbage collected after the user drops all references.
   * 
   * @example
   * ```typescript
   * const provider = await createHeliosProvider(config, "ethereum");
   * 
   * // ... use the provider ...
   * 
   * // Clean up when done
   * await provider.shutdown();
   * ```
   */
  async shutdown(): Promise<void> {
    if (this.#closed) {
      return;
    }
    this.#closed = true;

    for (const id of this.#subscriptionIds) {
      try {
        this.#client.unsubscribe(id);
      } catch {
        // Ignore errors during cleanup
      }
    }
    this.#subscriptionIds.clear();
    await this.#client.shutdown();
    this.#eventEmitter.removeAllListeners();
    this.#client.free();
  }
  
  /**
   * This method is equivalent to `shutdown()`
   */
  async destroy(): Promise<void> {
    await this.shutdown();
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
  | "base-sepolia" // Base Sepolia testnet (chain ID: 85432)
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

/**
 * Converts a Map to an object, including nested Maps and arrays of Maps.
 * IMPORTANT: This function will mutate input!
 * 
 * @param map - The Map to convert
 * @returns The converted object
 */
function mapToObj(map: Map<any, any> | undefined): Record<string, any> | undefined {
  if (!map) return undefined;

  const result: Record<string, any> = {};
  
  for (const [key, value] of map) {
    if (value === undefined) continue;

    if (value instanceof Map) {
      result[key] = mapToObj(value);
    } else if (Array.isArray(value)) {
      for (let i = 0; i < value.length; i++) {
        if (value[i] instanceof Map) {
          // Mutate in-place
          value[i] = mapToObj(value[i]);
        }
      }
      result[key] = value;
    } else {
      result[key] = value;
    }
  }
  return result;
}

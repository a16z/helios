import { createPublicClient, custom, type PublicClient, type Block, type Chain } from 'viem';
import { mainnet, optimism, base, linea } from 'viem/chains';

// Import helios from the parent directory's built output
// @ts-ignore - importing local build
import * as helios from '../../dist/lib.mjs';

interface NetworkConfig {
  name: string;
  blockTime: number;
  cfg: any;
  kind: 'ethereum' | 'opstack' | 'linea';
  provider?: any;
  viemClient?: PublicClient;
  lastSeen?: bigint;
  chain: Chain;
}

// Build display string for a block
function formatBlock(block: Block): string {
  const ts = new Date(Number(block.timestamp) * 1000);
  const date = ts.toLocaleDateString();
  const time = ts.toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
  const txCount = block.transactions.length;
  const txLabel = txCount === 1 ? "transaction" : "transactions";
  return `<span class="block-number">Block ${block.number}</span><br>${date} ${time}<br>${txCount} ${txLabel}`;
}

// Render a block and start animations
function renderBlock(network: string, blockTime: number, block: Block): void {
  const el = document.getElementById(`${network}-latest`);
  if (!el) return;
  
  el.classList.remove("loading");
  el.innerHTML = formatBlock(block);
  el.style.setProperty("--duration", `${blockTime}s`);
  el.classList.remove("animate", "flash");
  void el.offsetWidth; // Force reflow
  el.classList.add("animate", "flash");
}

// Main function
async function main() {
  // Get API key from environment variable
  const key = import.meta.env.VITE_ALCHEMY_KEY;
  
  if (!key) {
    console.error('Please set VITE_ALCHEMY_KEY in your .env file');
    const containers = document.querySelectorAll('.block-info');
    containers.forEach(el => {
      el.classList.remove('loading');
      el.textContent = 'Missing Alchemy API Key\nPlease set VITE_ALCHEMY_KEY in .env file';
    });
    return;
  }

  const networks: NetworkConfig[] = [
    {
      name: "ethereum",
      blockTime: 12,
      cfg: {
        executionRpc: `https://eth-mainnet.g.alchemy.com/v2/${key}`,
        checkpoint:
          "0x872dbe854d76a78fd22c8c7f2e157467a1604f585785e5e6fbbc2987c1b1980e",
        dbType: "localstorage",
      },
      kind: "ethereum",
      chain: mainnet,
    },
    {
      name: "op-mainnet",
      blockTime: 2,
      cfg: {
        executionRpc: `https://opt-mainnet.g.alchemy.com/v2/${key}`,
        network: "op-mainnet",
      },
      kind: "opstack",
      chain: optimism,
    },
    {
      name: "base",
      blockTime: 2,
      cfg: {
        executionRpc: `https://base-mainnet.g.alchemy.com/v2/${key}`,
        network: "base",
      },
      kind: "opstack",
      chain: base,
    },
    {
      name: "linea",
      blockTime: 3,
      cfg: {
        executionRpc: `https://linea-mainnet.g.alchemy.com/v2/${key}`,
        network: "mainnet",
      },
      kind: "linea",
      chain: linea,
    },
  ];

  // Create providers for each network and attach viem client
  await Promise.all(
    networks.map((n) =>
      (async () => {
        n.provider = await helios.createHeliosProvider(n.cfg, n.kind);
        // Create a viem public client using the Helios provider
        n.viemClient = createPublicClient({
          chain: n.chain,
          transport: custom(n.provider),
        });
      })()
    )
  );

  // Poll for latest blocks
  networks.forEach((n) => {
    setInterval(async () => {
      try {
        if (!n.viemClient) return;
        
        const latestNumber = await n.viemClient.getBlockNumber();
        if (latestNumber !== n.lastSeen) {
          n.lastSeen = latestNumber;
          const blk = await n.viemClient.getBlock({
            blockNumber: latestNumber,
            includeTransactions: true,
          });
          renderBlock(n.name, n.blockTime, blk);
        }
      } catch (err) {
        console.error(`Error fetching ${n.name}:`, err);
      }
    }, 1000);
  });
}

// Start the application when DOM is ready
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', main);
} else {
  main();
}

// Export for global access if needed
declare global {
  interface Window {
    networks: NetworkConfig[];
  }
}

window.networks = [];
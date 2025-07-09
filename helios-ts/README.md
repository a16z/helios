# @a16z/helios

Helios is a trustless, efficient, and portable Ethereum light client written in Rust with TypeScript bindings for web and Node.js environments. It comes both as a UMD and ES module.

## Installation & Basic Setup Examples

### 1. Installing using NPM

Install the package:
```bash
npm install @a16z/helios
```

Basic usage in your project (e.g., `index.js` or `index.mjs` or `main.ts`):
```typescript
import { createHeliosProvider } from '@a16z/helios';

async function main() {
  // Create provider - WASM initialization is handled automatically
  const heliosProvider = await createHeliosProvider({
    executionRpc: 'https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY',
    consensusRpc: 'https://ethereum.operationsolarstorm.org',
    network: 'mainnet',
    checkpoint: "0x..."
  }, 'ethereum');

  await heliosProvider.waitSynced();
  console.log('Helios is synced and ready!');
  
  // Example: Get latest block number
  const blockNumber = await heliosProvider.request({ method: 'eth_blockNumber', params: [] });
  console.log('Latest block number:', parseInt(blockNumber, 16));
}

main().catch(console.error);
```

### 2. Loading a webpage from a CDN

For a quick test, you can try Helios directly in an HTML file using a CDN like unpkg.

**As a UMD module:**
```html
<!DOCTYPE html>
<html>
<head>
  <title>Helios UMD Test</title>
  <script src="https://unpkg.com/@a16z/helios/dist/lib.umd.js"></script>
</head>
<body>
  <script>
    async function main() {
      // Helios is available via global variable `helios`
      const provider = await helios.createHeliosProvider({
        executionRpc: 'https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY',
        consensusRpc: 'https://ethereum.operationsolarstorm.org',
        network: 'mainnet',
        checkpoint: "0x..."
      }, 'ethereum');
      
      await provider.waitSynced();
      console.log('Helios is ready!');
    }
    
    main().catch(console.error);
  </script>
</body>
</html>
```

**As an ES module:**
```html
<!DOCTYPE html>
<html>
<head>
  <title>Helios ESM Test</title>
</head>
<body>
  <script type="module">
    import { createHeliosProvider } from 'https://unpkg.com/@a16z/helios/dist/lib.mjs';
    
    async function main() {
      const provider = await createHeliosProvider({
        executionRpc: 'https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY',
        consensusRpc: 'https://ethereum.operationsolarstorm.org',
        network: 'mainnet',
        checkpoint: "0x..."
      }, 'ethereum');
      
      await provider.waitSynced();
      console.log('Helios is ready!');
    }
    
    main().catch(console.error);
  </script>
</body>
</html>
```

## Usage as EIP-1193 provider

Helios can be used as an EIP-1193 provider. Once initialized and synced (as shown in the examples above), `heliosProvider` can be used with libraries like viem, ethers.js, web3.js, etc.

Example with viem:
```typescript
import { createPublicClient, custom } from 'viem';
import { mainnet } from 'viem/chains';
import { createHeliosProvider } from '@a16z/helios';

// Create and sync Helios provider
const heliosProvider = await createHeliosProvider({
  executionRpc: 'https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY',
  consensusRpc: 'https://lodestar-mainnet.chainsafe.io',
  network: 'mainnet',
  checkpoint: "0x..."
}, 'ethereum');

await heliosProvider.waitSynced();

// Use with viem
const client = createPublicClient({
  chain: mainnet,
  transport: custom(heliosProvider)
});

const blockNumber = await client.getBlockNumber();
console.log('Latest block number:', blockNumber);
```

## Documentation

See [helios github repo](https://github.com/a16z/helios/) for more details.

## License

This project is licensed under the MIT License - see the main repository for details.

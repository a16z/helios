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
import { init, HeliosProvider } from '@a16z/helios';

async function main() {
  await init(); // Initialize Helios WASM
  const heliosProvider = new HeliosProvider({
    network: 'mainnet',
    executionRpc: 'https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY',
    checkpoint: "0x..."
  });

  await heliosProvider.waitSynced();
  console.log('Helios is synced and ready!');
  
  // Example: Get latest block number
  const blockNumber = await heliosProvider.request({ method: 'eth_blockNumber' });
  console.log('Latest block number:', parseInt(blockNumber, 16));
}
```

### 2. Loading on a webpage from a CDN

For a quick test, you can try Helios directly in an HTML file using a CDN like unpkg.

**As a UMD module:**
```html
<!DOCTYPE html>
<html>
<head>
  <title>Helios UMD Test</title>
  <script src=""></script>
</head>
<body>
  <script>
    // Helios will be available via a global variable `helios`, call helios.init() to initialize it
    // and use helios.HeliosProvider constructor to create a provider
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
    import { init, HeliosProvider } from '';
    // your code here
  </script>
</body>
</html>
```

## Usage as EIP-1193 provider

Helios can be used as an EIP-1193 provider. Once initialized and synced (as shown in the examples above), `heliosProvider` can be used with libraries like ethers.js, web3.js, etc.

Example with ethers.js:
```typescript
import { ethers } from 'ethers';
// Assuming heliosProvider is initialized and synced as shown in the Node.js/Bundler example
const ethersProvider = new ethers.providers.Web3Provider(heliosProvider);
const blockNumber = await ethersProvider.getBlockNumber();
console.log('Latest block number:', blockNumber);
```

## Documentation

See [helios github repo](https://github.com/a16z/helios/) for more details.

## License

This project is licensed under the MIT License - see the main repository for details.

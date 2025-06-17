# Helios TypeScript Example

This is a TypeScript implementation of the Helios multichain light client demo that displays the latest blocks from multiple chains.

## Prerequisites

Before running this example, you need to build the parent helios-ts library:

```bash
cd ..
npm install
npm run build
```

## Setup

1. Create a `.env` file by copying the example:
```bash
cp .env.example .env
```

2. Edit `.env` and add your Alchemy API key:
```
VITE_ALCHEMY_KEY=your_actual_alchemy_key_here
```

You can get an Alchemy API key at https://www.alchemy.com/

3. Install dependencies:
```bash
npm install
```

## Development

Run the development server:
```bash
npm run dev
```

The application will be available at http://localhost:3000

## Build

To build for production:
```bash
npm run build
```

The built files will be in the `dist` directory.

## Features

- TypeScript for type safety
- Viem for modern, lightweight Ethereum interactions
- Vite for fast development and optimized builds
- Real-time block updates from multiple chains:
  - Ethereum
  - OP Mainnet
  - Base
  - Linea

## How it works

The application uses the Helios light client as a custom transport provider for Viem. This creates a secure, verifiable connection to multiple blockchain networks. The app displays the latest block information for each chain, polling for new blocks every second and updating the display with animations when new blocks are found.

## Tech Stack

- **Helios**: Light client for secure blockchain connections
- **Viem**: Modern TypeScript library for Ethereum interactions
- **Vite**: Build tool and dev server
- **TypeScript**: Type-safe development
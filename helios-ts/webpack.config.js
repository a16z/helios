import path from 'node:path'
import { fileURLToPath } from 'node:url'
import WasmPackPlugin from '@wasm-tool/wasm-pack-plugin'
import { merge } from 'webpack-merge'

const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)

/**
 * @type {import('webpack').Configuration}
 */
const commonConfig = {
  entry: './lib.ts',
  output: {
    path: path.resolve(__dirname, 'dist'),
  },
  module: {
    rules: [
      {
        test: /\.ts?$/,
        use: {
          loader: 'ts-loader',
          options: {
            transpileOnly: true, 
          },
        },
        exclude: /node_modules/,
      },
      {
        test: /\.wasm$/,
        type: 'asset/inline',
      },
    ],
  },
  resolve: {
    extensions: ['.ts', '.js'],
  },
  experiments: {
    asyncWebAssembly: true,
  },
  plugins: [
    new WasmPackPlugin({
      extraArgs: '--target web',
      crateDirectory: path.resolve(__dirname),
    }),
  ],
}

/**
 * @type {import('webpack').Configuration}
 */
const commonJSConfig = {
  output: {
    filename: 'lib.cjs',
    library: {
      type: 'commonjs2',
    },
  },
}

/**
 * @type {import('webpack').Configuration}
 */
const esmConfig = {
  output: {
    filename: 'lib.js',
    library: {
      type: 'module',
    },
  },
  experiments: {
    outputModule: true,
  },
}

/**
 * @type {import('webpack').Configuration[]}
 */
export default [merge(commonConfig, commonJSConfig), merge(commonConfig, esmConfig)]

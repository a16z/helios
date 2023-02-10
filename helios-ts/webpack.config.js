const path = require("path");
const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin");

module.exports = {
  entry: "./lib.ts",
  module: {
    rules: [
      {
        test: /\.ts?$/,
        use: 'ts-loader',
        exclude: /node_modules/,
      },
      {
        test: /\.wasm$/,
        type: "asset/inline",
      },
    ],
  },
  resolve: {
    extensions: ['.ts', '.js'],
  },
  output: {
    filename: "lib.js",
    globalObject: 'this', 
    // publicPath: '',
    path: path.resolve(__dirname, "dist"),
    library: {
      name: "helios",
      type: "umd",
    }
  },
  experiments: {
    asyncWebAssembly: true,
  },
  plugins: [
    new WasmPackPlugin({
      extraArgs: "--target web",
      crateDirectory: path.resolve(__dirname),
    }),
  ],
};

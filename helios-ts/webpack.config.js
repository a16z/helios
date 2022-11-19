const path = require("path");

module.exports = {
  entry: "./lib.ts",
  module: {
    rules: [
      {
        test: /\.ts?$/,
        use: 'ts-loader',
        exclude: /node_modules/,
      },
    ],
  },
  resolve: {
    extensions: ['.ts', '.js'],
  },
  output: {
    filename: "bundle.js",
    path: path.resolve(__dirname, "dist"),
    library: {
      name: "helios",
      type: "umd",
    }
  },
  experiments: {
    asyncWebAssembly: true,
  }
};

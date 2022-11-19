const path = require("path");

module.exports = {
  entry: "./lib.js", // input file of the JS bundle
  output: {
    filename: "bundle.js", // output filename
    path: path.resolve(__dirname, "dist"), // directory of where the bundle will be created at
    library: {
      name: "helios",
      type: "umd",
    }
  },
  experiments: {
    asyncWebAssembly: true,
  }
};

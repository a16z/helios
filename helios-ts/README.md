## Dependencies
Building requires [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)

Testing in the browser with `run.sh` requires [local-cors-proxy](https://www.npmjs.com/package/local-cors-proxy) and [simple-http-server](https://crates.io/crates/simple-http-server)

## Build Instructions
To build run:
```
wasm-pack build
```

To build and start run:
```
./run.sh
```

## Running in the browser
After running `run.sh`, head over to `localhost:8000/index.html` and open the javascript console. This website injects a `client` object in the browser for testing. Try running `await client.getBalance("0xDAFEA492D9c6733ae3d56b7Ed1ADB60692c98Bc5")` to test it out.

## Running with npm
TODO

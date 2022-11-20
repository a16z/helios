## Dependencies
Building requires [emcc](https://emscripten.org/) and [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)

Testing in the browser with `run.sh` requires [local-cors-proxy](https://www.npmjs.com/package/local-cors-proxy) and [simple-http-server](https://crates.io/crates/simple-http-server)

## Build Instructions
All commands must be run from the `helios-ts` directory

on first build run (this will fail)
```
wasm-pack build
```

then run (future builds will only need this command)
```
CC=emcc AR=emar wasm-pack build
```

or (you may need to do `chmod +x run.sh` to make it executable)
```
./run.sh
```

## Running in the browser
After running `run.sh`, head over to `localhost:8000/index.html` and open the javascript console. This website injects a `client` object in the browser for testing. Try running `await client.getBalance("0xDAFEA492D9c6733ae3d56b7Ed1ADB60692c98Bc5")` to test it out.

## Running with npm
TODO

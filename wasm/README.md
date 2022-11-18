## Build Instructions
first run (this will fail)
```
wasm-pack build --target web
```

then run
```
CC=emcc AR=emar wasm-pack build --target web
```

Do I know why this fixes it? Nope. Does it fix it? Yep. Oh WebAssembly...


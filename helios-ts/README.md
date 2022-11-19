## Build Instructions
on first build run (this will fail)
```
wasm-pack build
```

then run (future builds will only need this command)
```
CC=emcc AR=emar wasm-pack build
```

or
```
./run.sh
```


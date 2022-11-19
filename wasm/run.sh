lcp --proxyUrl https://www.lightclientdata.org --port 9001 &
lcp --proxyUrl https://eth-mainnet.g.alchemy.com/v2/23IavJytUwkTtBMpzt_TZKwgwAarocdT --port 9002 &

CC=emcc AR=emar wasm-pack build
npm run build
simple-http-server

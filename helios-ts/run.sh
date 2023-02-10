set -e

(&>/dev/null lcp --proxyUrl https://eth-mainnet.g.alchemy.com/v2/23IavJytUwkTtBMpzt_TZKwgwAarocdT --port 9001 &)
(&>/dev/null lcp --proxyUrl https://www.lightclientdata.org --port 9002 &)

npm run build
simple-http-server

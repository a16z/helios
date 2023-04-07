set -e

if [ -z $ETH_RPC_URL ]; then
    echo "ETH_RPC_URL not set"
    exit 1
fi

(&>/dev/null lcp --proxyUrl $ETH_RPC_URL --port 9001 &)
(&>/dev/null lcp --proxyUrl https://www.lightclientdata.org --port 9002 &)

npm run build
simple-http-server

import {createHeliosProvider} from '../../../dist/lib';

import type { Config as NetworkConfig, HeliosProvider, Request, Config, NetworkKind } from '../../../dist/lib'

type IndexedNetworks = {
  [key: string]: {
    cfg: NetworkConfig,
    kind: NetworkKind,
    provider?: HeliosProvider | null
  };
};
let networks: IndexedNetworks = {}

self.onmessage = async (e) => {  
    let res  
    switch (e.data.method) {
        case 'networks':
            res = await handleNetworks(e.data.params);            
            self.postMessage({jsonrpc: '2.0', id: e.data.id, result: res})
            break;
        case 'eth_rpc_req':
            res = await handleEthRpcReq(e.data.params)            
            self.postMessage({jsonrpc: '2.0', id: e.data.id, result: res})
            break;
        default:
            console.error('Unhandled message/<method>:', e.data.method);
    }
};

const handleNetworks = async (options: {method: string, params: {name?: string, cfg: Config, kind: NetworkKind}}): Promise<any> => {
    const { method, params: { name = '', cfg, kind } } = options; // Set default value for name
    // const method = options.method
    // const params = options.params
    
    return new Promise(async (resolve, reject) => {
        try {
            switch(method) {
                // If the name already exists resolve false, otherwise create the new networks Record and resolve true
                // Reject with error if createHeliosProvider() fails
                case 'create':
                    if(name in networks) {
                        resolve(false)
                    } else {
                        try {
                            networks[name] = {
                                kind: kind,
                                cfg: cfg,
                                provider: await createHeliosProvider(cfg, kind)
                            }   
                            resolve(true)
                        } catch(err) {
                            reject(err)
                        }
                    }
                    break;
                // Await for the network provider to sync the chain and then resolve with the entire network object and provider set to true
                // Else resolve false
                case 'read':
                    if(name in networks) {
                        let res = networks[name]
                        await res.provider?.waitSynced()
                        resolve({name: name, kind: res.kind, cfg: res.cfg, provider: true})
                    } else {
                        resolve(false)
                    }
                    break;
                // Update the network object for a given name
                case 'update':
                    if(name in networks) {
                        try {
                            networks[name].cfg = {
                                ...networks[name].cfg,
                                ...cfg,
                            };
                            if(kind) {
                                networks[name].kind = kind
                            }
                            networks[name].provider = await createHeliosProvider(cfg, kind)
                            resolve(true)
                        } catch(err) {
                            reject(err)
                        }
                    } else {
                        resolve(false)
                    }
                    break;
                // Delete a network Record
                case 'delete':
                    if(name in networks) {
                        delete networks[name]
                        resolve(true)
                    } else {
                        resolve(false)
                    }
                    break;
                default:
                    reject('Unhandled message/networks/<method>')
            }
        } catch(err) {

        }
    })
}

const handleEthRpcReq = async (params: {name: string, req: Request}): Promise<any> => {
    await networks[params.name].provider?.waitSynced()    
    
    return new Promise(async (resolve, reject) => {
        try {
            resolve(await networks[params.name]?.provider?.request(params.req))
        } catch(err) {
            reject(err)
        }
    })
}
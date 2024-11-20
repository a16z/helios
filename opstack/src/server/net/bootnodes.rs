use std::str::FromStr;

use discv5::enr::{CombinedKey, Enr};

/// Default bootnodes to use. Currently consists of 2 Base bootnodes & 1 Op Mainnet bootnode.
pub fn bootnodes() -> Vec<Enr<CombinedKey>> {
    let bootnodes = [
        "enr:-J64QBbwPjPLZ6IOOToOLsSjtFUjjzN66qmBZdUexpO32Klrc458Q24kbty2PdRaLacHM5z-cZQr8mjeQu3pik6jPSOGAYYFIqBfgmlkgnY0gmlwhDaRWFWHb3BzdGFja4SzlAUAiXNlY3AyNTZrMaECmeSnJh7zjKrDSPoNMGXoopeDF4hhpj5I0OsQUUt4u8uDdGNwgiQGg3VkcIIkBg",
        "enr:-J64QAlTCDa188Hl1OGv5_2Kj2nWCsvxMVc_rEnLtw7RPFbOfqUOV6khXT_PH6cC603I2ynY31rSQ8sI9gLeJbfFGaWGAYYFIrpdgmlkgnY0gmlwhANWgzCHb3BzdGFja4SzlAUAiXNlY3AyNTZrMaECkySjcg-2v0uWAsFsZZu43qNHppGr2D5F913Qqs5jDCGDdGNwgiQGg3VkcIIkBg",
        "enr:-J24QGEzN4mJgLWNTUNwj7riVJ2ZjRLenOFccl2dbRFxHHOCCZx8SXWzgf-sLzrGs6QgqSFCvGXVgGPBkRkfOWlT1-iGAYe6Cu93gmlkgnY0gmlwhCJBEUSHb3BzdGFja4OkAwCJc2VjcDI1NmsxoQLuYIwaYOHg3CUQhCkS-RsSHmUd1b_x93-9yQ5ItS6udIN0Y3CCIyuDdWRwgiMr",

        // Base bootnodes
        "enr:-J24QNz9lbrKbN4iSmmjtnr7SjUMk4zB7f1krHZcTZx-JRKZd0kA2gjufUROD6T3sOWDVDnFJRvqBBo62zuF-hYCohOGAYiOoEyEgmlkgnY0gmlwhAPniryHb3BzdGFja4OFQgCJc2VjcDI1NmsxoQKNVFlCxh_B-716tTs-h1vMzZkSs1FTu_OYTNjgufplG4N0Y3CCJAaDdWRwgiQG",
        "enr:-J24QH-f1wt99sfpHy4c0QJM-NfmsIfmlLAMMcgZCUEgKG_BBYFc6FwYgaMJMQN5dsRBJApIok0jFn-9CS842lGpLmqGAYiOoDRAgmlkgnY0gmlwhLhIgb2Hb3BzdGFja4OFQgCJc2VjcDI1NmsxoQJ9FTIv8B9myn1MWaC_2lJ-sMoeCDkusCsk4BYHjjCq04N0Y3CCJAaDdWRwgiQG",
        "enr:-J24QDXyyxvQYsd0yfsN0cRr1lZ1N11zGTplMNlW4xNEc7LkPXh0NAJ9iSOVdRO95GPYAIc6xmyoCCG6_0JxdL3a0zaGAYiOoAjFgmlkgnY0gmlwhAPckbGHb3BzdGFja4OFQgCJc2VjcDI1NmsxoQJwoS7tzwxqXSyFL7g0JM-KWVbgvjfB8JA__T7yY_cYboN0Y3CCJAaDdWRwgiQG",
        "enr:-J24QHmGyBwUZXIcsGYMaUqGGSl4CFdx9Tozu-vQCn5bHIQbR7On7dZbU61vYvfrJr30t0iahSqhc64J46MnUO2JvQaGAYiOoCKKgmlkgnY0gmlwhAPnCzSHb3BzdGFja4OFQgCJc2VjcDI1NmsxoQINc4fSijfbNIiGhcgvwjsjxVFJHUstK9L1T8OTKUjgloN0Y3CCJAaDdWRwgiQG",
        "enr:-J24QG3ypT4xSu0gjb5PABCmVxZqBjVw9ca7pvsI8jl4KATYAnxBmfkaIuEqy9sKvDHKuNCsy57WwK9wTt2aQgcaDDyGAYiOoGAXgmlkgnY0gmlwhDbGmZaHb3BzdGFja4OFQgCJc2VjcDI1NmsxoQIeAK_--tcLEiu7HvoUlbV52MspE0uCocsx1f_rYvRenIN0Y3CCJAaDdWRwgiQG",
    ];

    bootnodes
        .iter()
        .filter_map(|enr| Enr::from_str(enr).ok())
        .collect()
}

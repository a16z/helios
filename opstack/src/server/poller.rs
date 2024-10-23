use std::time::Duration;

use alloy::primitives::Address;
use eyre::Result;
use futures::future::join_all;
use reqwest::{Client, ClientBuilder};
use tokio::{sync::mpsc::Sender, time::sleep};
use tracing::warn;
use url::Url;

use crate::SequencerCommitment;

pub fn start(urls: Vec<Url>, signer: Address, chain_id: u64, sender: Sender<SequencerCommitment>) {
    tokio::spawn(async move {
        let client = ClientBuilder::new()
            .timeout(Duration::from_millis(500))
            .build()
            .unwrap();

        let mut final_urls = Vec::new();
        for url in urls {
            if let Ok(replica_chain_id) = get_chain_id(&client, &url).await {
                if chain_id == replica_chain_id {
                    final_urls.push(url);
                } else {
                    warn!("received bad chain id from {}", url);
                }
            } else {
                warn!("received no chain id from {}", url);
            }
        }

        loop {
            join_all(
                final_urls
                    .iter()
                    .map(|url| get_commitment(&client, url, sender.clone(), signer, chain_id)),
            )
            .await;
            sleep(Duration::from_millis(500)).await;
        }
    });
}

async fn get_commitment(
    client: &Client,
    url: &Url,
    sender: Sender<SequencerCommitment>,
    signer: Address,
    chain_id: u64,
) -> Result<()> {
    let commitment = client
        .get(url.join("latest")?)
        .send()
        .await?
        .json::<SequencerCommitment>()
        .await?;

    if commitment.verify(signer, chain_id).is_ok() {
        sender.send(commitment).await?;
    }

    Ok(())
}

async fn get_chain_id(client: &Client, url: &Url) -> Result<u64> {
    let chain_id = client
        .get(url.join("chain_id")?)
        .send()
        .await?
        .json::<u64>()
        .await?;

    Ok(chain_id)
}

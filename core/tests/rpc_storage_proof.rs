#[tokio::test]
async fn rejects_missing_requested_storage_proof() {
    use alloy::primitives::B256;
    use helios_common::execution_provider::AccountProvider;
    use helios_common::execution_provider::BlockProvider;
    use helios_core::execution::providers::block::block_cache::BlockCache;
    use helios_core::execution::providers::rpc::RpcExecutionProvider;
    use helios_ethereum::spec::Ethereum;
    use helios_test_utils::{rpc_block, rpc_proof};
    use jsonrpsee::{server::ServerBuilder, types::ErrorObjectOwned, RpcModule};

    let server = ServerBuilder::default().build("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", server.local_addr().unwrap());
    let proof = rpc_proof();
    let address = proof.address;
    let mut methods = RpcModule::new(proof);
    methods
        .register_method("eth_getProof", |_, proof| {
            Ok::<_, ErrorObjectOwned>(proof.clone())
        })
        .unwrap();
    let handle = server.start(methods);
    let cache = BlockCache::<Ethereum>::new();
    cache
        .push_block(rpc_block(), alloy::eips::BlockId::latest())
        .await;
    let provider = RpcExecutionProvider::<Ethereum, _, ()>::new(url.parse().unwrap(), cache);
    let valid = provider
        .get_account(address, &[], false, alloy::eips::BlockId::latest())
        .await;
    assert!(valid.is_ok(), "{valid:?}");
    let forged = provider
        .get_account(
            address,
            &[B256::repeat_byte(0xff)],
            false,
            alloy::eips::BlockId::latest(),
        )
        .await;
    handle.stop().unwrap();
    assert!(forged.is_err(), "accepted an incomplete storage proof");
}

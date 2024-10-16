use std::path::PathBuf;

use alloy::primitives::{fixed_bytes, B256};
use serde::Deserialize;
use serde_yaml::Value;
use ssz::Decode;
use tree_hash::TreeHash;

use helios_consensus_core::{
    apply_bootstrap, apply_generic_update,
    consensus_spec::MinimalConsensusSpec,
    types::{
        Bootstrap, FinalityUpdate, Fork, Forks, GenericUpdate, LightClientHeader, LightClientStore,
        OptimisticUpdate, Update,
    },
    verify_bootstrap, verify_generic_update,
};

#[test]
fn spec_sync_deneb() {
    let path: PathBuf = "testdata/steps.yaml".parse().unwrap();
    let steps = std::fs::read_to_string(path).unwrap();
    let steps: Vec<Value> = serde_yaml::from_str(&steps).unwrap();

    let mut store = LightClientStore::default();
    let genesis_root =
        fixed_bytes!("0a08c27fe4ece2483f9e581f78c66379a06f96e9c24cd1390594ff939b26f95b");
    let forks = Forks {
        genesis: Fork {
            epoch: 0,
            fork_version: fixed_bytes!("00000001"),
        },
        altair: Fork {
            epoch: 0,
            fork_version: fixed_bytes!("01000001"),
        },
        bellatrix: Fork {
            epoch: 0,
            fork_version: fixed_bytes!("02000001"),
        },
        capella: Fork {
            epoch: 0,
            fork_version: fixed_bytes!("03000001"),
        },
        deneb: Fork {
            epoch: 0,
            fork_version: fixed_bytes!("04000001"),
        },
    };

    let checkpoint =
        fixed_bytes!("c0f6807024e3a40cea50955a9daa481045e44a5e08ccb5aed4d1cd705fc624d4");
    let bootstrap = get_bootstrap("testdata/bootstrap.ssz_snappy".parse().unwrap());

    verify_bootstrap(&bootstrap, checkpoint, &forks).expect("bootstrap failed");
    apply_bootstrap(&mut store, &bootstrap);

    for step in steps {
        let process_update = step
            .as_mapping()
            .unwrap()
            .get("process_update")
            .unwrap()
            .as_mapping()
            .unwrap();

        let update = process_update.get("update").unwrap().as_str().unwrap();
        let update = format!("testdata/{}.ssz_snappy", update);
        let update = get_update(update.parse().unwrap());

        let current_slot = process_update
            .get("current_slot")
            .unwrap()
            .as_u64()
            .unwrap();

        let update_res = verify_generic_update::<MinimalConsensusSpec>(
            &update,
            current_slot,
            &store,
            genesis_root,
            &forks,
        );

        if update_res.is_ok() {
            apply_generic_update::<MinimalConsensusSpec>(&mut store, &update);
            let checks = process_update.get("checks").unwrap().as_mapping().unwrap();
            let finalized_header = checks.get("finalized_header").unwrap();
            let optimistic_header = checks.get("optimistic_header").unwrap();
            check_update(finalized_header, &store.finalized_header);
            check_update(optimistic_header, &store.optimistic_header);
        } else {
            println!("update failed: {:?}", update_res.err().unwrap());
        }
    }
}

fn get_bootstrap(path: PathBuf) -> Bootstrap<MinimalConsensusSpec> {
    let data = std::fs::read(path).unwrap();
    let mut decoder = snap::raw::Decoder::new();
    let decompressed = decoder.decompress_vec(&data).unwrap();
    Bootstrap::from_ssz_bytes(&decompressed).expect("could not decode bootstrap")
}

fn get_update(path: PathBuf) -> GenericUpdate<MinimalConsensusSpec> {
    let data = std::fs::read(path).unwrap();
    let mut decoder = snap::raw::Decoder::new();
    let decompressed = decoder.decompress_vec(&data).unwrap();

    let res = Update::from_ssz_bytes(&decompressed);
    if let Ok(update) = res {
        return GenericUpdate::from(&update);
    }

    let res = FinalityUpdate::from_ssz_bytes(&decompressed);
    if let Ok(update) = res {
        return GenericUpdate::from(&update);
    }

    let res = OptimisticUpdate::from_ssz_bytes(&decompressed);
    if let Ok(update) = res {
        return GenericUpdate::from(&update);
    }

    panic!("could not decode update");
}

#[derive(Deserialize)]
struct Check {
    slot: u64,
    beacon_root: B256,
    execution_root: B256,
}

fn check_update(value: &Value, actual: &LightClientHeader) {
    let expected: Check = serde_yaml::from_value(value.clone()).unwrap();
    assert_eq!(expected.slot, actual.beacon().slot);
    assert_eq!(expected.beacon_root, actual.beacon().tree_hash_root());
    assert_eq!(
        expected.execution_root,
        actual.execution().unwrap().tree_hash_root()
    );
}

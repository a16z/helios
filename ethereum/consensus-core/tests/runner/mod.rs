use std::path::{Path, PathBuf};

use alloy::primitives::{fixed_bytes, B256};
use serde::Deserialize;
use serde_yaml::{Mapping, Value};
use ssz::Decode;
use tree_hash::TreeHash;

use helios_consensus_core::{
    apply_bootstrap, apply_generic_update,
    consensus_spec::MinimalConsensusSpec,
    force_update,
    types::{
        Bootstrap, FinalityUpdate, Fork, Forks, GenericUpdate, LightClientHeader, LightClientStore,
        OptimisticUpdate, Update,
    },
    verify_bootstrap, verify_generic_update,
};

pub fn run<P: Into<PathBuf>>(test_data_dir: P, with_electra: bool) {
    let test_data_dir: PathBuf = test_data_dir.into();
    let steps_file = test_data_dir.join("steps.yaml");
    let steps = std::fs::read_to_string(steps_file).unwrap();
    let steps: Vec<Value> = serde_yaml::from_str(&steps).unwrap();

    let mut store = LightClientStore::default();
    let config = get_meta_config(&test_data_dir);
    let forks = get_forks(with_electra);

    let bootstrap = get_bootstrap(&test_data_dir);
    verify_bootstrap(&bootstrap, config.trusted_block_root, &forks).expect("bootstrap failed");
    apply_bootstrap(&mut store, &bootstrap);

    for step in steps {
        if let Some(step) = step.as_mapping().unwrap().get("process_update") {
            let step = step.as_mapping().unwrap();
            process_update(
                &test_data_dir,
                step,
                &mut store,
                config.genesis_validators_root,
                &forks,
            );
        }

        if let Some(step) = step.as_mapping().unwrap().get("force_update") {
            let step = step.as_mapping().unwrap();
            process_force_update(step, &mut store);
        }
    }
}

fn process_update(
    test_data_dir: &Path,
    step: &Mapping,
    store: &mut LightClientStore<MinimalConsensusSpec>,
    genesis_root: B256,
    forks: &Forks,
) {
    let update = step.get("update").unwrap().as_str().unwrap();
    let path = test_data_dir.join(format!("{}.ssz_snappy", update));
    let update = get_update(path);

    let current_slot = step.get("current_slot").unwrap().as_u64().unwrap();

    let update_res = verify_generic_update::<MinimalConsensusSpec>(
        &update,
        current_slot,
        store,
        genesis_root,
        forks,
    );

    if update_res.is_ok() {
        apply_generic_update::<MinimalConsensusSpec>(store, &update);
        let checks = step.get("checks").unwrap().as_mapping().unwrap();
        let finalized_header = checks.get("finalized_header").unwrap();
        let optimistic_header = checks.get("optimistic_header").unwrap();
        check_update(finalized_header, &store.finalized_header);
        check_update(optimistic_header, &store.optimistic_header);
        println!("update success");
    } else {
        println!("update failed: {:?}", update_res.err().unwrap());
    }
}

fn process_force_update(step: &Mapping, store: &mut LightClientStore<MinimalConsensusSpec>) {
    let current_slot = step.get("current_slot").unwrap().as_u64().unwrap();

    force_update(store, current_slot);

    let checks = step.get("checks").unwrap().as_mapping().unwrap();
    let finalized_header = checks.get("finalized_header").unwrap();
    let optimistic_header = checks.get("optimistic_header").unwrap();
    check_update(finalized_header, &store.finalized_header);
    check_update(optimistic_header, &store.optimistic_header);
    println!("force update success");
}

fn get_bootstrap(test_data_dir: &Path) -> Bootstrap<MinimalConsensusSpec> {
    let path = test_data_dir.join("bootstrap.ssz_snappy");
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

#[derive(Deserialize)]
struct MetaConfig {
    genesis_validators_root: B256,
    trusted_block_root: B256,
}

fn get_meta_config(test_data_dir: &Path) -> MetaConfig {
    let file = test_data_dir.join("meta.yaml");
    let meta = std::fs::read_to_string(file).unwrap();
    serde_yaml::from_str(&meta).unwrap()
}

fn get_forks(with_electra: bool) -> Forks {
    Forks {
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
        electra: Fork {
            epoch: if with_electra { 0 } else { u64::MAX },
            fork_version: fixed_bytes!("05000001"),
        },
    }
}

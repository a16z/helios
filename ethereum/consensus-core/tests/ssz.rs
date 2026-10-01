use std::path::{Path, PathBuf};

use helios_consensus_core::{
    consensus_spec::{ConsensusSpec, MainnetConsensusSpec, MinimalConsensusSpec},
    types::{Bootstrap, FinalityUpdate, LightClientHeader, OptimisticUpdate, Update},
};
use serde::Serialize;
use ssz::Decode;

#[test]
#[ignore = "set CONSENSUS_SPEC_TESTS to the extracted tests directory"]
fn official_gloas_ssz_decoding() {
    let root = PathBuf::from(std::env::var("CONSENSUS_SPEC_TESTS").expect("CONSENSUS_SPEC_TESTS"));
    check_preset::<MinimalConsensusSpec>(&root.join("minimal/gloas/ssz_static"));
    check_preset::<MainnetConsensusSpec>(&root.join("mainnet/gloas/ssz_static"));
}

fn check_preset<S: ConsensusSpec>(root: &Path) {
    check_type::<LightClientHeader>(&root.join("LightClientHeader"));
    check_type::<Bootstrap<S>>(&root.join("LightClientBootstrap"));
    check_type::<Update<S>>(&root.join("LightClientUpdate"));
    check_type::<FinalityUpdate<S>>(&root.join("LightClientFinalityUpdate"));
    check_type::<OptimisticUpdate<S>>(&root.join("LightClientOptimisticUpdate"));
}

fn check_type<T: Decode + Serialize>(root: &Path) {
    let cases = std::fs::read_dir(root.join("ssz_random")).unwrap();
    let mut count = 0;
    for entry in cases {
        let case = entry.unwrap().path();
        let compressed = std::fs::read(case.join("serialized.ssz_snappy")).unwrap();
        let bytes = snap::raw::Decoder::new()
            .decompress_vec(&compressed)
            .unwrap();
        let actual = T::from_ssz_bytes(&bytes).unwrap();
        let mut expected: serde_json::Value =
            serde_yaml::from_str(&std::fs::read_to_string(case.join("value.yaml")).unwrap())
                .unwrap();
        // The Beacon API quotes uint64s; the official YAML fixtures use numbers.
        quote_numbers(&mut expected);
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            expected,
            "{}",
            case.display()
        );
        count += 1;
    }
    assert!(count > 0, "no vectors in {}", root.display());
}

fn quote_numbers(value: &mut serde_json::Value) {
    use serde_json::Value;
    match value {
        Value::Number(number) => *value = Value::String(number.to_string()),
        Value::Object(fields) => fields.values_mut().for_each(quote_numbers),
        Value::Array(values) => values.iter_mut().for_each(quote_numbers),
        _ => {}
    }
}

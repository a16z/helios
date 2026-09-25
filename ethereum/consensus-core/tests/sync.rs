mod runner;

/// Run the downloaded consensus-specs release without checking large archives into git.
#[test]
#[ignore = "set CONSENSUS_SPEC_TESTS to the extracted tests directory"]
fn official_sync_vectors() {
    use std::path::PathBuf;

    let root = PathBuf::from(std::env::var("CONSENSUS_SPEC_TESTS").expect("CONSENSUS_SPEC_TESTS"));
    for fork in ["capella", "deneb", "electra", "fulu", "gloas"] {
        let cases = root
            .join("minimal")
            .join(fork)
            .join("light_client/sync/pyspec_tests");
        let mut cases: Vec<_> = std::fs::read_dir(cases)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.is_dir())
            .collect();
        cases.sort();
        assert!(!cases.is_empty());
        for case in cases {
            println!("{}", case.display());
            runner::run(case);
        }
    }
}

#[test]
fn gloas_fork_transitions() {
    for (fork, case) in [
        ("capella", "deneb_gloas_fork"),
        ("deneb", "electra_gloas_fork"),
        ("fulu", "gloas_fork"),
    ] {
        runner::run(format!("testdata/{fork}/{case}"));
    }
}

#[test]
fn gloas_store_with_legacy_data() {
    for fork in ["capella", "deneb", "electra", "fulu"] {
        runner::run(format!("testdata/{fork}/gloas_store_with_legacy_data"));
    }
}

#[test]
fn light_client_sync_deneb() {
    runner::run("testdata/deneb/light_client_sync");
}

#[test]
fn supply_sync_committee_from_past_update_deneb() {
    runner::run("testdata/deneb/supply_sync_committee_from_past_update");
}

#[test]
fn advance_finality_without_sync_committee_deneb() {
    runner::run("testdata/deneb/advance_finality_without_sync_committee");
}

#[test]
fn light_client_sync_electra() {
    runner::run("testdata/electra/light_client_sync");
}

#[test]
fn supply_sync_committee_from_past_update_electra() {
    runner::run("testdata/electra/supply_sync_committee_from_past_update");
}

#[test]
fn advance_finality_without_sync_committee_electra() {
    runner::run("testdata/electra/advance_finality_without_sync_committee");
}

#[test]
fn light_client_sync_gloas() {
    runner::run("testdata/gloas/light_client_sync");
}

#[test]
fn light_client_sync_no_force_update_gloas() {
    runner::run("testdata/gloas/light_client_sync_no_force_update");
}

#[test]
fn supply_sync_committee_from_past_update_gloas() {
    runner::run("testdata/gloas/supply_sync_committee_from_past_update");
}

#[test]
fn advance_finality_without_sync_committee_gloas() {
    runner::run("testdata/gloas/advance_finality_without_sync_committee");
}

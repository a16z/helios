mod runner;

#[test]
fn light_client_sync_deneb() {
    runner::run("testdata/deneb/light_client_sync", false);
}

#[test]
fn supply_sync_committee_from_past_update_deneb() {
    runner::run(
        "testdata/deneb/supply_sync_committee_from_past_update",
        false,
    );
}

#[test]
fn advance_finality_without_sync_committee_deneb() {
    runner::run(
        "testdata/deneb/advance_finality_without_sync_committee",
        false,
    );
}

#[test]
fn light_client_sync_electra() {
    runner::run("testdata/electra/light_client_sync", true);
}

#[test]
fn supply_sync_committee_from_past_update_electra() {
    runner::run(
        "testdata/electra/supply_sync_committee_from_past_update",
        true,
    );
}

#[test]
fn advance_finality_without_sync_committee_electra() {
    runner::run(
        "testdata/electra/advance_finality_without_sync_committee",
        true,
    );
}

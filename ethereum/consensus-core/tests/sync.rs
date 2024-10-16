mod runner;

#[test]
fn light_client_sync_deneb() {
    runner::run("testdata/deneb/light_client_sync");
}

#[test]
fn supply_sync_committee_from_past_update() {
    runner::run("testdata/deneb/supply_sync_committee_from_past_update");
}

#[test]
fn advance_finality_without_sync_committee() {
    runner::run("testdata/deneb/advance_finality_without_sync_committee");
}

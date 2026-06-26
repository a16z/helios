mod runner;

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
fn supply_sync_committee_from_past_update_gloas() {
    runner::run("testdata/gloas/supply_sync_committee_from_past_update");
}

#[test]
fn advance_finality_without_sync_committee_gloas() {
    runner::run("testdata/gloas/advance_finality_without_sync_committee");
}

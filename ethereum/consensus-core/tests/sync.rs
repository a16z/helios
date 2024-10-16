mod runner;

#[test]
fn spec_sync_deneb() {
    runner::run("testdata/deneb/sync");
}

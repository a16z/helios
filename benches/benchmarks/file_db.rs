use alloy::primitives::b256;
use criterion::{criterion_group, criterion_main, Criterion};
use helios_ethereum::{
    config::Config,
    database::{Database, FileDB},
};
use tempfile::tempdir;

#[path = "../lib/harness.rs"]
mod harness;

criterion_group! {
    name = file_db;
    config = Criterion::default();
    targets = save_checkpoint, load_checkpoint
}

criterion_main!(file_db);

/// Benchmark saving/writing a checkpoint to the file db.
pub fn save_checkpoint(c: &mut Criterion) {
    c.bench_function("save_checkpoint", |b| {
        let checkpoint: alloy::primitives::FixedBytes<32> =
            b256!("c7fc7b2f4b548bfc9305fa80bc1865ddc6eea4557f0a80507af5dc34db7bd9ce");
        b.iter(|| {
            let data_dir = Some(tempdir().unwrap().into_path());
            let config = Config {
                data_dir,
                ..Default::default()
            };
            let db = FileDB::new(&config).unwrap();
            db.save_checkpoint(checkpoint).unwrap();
        })
    });
}

/// Benchmark loading a checkpoint from the file db.
pub fn load_checkpoint(c: &mut Criterion) {
    c.bench_function("load_checkpoint", |b| {
        // First write to the db
        let data_dir = Some(tempdir().unwrap().into_path());
        let config = Config {
            data_dir,
            ..Default::default()
        };
        let db = FileDB::new(&config).unwrap();
        let written_checkpoint =
            b256!("c7fc7b2f4b548bfc9305fa80bc1865ddc6eea4557f0a80507af5dc34db7bd9ce");
        db.save_checkpoint(written_checkpoint.clone()).unwrap();

        // Then read from the db
        b.iter(|| {
            let checkpoint = db.load_checkpoint().unwrap();
            assert_eq!(checkpoint, written_checkpoint.clone());
        })
    });
}

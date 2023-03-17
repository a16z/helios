use client::database::Database;
use config::Config;
use criterion::{criterion_group, criterion_main, Criterion};
use helios::prelude::FileDB;
use tempfile::tempdir;

mod harness;

criterion_main!(file_db);
criterion_group! {
    name = file_db;
    config = Criterion::default();
    targets = save_checkpoint, load_checkpoint
}

/// Benchmark saving/writing a checkpoint to the file db.
pub fn save_checkpoint(c: &mut Criterion) {
    c.bench_function("save_checkpoint", |b| {
        let checkpoint = vec![1, 2, 3];
        b.iter(|| {
            let data_dir = Some(tempdir().unwrap().into_path());
            let config = Config {
                data_dir,
                ..Default::default()
            };
            let db = FileDB::new(&config).unwrap();
            db.save_checkpoint(checkpoint.clone()).unwrap();
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
        let written_checkpoint = vec![1; 32];
        db.save_checkpoint(written_checkpoint.clone()).unwrap();

        // Then read from the db
        b.iter(|| {
            let checkpoint = db.load_checkpoint().unwrap();
            assert_eq!(checkpoint, written_checkpoint.clone());
        })
    });
}

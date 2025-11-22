use acme_disk_use::DiskUse;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::env;
use std::path::PathBuf;

fn benchmark_real_data(c: &mut Criterion) {
    let mut group = c.benchmark_group("real_data");
    group.sample_size(10); // Reduce sample size as this might be slow

    // Locate the benchmark_data directory
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let data_dir = PathBuf::from(manifest_dir).join("benches/benchmark_data");

    if !data_dir.exists() {
        eprintln!("Benchmark data not found at {:?}. Skipping.", data_dir);
        return;
    }

    // Cold Cache Benchmark
    group.bench_function("cold_cache", |b| {
        b.iter_batched(
            || {
                // Setup: Ensure cache doesn't exist
                let cache_path = PathBuf::from("/tmp/acme_bench_cold.bin");
                if cache_path.exists() {
                    std::fs::remove_file(&cache_path).unwrap();
                }
                cache_path
            },
            |cache_path| {
                let mut disk_use = DiskUse::new(cache_path);
                let stats = disk_use.scan_with_options(&data_dir, true).unwrap();
                black_box(stats);
            },
            criterion::BatchSize::PerIteration,
        );
    });

    // Warm Cache Benchmark
    group.bench_function("warm_cache", |b| {
        b.iter_batched(
            || {
                // Setup: Create a warm cache
                let cache_path = PathBuf::from("/tmp/acme_bench_warm.bin");
                let mut disk_use = DiskUse::new(cache_path.clone());
                disk_use.scan_with_options(&data_dir, true).unwrap(); // Force fresh scan
                disk_use.save_cache().unwrap();
                cache_path
            },
            |cache_path| {
                let mut disk_use = DiskUse::new(cache_path);
                // Scan without ignore_cache (should use cache)
                let stats = disk_use.scan_with_options(&data_dir, false).unwrap();
                black_box(stats);
            },
            criterion::BatchSize::PerIteration,
        );
    });

    group.finish();
}

criterion_group!(benches, benchmark_real_data);
criterion_main!(benches);

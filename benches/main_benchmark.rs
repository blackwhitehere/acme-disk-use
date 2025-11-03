use acme_disk_use::DiskUse;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Create a test directory structure with specified parameters
///
/// # Arguments
/// * `base_dir` - Base directory to create structure in
/// * `depth` - How many levels deep to nest directories
/// * `files_per_dir` - Number of files to create in each directory
/// * `subdirs_per_dir` - Number of subdirectories in each directory
/// * `file_size` - Size of each file in bytes
fn create_test_structure(
    base_dir: &std::path::Path,
    depth: usize,
    files_per_dir: usize,
    subdirs_per_dir: usize,
    file_size: usize,
) -> std::io::Result<usize> {
    let mut total_files = 0;

    // Create files in current directory
    for i in 0..files_per_dir {
        let file_path = base_dir.join(format!("file_{}.dat", i + 1));
        let content = vec![b'x'; file_size];
        fs::write(file_path, content)?;
        total_files += 1;
    }

    // Recursively create subdirectories
    if depth > 0 {
        for i in 0..subdirs_per_dir {
            let subdir = base_dir.join(format!("subdir_{}", i + 1));
            fs::create_dir(&subdir)?;
            total_files += create_test_structure(
                &subdir,
                depth - 1,
                files_per_dir,
                subdirs_per_dir,
                file_size,
            )?;
        }
    }

    Ok(total_files)
}

/// Configuration for a benchmark scenario
#[derive(Clone)]
struct BenchConfig {
    name: &'static str,
    depth: usize,
    files_per_dir: usize,
    subdirs_per_dir: usize,
    file_size: usize,
}

impl BenchConfig {
    #[allow(dead_code)]
    fn estimated_files(&self) -> usize {
        // Calculate total files: files_per_dir * (sum of subdirs^i for i=0 to depth)
        let mut total = 0;
        let mut subdirs_at_level = 1;
        for _ in 0..=self.depth {
            total += self.files_per_dir * subdirs_at_level;
            subdirs_at_level *= self.subdirs_per_dir;
        }
        total
    }
}

/// Benchmark configurations for different directory sizes
fn get_benchmark_configs() -> Vec<BenchConfig> {
    vec![
        BenchConfig {
            name: "tiny",
            depth: 2,
            files_per_dir: 3,
            subdirs_per_dir: 2,
            file_size: 512,
        },
        BenchConfig {
            name: "small",
            depth: 3,
            files_per_dir: 5,
            subdirs_per_dir: 2,
            file_size: 1024,
        },
        BenchConfig {
            name: "medium",
            depth: 4,
            files_per_dir: 10,
            subdirs_per_dir: 3,
            file_size: 2048,
        },
        BenchConfig {
            name: "large",
            depth: 4,
            files_per_dir: 15,
            subdirs_per_dir: 4,
            file_size: 4096,
        },
    ]
}

/// Benchmark cold cache (first scan)
fn benchmark_cold_cache(c: &mut Criterion) {
    let mut group = c.benchmark_group("cold_cache");

    for config in get_benchmark_configs() {
        group.bench_with_input(BenchmarkId::new("scan", config.name), &config, |b, cfg| {
            b.iter_batched(
                || {
                    // Setup: Create temp directory with test structure
                    let temp_dir = TempDir::new().unwrap();
                    let test_dir = temp_dir.path().join("test");
                    fs::create_dir(&test_dir).unwrap();

                    create_test_structure(
                        &test_dir,
                        cfg.depth,
                        cfg.files_per_dir,
                        cfg.subdirs_per_dir,
                        cfg.file_size,
                    )
                    .unwrap();

                    (temp_dir, test_dir)
                },
                |(temp_dir, test_dir)| {
                    // Benchmark: Cold cache scan
                    let mut disk_use = DiskUse::new(PathBuf::from("/tmp/bench_cache.bin"));
                    let stats = disk_use.scan_with_options(&test_dir, true).unwrap();
                    black_box(stats);
                    drop(temp_dir); // Clean up
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

/// Benchmark warm cache (subsequent scans with no changes)
fn benchmark_warm_cache(c: &mut Criterion) {
    let mut group = c.benchmark_group("warm_cache");

    for config in get_benchmark_configs() {
        group.bench_with_input(BenchmarkId::new("scan", config.name), &config, |b, cfg| {
            b.iter_batched(
                || {
                    // Setup: Create temp directory and do initial scan
                    let temp_dir = TempDir::new().unwrap();
                    let test_dir = temp_dir.path().join("test");
                    fs::create_dir(&test_dir).unwrap();

                    create_test_structure(
                        &test_dir,
                        cfg.depth,
                        cfg.files_per_dir,
                        cfg.subdirs_per_dir,
                        cfg.file_size,
                    )
                    .unwrap();

                    let cache_path = temp_dir.path().join("cache.bin");
                    let mut disk_use = DiskUse::new(cache_path.clone());

                    // Initial scan to populate cache
                    disk_use.scan_with_options(&test_dir, false).unwrap();
                    disk_use.save_cache().unwrap();

                    (temp_dir, test_dir, cache_path)
                },
                |(temp_dir, test_dir, cache_path)| {
                    // Benchmark: Warm cache scan (should use cache)
                    let mut disk_use = DiskUse::new(cache_path);
                    let stats = disk_use.scan_with_options(&test_dir, false).unwrap();
                    black_box(stats);
                    drop(temp_dir); // Clean up
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

/// Benchmark cache invalidation (detecting new files)
fn benchmark_cache_invalidation(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_invalidation");

    for config in get_benchmark_configs() {
        group.bench_with_input(
            BenchmarkId::new("scan_with_change", config.name),
            &config,
            |b, cfg| {
                b.iter_batched(
                    || {
                        // Setup: Create directory, scan, then add a new file
                        let temp_dir = TempDir::new().unwrap();
                        let test_dir = temp_dir.path().join("test");
                        fs::create_dir(&test_dir).unwrap();

                        create_test_structure(
                            &test_dir,
                            cfg.depth,
                            cfg.files_per_dir,
                            cfg.subdirs_per_dir,
                            cfg.file_size,
                        )
                        .unwrap();

                        let cache_path = temp_dir.path().join("cache.bin");
                        let mut disk_use = DiskUse::new(cache_path.clone());

                        // Initial scan
                        disk_use.scan_with_options(&test_dir, false).unwrap();
                        disk_use.save_cache().unwrap();

                        // Add a new file to trigger cache invalidation
                        fs::write(test_dir.join("new_file.dat"), vec![b'y'; cfg.file_size])
                            .unwrap();

                        (temp_dir, test_dir, cache_path)
                    },
                    |(temp_dir, test_dir, cache_path)| {
                        // Benchmark: Scan with cache invalidation
                        let mut disk_use = DiskUse::new(cache_path);
                        let stats = disk_use.scan_with_options(&test_dir, false).unwrap();
                        black_box(stats);
                        drop(temp_dir); // Clean up
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark format_size function
fn benchmark_format_size(c: &mut Criterion) {
    use acme_disk_use::format_size;

    let mut group = c.benchmark_group("format_size");

    let sizes = vec![
        ("bytes", 512),
        ("kilobytes", 1024 * 512),
        ("megabytes", 1024 * 1024 * 512),
        ("gigabytes", 1024_u64 * 1024 * 1024 * 512),
    ];

    for (name, size) in sizes {
        group.bench_with_input(BenchmarkId::new("human_readable", name), &size, |b, &s| {
            b.iter(|| black_box(format_size(black_box(s), true)))
        });

        group.bench_with_input(BenchmarkId::new("raw_bytes", name), &size, |b, &s| {
            b.iter(|| black_box(format_size(black_box(s), false)))
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_cold_cache,
    benchmark_warm_cache,
    benchmark_cache_invalidation,
    benchmark_format_size
);
criterion_main!(benches);

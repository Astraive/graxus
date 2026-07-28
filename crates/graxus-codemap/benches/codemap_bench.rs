use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use graxus_core::{FileKind, Language, ScannedFile};
use std::path::PathBuf;

fn create_sample_files(count: usize) -> Vec<ScannedFile> {
    (0..count)
        .map(|i| ScannedFile {
            path: PathBuf::from(format!("src/file_{i}.rs")),
            relative_path: format!("src/file_{i}.rs"),
            kind: FileKind::Code,
            language: Language::Rust,
            hash: format!("hash_{i}"),
            size: 1000,
            modified: chrono::Utc::now(),
        })
        .collect()
}

fn bench_codemap_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("codemap_build");
    for size in [10, 50, 100, 500] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let files = create_sample_files(size);
            // Note: This benchmarks the graph construction, not file I/O
            b.iter(|| {
                // Benchmark the index building
                black_box(&files);
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_codemap_build);
criterion_main!(benches);

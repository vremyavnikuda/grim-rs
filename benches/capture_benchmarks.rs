use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use grim_rs::{Box as GrimBox, Grim};

fn benchmark_capture_all(c: &mut Criterion) {
    let mut group = c.benchmark_group("capture_all");

    group.bench_function("capture_all", |b| {
        b.iter(|| {
            let mut grim = Grim::new().expect("Failed to create Grim");
            let result = grim.capture_all().expect("Failed to capture");
            black_box(result);
        });
    });

    group.finish();
}

fn benchmark_capture_with_scale(c: &mut Criterion) {
    let mut group = c.benchmark_group("capture_scale");

    for scale in [0.5, 1.0, 2.0].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(scale), scale, |b, &scale| {
            b.iter(|| {
                let mut grim = Grim::new().expect("Failed to create Grim");
                let result = grim
                    .capture_all_with_scale(scale)
                    .expect("Failed to capture");
                black_box(result);
            });
        });
    }

    group.finish();
}

fn benchmark_capture_region(c: &mut Criterion) {
    let mut group = c.benchmark_group("capture_region");

    let regions = [
        ("small_100x100", GrimBox::new(0, 0, 100, 100)),
        ("medium_500x500", GrimBox::new(0, 0, 500, 500)),
        ("large_1920x1080", GrimBox::new(0, 0, 1920, 1080)),
    ];

    for (name, region) in regions.iter() {
        group.bench_with_input(BenchmarkId::from_parameter(name), region, |b, region| {
            b.iter(|| {
                let mut grim = Grim::new().expect("Failed to create Grim");
                let result = grim.capture_region(*region).expect("Failed to capture");
                black_box(result);
            });
        });
    }

    group.finish();
}

fn benchmark_get_outputs(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_outputs");

    group.bench_function("get_outputs", |b| {
        b.iter(|| {
            let mut grim = Grim::new().expect("Failed to create Grim");
            let outputs = grim.get_outputs().expect("Failed to get outputs");
            black_box(outputs);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_capture_all,
    benchmark_capture_with_scale,
    benchmark_capture_region,
    benchmark_get_outputs
);
criterion_main!(benches);

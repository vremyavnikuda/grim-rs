use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use grim_rs::Grim;

fn generate_test_data(width: u32, height: u32) -> Vec<u8> {
    let size = (width * height * 4) as usize;
    vec![0xAA; size]
}

fn benchmark_png_encoding(c: &mut Criterion) {
    let mut group = c.benchmark_group("png_encoding");

    let sizes = [
        ("640x480", 640, 480),
        ("1920x1080", 1920, 1080),
        ("3840x2160", 3840, 2160),
    ];

    for (name, width, height) in sizes.iter() {
        let data = generate_test_data(*width, *height);
        let bytes = data.len() as u64;

        group.throughput(Throughput::Bytes(bytes));
        group.bench_with_input(BenchmarkId::from_parameter(name), &data, |b, data| {
            b.iter(|| {
                let grim = Grim::new().expect("Failed to create Grim");
                let result = grim
                    .to_png(data, *width, *height)
                    .expect("Failed to encode PNG");
                black_box(result);
            });
        });
    }

    group.finish();
}

fn benchmark_png_compression_levels(c: &mut Criterion) {
    let mut group = c.benchmark_group("png_compression_levels");

    let width = 1920;
    let height = 1080;
    let data = generate_test_data(width, height);

    for level in [1, 6, 9].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(level), level, |b, &level| {
            b.iter(|| {
                let grim = Grim::new().expect("Failed to create Grim");
                let result = grim
                    .to_png_with_compression(&data, width, height, level)
                    .expect("Failed to encode PNG");
                black_box(result);
            });
        });
    }

    group.finish();
}

#[cfg(feature = "jpeg")]
fn benchmark_jpeg_encoding(c: &mut Criterion) {
    let mut group = c.benchmark_group("jpeg_encoding");

    let sizes = [
        ("640x480", 640, 480),
        ("1920x1080", 1920, 1080),
        ("3840x2160", 3840, 2160),
    ];

    for (name, width, height) in sizes.iter() {
        let data = generate_test_data(*width, *height);
        let bytes = data.len() as u64;

        group.throughput(Throughput::Bytes(bytes));
        group.bench_with_input(BenchmarkId::from_parameter(name), &data, |b, data| {
            b.iter(|| {
                let grim = Grim::new().expect("Failed to create Grim");
                let result = grim
                    .to_jpeg(data, *width, *height)
                    .expect("Failed to encode JPEG");
                black_box(result);
            });
        });
    }

    group.finish();
}

#[cfg(feature = "jpeg")]
fn benchmark_jpeg_quality_levels(c: &mut Criterion) {
    let mut group = c.benchmark_group("jpeg_quality_levels");

    let width = 1920;
    let height = 1080;
    let data = generate_test_data(width, height);

    for quality in [60, 80, 95].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(quality),
            quality,
            |b, &quality| {
                b.iter(|| {
                    let grim = Grim::new().expect("Failed to create Grim");
                    let result = grim
                        .to_jpeg_with_quality(&data, width, height, quality)
                        .expect("Failed to encode JPEG");
                    black_box(result);
                });
            },
        );
    }

    group.finish();
}

fn benchmark_ppm_encoding(c: &mut Criterion) {
    let mut group = c.benchmark_group("ppm_encoding");

    let sizes = [
        ("640x480", 640, 480),
        ("1920x1080", 1920, 1080),
        ("3840x2160", 3840, 2160),
    ];

    for (name, width, height) in sizes.iter() {
        let data = generate_test_data(*width, *height);
        let bytes = data.len() as u64;

        group.throughput(Throughput::Bytes(bytes));
        group.bench_with_input(BenchmarkId::from_parameter(name), &data, |b, data| {
            b.iter(|| {
                let grim = Grim::new().expect("Failed to create Grim");
                let result = grim
                    .to_ppm(data, *width, *height)
                    .expect("Failed to encode PPM");
                black_box(result);
            });
        });
    }

    group.finish();
}

#[cfg(feature = "jpeg")]
criterion_group!(
    benches,
    benchmark_png_encoding,
    benchmark_png_compression_levels,
    benchmark_jpeg_encoding,
    benchmark_jpeg_quality_levels,
    benchmark_ppm_encoding
);

#[cfg(not(feature = "jpeg"))]
criterion_group!(
    benches,
    benchmark_png_encoding,
    benchmark_png_compression_levels,
    benchmark_ppm_encoding
);

criterion_main!(benches);

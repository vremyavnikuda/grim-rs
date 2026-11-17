use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use grim_rs::{CaptureParameters, Grim, Region};

fn benchmark_capture_all(c: &mut Criterion) {
    let mut group = c.benchmark_group("capture_all");
    group.bench_function("capture_all", |b| {
        let mut grim = Grim::new().expect("Failed to create Grim");
        b.iter(|| {
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
            let mut grim = Grim::new().expect("Failed to create Grim");
            b.iter(|| {
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
        ("small_100x100", Region::new(0, 0, 100, 100)),
        ("medium_500x500", Region::new(0, 0, 500, 500)),
        ("large_1920x1080", Region::new(0, 0, 1920, 1080)),
    ];
    for (name, region) in regions.iter() {
        group.bench_with_input(BenchmarkId::from_parameter(name), region, |b, region| {
            let mut grim = Grim::new().expect("Failed to create Grim");
            b.iter(|| {
                let result = grim.capture_region(*region).expect("Failed to capture");
                black_box(result);
            });
        });
    }
    group.finish();
}

fn benchmark_capture_region_with_scale(c: &mut Criterion) {
    let mut group = c.benchmark_group("capture_region_scale");
    let mut grim = Grim::new().expect("Failed to create Grim");
    let outputs = grim.get_outputs().expect("Failed to get outputs");
    let output = match outputs.first() {
        Some(output) => output,
        None => return,
    };
    let geom = output.geometry();
    let region = Region::new(
        geom.x() + geom.width() / 4,
        geom.y() + geom.height() / 4,
        (geom.width() / 2).max(1),
        (geom.height() / 2).max(1),
    );
    for scale in [0.5, 1.0].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(scale), scale, |b, &scale| {
            b.iter(|| {
                let result = grim
                    .capture_region_with_scale(region, scale)
                    .expect("Failed to capture");
                black_box(result);
            });
        });
    }
    group.finish();
}

fn benchmark_capture_output(c: &mut Criterion) {
    let mut group = c.benchmark_group("capture_output");
    let mut grim = Grim::new().expect("Failed to create Grim");
    let outputs = grim.get_outputs().expect("Failed to get outputs");
    let output = match outputs.first() {
        Some(output) => output,
        None => return,
    };
    let name = output.name().to_string();
    group.bench_function("capture_output", |b| {
        b.iter(|| {
            let result = grim.capture_output(&name).expect("Failed to capture");
            black_box(result);
        });
    });
    group.finish();
}

fn benchmark_capture_output_with_scale(c: &mut Criterion) {
    let mut group = c.benchmark_group("capture_output_scale");
    let mut grim = Grim::new().expect("Failed to create Grim");
    let outputs = grim.get_outputs().expect("Failed to get outputs");
    let output = match outputs.first() {
        Some(output) => output,
        None => return,
    };
    let name = output.name().to_string();
    for scale in [0.5, 1.0].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(scale), scale, |b, &scale| {
            b.iter(|| {
                let result = grim
                    .capture_output_with_scale(&name, scale)
                    .expect("Failed to capture");
                black_box(result);
            });
        });
    }
    group.finish();
}

fn benchmark_capture_outputs(c: &mut Criterion) {
    let mut group = c.benchmark_group("capture_outputs");
    let mut grim = Grim::new().expect("Failed to create Grim");
    let outputs = grim.get_outputs().expect("Failed to get outputs");
    if outputs.is_empty() {
        return;
    }
    let params: Vec<CaptureParameters> = outputs
        .iter()
        .enumerate()
        .map(|(idx, output)| {
            let mut params = CaptureParameters::new(output.name());
            if idx == 0 {
                params = params.overlay_cursor(true);
            }
            let geom = output.geometry();
            let region = Region::new(0, 0, (geom.width() / 2).max(1), (geom.height() / 2).max(1));
            params.region(region)
        })
        .collect();
    group.bench_function("capture_outputs", |b| {
        b.iter(|| {
            let result = grim
                .capture_outputs(params.clone())
                .expect("Failed to capture");
            black_box(result);
        });
    });
    group.finish();
}

fn benchmark_capture_outputs_with_scale(c: &mut Criterion) {
    let mut group = c.benchmark_group("capture_outputs_scale");
    let mut grim = Grim::new().expect("Failed to create Grim");
    let outputs = grim.get_outputs().expect("Failed to get outputs");
    if outputs.is_empty() {
        return;
    }
    let params: Vec<CaptureParameters> = outputs
        .iter()
        .map(|output| {
            let geom = output.geometry();
            let region = Region::new(0, 0, (geom.width() / 2).max(1), (geom.height() / 2).max(1));
            CaptureParameters::new(output.name()).region(region)
        })
        .collect();
    for scale in [0.5, 1.0].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(scale), scale, |b, &scale| {
            b.iter(|| {
                let result = grim
                    .capture_outputs_with_scale(params.clone(), scale)
                    .expect("Failed to capture");
                black_box(result);
            });
        });
    }
    group.finish();
}

fn benchmark_get_outputs(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_outputs");
    group.bench_function("get_outputs", |b| {
        let mut grim = Grim::new().expect("Failed to create Grim");
        b.iter(|| {
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
    benchmark_capture_region_with_scale,
    benchmark_capture_output,
    benchmark_capture_output_with_scale,
    benchmark_capture_outputs,
    benchmark_capture_outputs_with_scale,
    benchmark_get_outputs
);

criterion_main!(benches);

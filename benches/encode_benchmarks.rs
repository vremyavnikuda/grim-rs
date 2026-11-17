use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use grim_rs::Grim;

#[cfg(unix)]
use std::ffi::CString;

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;

#[cfg(unix)]
struct StdoutSilencer {
    saved_fd: i32,
}

#[cfg(unix)]
impl StdoutSilencer {
    fn new() -> Option<Self> {
        unsafe {
            let devnull_path = CString::new("/dev/null").ok()?;
            let devnull = libc::open(devnull_path.as_ptr(), libc::O_WRONLY);
            if devnull < 0 {
                return None;
            }
            let saved = libc::dup(libc::STDOUT_FILENO);
            if saved < 0 {
                libc::close(devnull);
                return None;
            }
            if libc::dup2(devnull, libc::STDOUT_FILENO) < 0 {
                libc::close(devnull);
                libc::close(saved);
                return None;
            }
            libc::close(devnull);
            Some(Self { saved_fd: saved })
        }
    }
}

#[cfg(unix)]
impl Drop for StdoutSilencer {
    fn drop(&mut self) {
        unsafe {
            libc::dup2(self.saved_fd, libc::STDOUT_FILENO);
            libc::close(self.saved_fd);
        }
    }
}

#[cfg(not(any(unix, windows)))]
struct StdoutSilencer;

#[cfg(not(any(unix, windows)))]
impl StdoutSilencer {
    fn new() -> Option<Self> {
        None
    }
}

#[cfg(windows)]
mod win32 {
    pub const STD_OUTPUT_HANDLE: u32 = 0xFFFFFFF5;
    pub const GENERIC_WRITE: u32 = 0x40000000;
    pub const OPEN_EXISTING: u32 = 3;
    extern "system" {
        pub fn GetStdHandle(nStdHandle: u32) -> isize;
        pub fn SetStdHandle(nStdHandle: u32, hHandle: isize) -> i32;
        pub fn CreateFileW(
            lpFileName: *const u16,
            dwDesiredAccess: u32,
            dwShareMode: u32,
            lpSecurityAttributes: *mut std::ffi::c_void,
            dwCreationDisposition: u32,
            dwFlagsAndAttributes: u32,
            hTemplateFile: isize,
        ) -> isize;
        pub fn CloseHandle(hObject: isize) -> i32;
    }
}

#[cfg(windows)]
struct StdoutSilencer {
    saved_handle: isize,
    nul_handle: isize,
}

#[cfg(windows)]
impl StdoutSilencer {
    fn new() -> Option<Self> {
        unsafe {
            let saved = win32::GetStdHandle(win32::STD_OUTPUT_HANDLE);
            if saved == -1 || saved == 0 {
                return None;
            }
            let nul_path: Vec<u16> = std::ffi::OsStr::new("NUL")
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let nul_handle = win32::CreateFileW(
                nul_path.as_ptr(),
                win32::GENERIC_WRITE,
                0,
                std::ptr::null_mut(),
                win32::OPEN_EXISTING,
                0,
                0,
            );
            if nul_handle == -1 {
                return None;
            }
            win32::SetStdHandle(win32::STD_OUTPUT_HANDLE, nul_handle);
            Some(Self {
                saved_handle: saved,
                nul_handle,
            })
        }
    }
}

#[cfg(windows)]
impl Drop for StdoutSilencer {
    fn drop(&mut self) {
        unsafe {
            win32::SetStdHandle(win32::STD_OUTPUT_HANDLE, self.saved_handle);
            win32::CloseHandle(self.nul_handle);
        }
    }
}

fn temp_path(prefix: &str, ext: &str, width: u32, height: u32) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "grim-rs-{}-{}x{}-{}.{}",
        prefix,
        width,
        height,
        std::process::id(),
        ext
    ));
    path
}

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
            let grim = Grim::new().expect("Failed to create Grim");
            b.iter(|| {
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
            let grim = Grim::new().expect("Failed to create Grim");
            b.iter(|| {
                let result = grim
                    .to_png_with_compression(&data, width, height, level)
                    .expect("Failed to encode PNG");
                black_box(result);
            });
        });
    }
    group.finish();
}

fn benchmark_png_save(c: &mut Criterion) {
    let mut group = c.benchmark_group("png_save");
    let sizes = [
        ("640x480", 640, 480),
        ("1920x1080", 1920, 1080),
        ("3840x2160", 3840, 2160),
    ];
    for (name, width, height) in sizes.iter() {
        let data = generate_test_data(*width, *height);
        let path = temp_path("save_png", "png", *width, *height);
        group.bench_with_input(BenchmarkId::from_parameter(name), &data, |b, data| {
            let grim = Grim::new().expect("Failed to create Grim");
            b.iter(|| {
                grim.save_png(data, *width, *height, &path)
                    .expect("Failed to save PNG");
            });
        });
    }
    group.finish();
}

fn benchmark_png_save_compression_levels(c: &mut Criterion) {
    let mut group = c.benchmark_group("png_save_compression_levels");
    let width = 1920;
    let height = 1080;
    let data = generate_test_data(width, height);
    for level in [1, 6, 9].iter() {
        let path = temp_path(&format!("save_png_lvl{}", level), "png", width, height);
        group.bench_with_input(BenchmarkId::from_parameter(level), level, |b, &level| {
            let grim = Grim::new().expect("Failed to create Grim");
            b.iter(|| {
                grim.save_png_with_compression(&data, width, height, &path, level)
                    .expect("Failed to save PNG");
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
            let grim = Grim::new().expect("Failed to create Grim");
            b.iter(|| {
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
                let grim = Grim::new().expect("Failed to create Grim");
                b.iter(|| {
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

#[cfg(feature = "jpeg")]
fn benchmark_jpeg_save(c: &mut Criterion) {
    let mut group = c.benchmark_group("jpeg_save");
    let sizes = [
        ("640x480", 640, 480),
        ("1920x1080", 1920, 1080),
        ("3840x2160", 3840, 2160),
    ];
    for (name, width, height) in sizes.iter() {
        let data = generate_test_data(*width, *height);
        let path = temp_path("save_jpeg", "jpg", *width, *height);
        group.bench_with_input(BenchmarkId::from_parameter(name), &data, |b, data| {
            let grim = Grim::new().expect("Failed to create Grim");
            b.iter(|| {
                grim.save_jpeg(data, *width, *height, &path)
                    .expect("Failed to save JPEG");
            });
        });
    }
    group.finish();
}

#[cfg(feature = "jpeg")]
fn benchmark_jpeg_save_quality_levels(c: &mut Criterion) {
    let mut group = c.benchmark_group("jpeg_save_quality_levels");
    let width = 1920;
    let height = 1080;
    let data = generate_test_data(width, height);
    for quality in [60, 80, 95].iter() {
        let path = temp_path(&format!("save_jpeg_q{}", quality), "jpg", width, height);
        group.bench_with_input(
            BenchmarkId::from_parameter(quality),
            quality,
            |b, &quality| {
                let grim = Grim::new().expect("Failed to create Grim");
                b.iter(|| {
                    grim.save_jpeg_with_quality(&data, width, height, &path, quality)
                        .expect("Failed to save JPEG");
                });
            },
        );
    }
    group.finish();
}

fn benchmark_png_stdout(c: &mut Criterion) {
    let mut group = c.benchmark_group("png_stdout");
    let width = 1920;
    let height = 1080;
    let data = generate_test_data(width, height);
    group.bench_function("write_png_to_stdout", |b| {
        let grim = Grim::new().expect("Failed to create Grim");
        b.iter(|| {
            let _silencer = StdoutSilencer::new().expect("Failed to silence stdout");
            grim.write_png_to_stdout(&data, width, height)
                .expect("Failed to write PNG");
        });
    });
    group.finish();
}

fn benchmark_png_stdout_compression(c: &mut Criterion) {
    let mut group = c.benchmark_group("png_stdout_compression");
    let width = 1920;
    let height = 1080;
    let data = generate_test_data(width, height);
    for level in [1, 6, 9].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(level), level, |b, &level| {
            let grim = Grim::new().expect("Failed to create Grim");
            b.iter(|| {
                let _silencer = StdoutSilencer::new().expect("Failed to silence stdout");
                grim.write_png_to_stdout_with_compression(&data, width, height, level)
                    .expect("Failed to write PNG");
            });
        });
    }
    group.finish();
}

#[cfg(feature = "jpeg")]
fn benchmark_jpeg_stdout(c: &mut Criterion) {
    let mut group = c.benchmark_group("jpeg_stdout");
    let width = 1920;
    let height = 1080;
    let data = generate_test_data(width, height);
    group.bench_function("write_jpeg_to_stdout", |b| {
        let grim = Grim::new().expect("Failed to create Grim");
        b.iter(|| {
            let _silencer = StdoutSilencer::new().expect("Failed to silence stdout");
            grim.write_jpeg_to_stdout(&data, width, height)
                .expect("Failed to write JPEG");
        });
    });
    group.finish();
}

#[cfg(feature = "jpeg")]
fn benchmark_jpeg_stdout_quality(c: &mut Criterion) {
    let mut group = c.benchmark_group("jpeg_stdout_quality");
    let width = 1920;
    let height = 1080;
    let data = generate_test_data(width, height);
    for quality in [60, 80, 95].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(quality),
            quality,
            |b, &quality| {
                let grim = Grim::new().expect("Failed to create Grim");
                b.iter(|| {
                    let _silencer = StdoutSilencer::new().expect("Failed to silence stdout");
                    grim.write_jpeg_to_stdout_with_quality(&data, width, height, quality)
                        .expect("Failed to write JPEG");
                });
            },
        );
    }
    group.finish();
}

#[cfg(feature = "jpeg")]
criterion_group!(
    benches,
    benchmark_png_encoding,
    benchmark_png_compression_levels,
    benchmark_png_save,
    benchmark_png_save_compression_levels,
    benchmark_jpeg_encoding,
    benchmark_jpeg_quality_levels,
    benchmark_jpeg_save,
    benchmark_jpeg_save_quality_levels,
    benchmark_png_stdout,
    benchmark_png_stdout_compression,
    benchmark_jpeg_stdout,
    benchmark_jpeg_stdout_quality
);

#[cfg(not(feature = "jpeg"))]
criterion_group!(
    benches,
    benchmark_png_encoding,
    benchmark_png_compression_levels,
    benchmark_png_save,
    benchmark_png_save_compression_levels,
    benchmark_png_stdout,
    benchmark_png_stdout_compression
);

criterion_main!(benches);

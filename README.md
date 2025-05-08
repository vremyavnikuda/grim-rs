# grim-rs

**grim-rs** — это библиотека и CLI-утилита на Rust для создания скриншотов экрана в среде Wayland с использованием протокола `zwlr_screencopy_manager_v1`.  
Она может использоваться как самостоятельная программа или как зависимость в других Rust-проектах.

---

## Возможности

- Скриншот всего экрана в Wayland-сессии (Hyprland, Sway, Wayfire и др.)
- Безопасная работа с памятью (Rust)
- Поддержка форматов PNG, JPEG, BMP
- Легко интегрируется в другие проекты (через функцию `capture_screenshot`)
- Методы для сохранения изображения: `.save_as_png()`, `.save_as_jpeg()`, `.save_as_bmp()`
- CLI-обёртка для быстрого использования из терминала

---

## Установка

### Через Cargo

```sh
cargo add grim-rs
```

### Сборка из исходников

```sh
git clone https://github.com/vremyavnikuda/grim-rs.git
cd grim-rs
cargo build --release
```

---

## Использование как CLI

```sh
./target/release/grim-rs
```

- Скриншот будет сохранён в файл вида `grim-rs_YYYYMMDD_HHMMSS.png` в текущей директории.
- Для подробного логирования используйте:
  ```sh
  RUST_LOG=info ./target/release/grim-rs
  ```

---

## Использование как библиотеки

Добавьте в `Cargo.toml`:

```toml
[dependencies]
grim-rs = "0.1"
```

Пример кода:

```rust
use grim_rs::{capture_screenshot, ScreenshotOptions, ScreenshotFormat, save_screenshot_with_format, ScreenshotSaveExt};

fn main() {
    let img = capture_screenshot(ScreenshotOptions::default())
        .expect("Failed to capture screenshot");
    // Сохранить в PNG через функцию
    save_screenshot_with_format(&img, "screenshot.png", ScreenshotFormat::Png).unwrap();
    // Сохранить в JPEG через функцию
    save_screenshot_with_format(&img, "screenshot.jpg", ScreenshotFormat::Jpeg).unwrap();
    // Сохранить в BMP через функцию
    save_screenshot_with_format(&img, "screenshot.bmp", ScreenshotFormat::Bmp).unwrap();
    // ---
    // Или использовать методы напрямую:
    img.save_as_png("screenshot2.png").unwrap();
    img.save_as_jpeg("screenshot2.jpg").unwrap();
    img.save_as_bmp("screenshot2.bmp").unwrap();
}
```

---

## API

### `capture_screenshot(options: ScreenshotOptions) -> Result<RgbaImage, ScreenshotError>`

- **ScreenshotOptions** — структура для расширенных опций:
  - `output_name: Option<String>` — имя/идентификатор экрана (или None для первого)
  - `region: Option<(u32, u32, u32, u32)>` — область (x, y, w, h) или None для всего экрана
  - `format: ScreenshotFormat` — формат (Png, Jpeg, Bmp)
- **ScreenshotFormat** — поддерживаемые форматы: PNG, JPEG, BMP
- **ScreenshotError** — подробное перечисление ошибок (Wayland, IO, внутренние ошибки)
- **RgbaImage** — изображение из crate [`image`](https://docs.rs/image/)

### `save_screenshot_with_format(img: &RgbaImage, path: &str, format: ScreenshotFormat)`

- Сохраняет изображение в нужном формате (PNG, JPEG, BMP)

### Методы расширения для RgbaImage

- `.save_as_png(path)` — сохранить как PNG
- `.save_as_jpeg(path)` — сохранить как JPEG
- `.save_as_bmp(path)` — сохранить как BMP

---

## Требования

- Wayland compositor с поддержкой протокола `zwlr_screencopy_manager_v1` (например, Sway, Hyprland, Wayfire)
- Rust 1.70+ (edition 2021)
- Пакеты: `wayland-client`, `wayland-protocols-wlr`, `image`, `libc`, `tempfile`, `log`, `env_logger`, `chrono`

---

## Пример расширения

Вы можете добавить новые поля в `ScreenshotOptions`, чтобы реализовать:
- Выбор определённого экрана/монитора
- Снимок области экрана
- Выбор формата изображения

---

## Лицензия

MIT

---

## Авторы

- [Andrew Nevsky](https://github.com/vremayvnikuda) 
# grim-rs

**grim-rs** — это библиотека и CLI-утилита на Rust для создания скриншотов экрана в среде Wayland с использованием протокола `zwlr_screencopy_manager_v1`.  
Она может использоваться как самостоятельная программа или как зависимость в других Rust-проектах.

---

## Возможности

- Скриншот всего экрана в Wayland-сессии (Hyprland, Sway, Wayfire и др.)
- Интерактивный выбор области экрана с помощью мыши
- Отображение размеров выбранной области в реальном времени
- Безопасная работа с памятью (Rust)
- Поддержка форматов PNG, JPEG, BMP
- Двойная буферизация для плавной отрисовки
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

- Скриншот будет сохранён в файл вида `YYYY-MM-DD_HH-MM-SS_grim-rs.png` в текущей директории.
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

### Пример кода для создания скриншота всего экрана:

```rust
use grim_rs::{capture_screenshot, ScreenshotOptions, ScreenshotFormat};

fn main() {
    let options = ScreenshotOptions::default();
    let image = capture_screenshot(options)
        .expect("Failed to capture screenshot");
    image.save_as_png("screenshot.png").unwrap();
}
```

### Пример кода для создания скриншота выбранной области:

```rust
use grim_rs::{select_region_interactive_sctk, capture_screenshot, ScreenshotOptions};

fn main() {
    if let Some(region) = select_region_interactive_sctk() {
        let options = ScreenshotOptions {
            region: Some((
                region.x as u32,
                region.y as u32,
                region.width as u32,
                region.height as u32
            )),
            ..Default::default()
        };
        let image = capture_screenshot(options)
            .expect("Failed to capture screenshot");
        image.save_as_png("region.png").unwrap();
    }
}
```

### Пример использования generate_filename:

```rust
use grim_rs::{capture_screenshot, ScreenshotOptions, ScreenshotFormat, ScreenshotSaveExt};

fn main() {
    let options = ScreenshotOptions::default();
    let image = capture_screenshot(options)
        .expect("Failed to capture screenshot");
    
    // Генерация имени файла с текущей датой и временем
    let filename = image.generate_filename(ScreenshotFormat::Png);
    // Результат: "2024-03-14_15-30-45_grim-rs.png"
    
    // Сохранение сгенерированным именем
    image.save_as_png(&filename).unwrap();
}
```

---

## API

### Основные структуры

#### `ScreenshotOptions`
Настройки для создания скриншота:
- `output_name: Option<String>` — имя/идентификатор экрана (None для первого)
- `region: Option<(u32, u32, u32, u32)>` — область (x, y, w, h) или None для всего экрана
- `format: ScreenshotFormat` — формат (Png, Jpeg, Bmp)

#### `Region`
Выбранная область экрана:
- `x: i32` — X-координата левого верхнего угла
- `y: i32` — Y-координата левого верхнего угла
- `width: i32` — ширина области
- `height: i32` — высота области

### Основные функции

#### `capture_screenshot(options: ScreenshotOptions) -> Result<RgbaImage, ScreenshotError>`
Создает скриншот с указанными параметрами.

#### `select_region_interactive_sctk() -> Option<Region>`
Интерактивно выбирает область экрана с помощью мыши.

#### `get_screen_dimensions() -> Result<(u32, u32), ScreenshotError>`
Получает размеры основного экрана.

### Трейты расширения

#### `ScreenshotSaveExt`
Методы для сохранения изображений:
- `.save_as_png(path)` — сохранить как PNG
- `.save_as_jpeg(path)` — сохранить как JPEG
- `.save_as_bmp(path)` — сохранить как BMP
- `.generate_filename(format)` — сгенерировать имя файла

---

## Зависимости

### Основные
- wayland-client — для работы с Wayland
- wayland-protocols-wlr — для поддержки протоколов WLR
- image — для работы с изображениями
- cairo — для отрисовки интерфейса выбора области
- smithay-client-toolkit — для работы с Wayland композитором

### Дополнительные
- tempfile — для создания временных файлов
- log — для логирования
- env_logger — для настройки логирования
- chrono — для работы с датами и временем

---

## Особенности реализации

### Двойная буферизация
Используется для плавной отрисовки рамки выбора области и предотвращения мерцания.

### Асинхронная обработка событий
Обеспечивает отзывчивость интерфейса при выборе области.

### Безопасная работа с памятью
- RAII для управления ресурсами
- Безопасные указатели
- Проверки границ массивов

---

## Рекомендации по использованию

1. Всегда обрабатывайте возможные ошибки при создании скриншотов
2. Используйте логирование для отладки (RUST_LOG=info)
3. При работе с большими областями экрана учитывайте возможные задержки
4. Для лучшей производительности используйте формат PNG
5. При выборе области используйте левую кнопку мыши для начала выбора и отпустите для завершения
6. Для отмены выбора нажмите клавишу Escape

---

## Лицензия

MIT

---

## Авторы

- [Andrew Nevsky](https://github.com/vremayvnikuda) 
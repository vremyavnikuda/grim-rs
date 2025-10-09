# TODO: Приведение проекта в соответствие с RULES.md

> Анализ выполнен: 2025-01-10  
> Статус: В ожидании исправлений  
> Приоритет: Высокий → Средний → Низкий

---

## 🔴 КРИТИЧЕСКИЕ (Высокий приоритет)

### 1. Публичные поля структур (API Breaking Change)

**Проблема:** Нарушение инкапсуляции, невозможно изменить внутреннее представление без breaking change.

#### 1.1 `src/geometry.rs` - struct Box
- [ ] Сделать поля `x`, `y`, `width`, `height` приватными
- [ ] Добавить геттеры: `x()`, `y()`, `width()`, `height()`
- [ ] Рассмотреть добавление сеттеров или builder pattern при необходимости

```rust
// Текущее состояние (строки 4-8):
pub struct Box {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

// Ожидаемое:
pub struct Box {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

impl Box {
    pub fn x(&self) -> i32 { self.x }
    pub fn y(&self) -> i32 { self.y }
    pub fn width(&self) -> i32 { self.width }
    pub fn height(&self) -> i32 { self.height }
}
```

#### 1.2 `src/lib.rs` - struct CaptureResult
- [ ] Строки 52-60: Сделать поля приватными
- [ ] Добавить геттеры: `data()`, `width()`, `height()`
- [ ] Рассмотреть `into_data()` для владения без клонирования

#### 1.3 `src/lib.rs` - struct Output
- [ ] Строки 66-74: Сделать поля приватными
- [ ] Добавить геттеры: `name()`, `geometry()`, `scale()`, `description()`

#### 1.4 `src/lib.rs` - struct CaptureParameters
- [ ] Строки 83-102: Сделать поля приватными
- [ ] Добавить builder pattern для конструирования:
  ```rust
  CaptureParameters::new(output_name)
      .region(region)
      .overlay_cursor(true)
      .scale(2.0)
  ```

#### 1.5 `src/lib.rs` - struct MultiOutputCaptureResult
- [ ] Строки 108-114: Сделать `outputs` приватным
- [ ] Добавить методы: `get(&str)`, `outputs()`, `into_outputs()`

**Миграция:**
- [ ] Создать MIGRATION.md с инструкциями
- [ ] Пометить старые публичные поля как `#[deprecated]` в версии 0.2.0
- [ ] Удалить в версии 0.3.0
- [ ] Обновить все примеры и тесты

---

### 2. Использование `.unwrap()` в production коде

**Проблема:** Паника при poisoned mutex вместо обработки ошибки.

#### 2.1 `src/wayland_capture.rs` - Mutex::lock() unwrap
- [ ] Строка 501: `frame_state.lock().unwrap()` → обработка ошибки
- [ ] Строка 529: `frame_state.lock().unwrap()` → обработка ошибки
- [ ] Строка 578: `frame_state.lock().unwrap()` → обработка ошибки
- [ ] Строка 640: `frame_state.lock().unwrap()` → обработка ошибки
- [ ] Строка 995: `frame_state.lock().unwrap()` → обработка ошибки
- [ ] Строка 1006: `frame_state.lock().unwrap()` → обработка ошибки
- [ ] Строка 1055: `frame_state.lock().unwrap()` → обработка ошибки
- [ ] Строка 1087: `frame_state.lock().unwrap()` → обработка ошибки
- [ ] Строка 1107: `frame_state.lock().unwrap()` → обработка ошибки
- [ ] Строка 1118: `frame_state.lock().unwrap()` → обработка ошибки
- [ ] Строка 1124: `frame_state.lock().unwrap()` → обработка ошибки
- [ ] Строка 1361: `frame_state.lock().unwrap()` → обработка ошибки
- [ ] Строка 1370: `frame_state.lock().unwrap()` → обработка ошибки
- [ ] Строка 1377: `frame_state.lock().unwrap()` → обработка ошибки
- [ ] Строка 1382: `frame_state.lock().unwrap()` → обработка ошибки

**Решение:**
```rust
// Вместо:
let state = frame_state.lock().unwrap();

// Использовать:
let state = frame_state
    .lock()
    .map_err(|e| Error::FrameCapture(format!("Mutex poisoned: {}", e)))?;
```

**Альтернатива:** Создать helper функцию:
```rust
fn lock_frame_state(
    frame_state: &Arc<Mutex<FrameState>>
) -> Result<std::sync::MutexGuard<FrameState>> {
    frame_state
        .lock()
        .map_err(|e| Error::FrameCapture(format!("Frame state mutex poisoned: {}", e)))
}
```

---

### 3. `.expect()` в `impl Default`

**Проблема:** Default::default() не должен паниковать по контракту трейта.

#### 3.1 `src/lib.rs` - impl Default for Grim
- [ ] Строка 1421: Удалить `impl Default` или документировать панику

**Варианты решения:**
1. **Удалить `impl Default`** (рекомендуется):
   ```rust
   // Просто убрать impl Default for Grim
   ```

2. **Документировать панику** (если Default необходим):
   ```rust
   impl Default for Grim {
       /// # Panics
       /// 
       /// Panics if Wayland connection cannot be established.
       /// Prefer using `Grim::new()` for proper error handling.
       fn default() -> Self {
           Self::new().expect("Failed to initialize Grim")
       }
   }
   ```

---

### 4. Критический баг в `capture_outputs`

**Проблема:** Используется первый output для всех захватов вместо конкретного output по имени.

#### 4.1 `src/wayland_capture.rs` - строка 932
- [ ] Исправить логику поиска output для каждого параметра
- [ ] Добавить тест на захват разных outputs

**Текущий код:**
```rust
let output = self.globals.outputs.first().ok_or_else(|| Error::NoOutputs)?;
```

**Исправление:**
```rust
// Для каждого параметра найти соответствующий output
for param in &parameters {
    let output = self.globals.outputs
        .iter()
        .find(|o| {
            let id = o.id().protocol_id();
            self.globals.output_info
                .get(&id)
                .map(|info| info.name == param.output_name)
                .unwrap_or(false)
        })
        .ok_or_else(|| Error::OutputNotFound(param.output_name.clone()))?;
    
    // Использовать найденный output для захвата
    // ...
}
```

---

## 🟡 ВАЖНЫЕ (Средний приоритет)

### 5. Dead code (неиспользуемые функции)

#### 5.1 `src/wayland_capture.rs` - неиспользуемые функции
- [ ] Строка 60: `get_output_rotation()` - удалить или использовать
- [ ] Строка 74: `get_output_flipped()` - удалить или использовать

**Варианты:**
1. Удалить, если не планируется использование
2. Добавить `#[allow(dead_code)]` с комментарием:
   ```rust
   /// Reserved for future output rotation handling
   #[allow(dead_code)]
   fn get_output_rotation(...) -> f64 { ... }
   ```

---

### 6. Неиспользуемые переменные

#### 6.1 `src/wayland_capture.rs`
- [ ] Строка 756: `_scaled_region` - использовать или удалить
- [ ] Строка 758: `_grid_aligned` - использовать или удалить

**Варианты:**
1. Если переменные нужны для будущей логики - добавить TODO комментарий
2. Если не нужны - удалить

---

### 7. Исправить Clippy warnings

#### 7.1 Лишние скобки
- [ ] `src/wayland_capture.rs:1123` - убрать лишние скобки вокруг let

#### 7.2 Идентичные блоки if
- [ ] `src/wayland_capture.rs:885` - объединить одинаковые ветки:
```rust
// Вместо:
let filter = if scale > 1.0 {
    imageops::FilterType::Triangle
} else if scale >= 0.75 {
    imageops::FilterType::Triangle
} else if scale >= 0.5 {
    imageops::FilterType::CatmullRom
} else {
    imageops::FilterType::Lanczos3
};

// Использовать:
let filter = if scale >= 0.75 {
    imageops::FilterType::Triangle
} else if scale >= 0.5 {
    imageops::FilterType::CatmullRom
} else {
    imageops::FilterType::Lanczos3
};
```

#### 7.3 Ручная проверка диапазонов
- [ ] `tests/test_filename_format.rs:21` - использовать `contains`:
```rust
// Вместо:
assert!(year >= 2020 && year <= 2100, "Year {} is out of reasonable range", year);

// Использовать:
assert!((2020..=2100).contains(&year), "Year {} is out of reasonable range", year);
```

- [ ] `tests/test_filename_format.rs:24` - то же для month
- [ ] `tests/test_filename_format.rs:27` - то же для day

#### 7.4 Match вместо if let
- [ ] `tests/test.rs:149, 164, 203` - заменить `match` на `if let`

#### 7.5 Iterator::flatten() потенциальная бесконечность
- [ ] `src/bin/grim.rs:367` - обработать ошибки вместо flatten:
```rust
// Вместо:
for line in reader.lines().flatten() {

// Использовать:
for line in reader.lines() {
    let line = line?;
    // ...
}
```

#### 7.6 Бесполезное использование vec!
- [ ] `tests/test.rs:118, 379` - использовать массив или slice

---

### 8. Рефакторинг повторяющегося кода

#### 8.1 `src/bin/grim.rs` - дублирование кода сохранения
- [ ] Строки 138-236: Вынести логику сохранения в функцию

**Предложение:**
```rust
fn save_or_output_result(
    grim: &Grim,
    result: &CaptureResult,
    output_file: &str,
    opts: &Options,
) -> grim_rs::Result<()> {
    if output_file == "-" {
        write_to_stdout(grim, result, opts)
    } else {
        save_to_file(grim, result, output_file, opts)
    }
}

fn write_to_stdout(grim: &Grim, result: &CaptureResult, opts: &Options) -> grim_rs::Result<()> {
    match opts.filetype {
        FileType::Png => {
            if opts.png_level == 6 {
                grim.write_png_to_stdout(&result.data, result.width, result.height)
            } else {
                grim.write_png_to_stdout_with_compression(
                    &result.data, result.width, result.height, opts.png_level
                )
            }
        }
        // ...
    }
}
```

---

### 9. Улучшение CLI парсинга

#### 9.1 `src/bin/grim.rs` - переход на clap
- [ ] Добавить зависимость `clap = { version = "4", features = ["derive"] }`
- [ ] Создать структуру с `#[derive(Parser)]`
- [ ] Заменить ручной парсинг на clap

**Преимущества:**
- Автоматическая генерация help
- Валидация аргументов
- Лучшие сообщения об ошибках
- Соответствие стандартам CLI приложений Rust

**Пример:**
```rust
use clap::Parser;

#[derive(Parser)]
#[command(name = "grim-rs")]
#[command(about = "Screenshot utility for Wayland", long_about = None)]
struct Cli {
    /// Set the output image's scale factor
    #[arg(short, long)]
    scale: Option<f64>,
    
    /// Set the region to capture
    #[arg(short, long)]
    geometry: Option<String>,
    
    /// Set the output filetype
    #[arg(short = 't', long, value_enum)]
    filetype: Option<FileType>,
    
    /// Output file (use '-' for stdout)
    output_file: Option<String>,
}
```

---

## 🟢 НИЗКИЙ ПРИОРИТЕТ (Оптимизации)

### 10. Оптимизация использования `.clone()`

#### 10.1 Аудит клонирований
- [ ] `src/wayland_capture.rs` - проанализировать все `.clone()` и `.to_vec()`
- [ ] Рассмотреть использование `Cow`, `Arc`, или ссылок где возможно

**Места для проверки:**
- Строка 756: `output_handle.clone()` - проверить необходимость
- Множественные `info.name.clone()` - можно ли использовать ссылки
- `data.to_vec()` при создании изображений - можно ли избежать копирования

#### 10.2 Профилирование
- [ ] Запустить `cargo flamegraph` на типичных сценариях
- [ ] Идентифицировать горячие точки
- [ ] Оптимизировать критичные участки

---

### 11. Документация и тесты

#### 11.1 Документация
- [ ] Добавить примеры использования новых геттеров
- [ ] Обновить README.md с migration guide
- [ ] Создать MIGRATION.md для версии 0.2.0

#### 11.2 Тесты
- [ ] Добавить тест на `capture_outputs` с разными output
- [ ] Добавить тест на poisoned mutex handling
- [ ] Добавить property-based тесты для геометрии (proptest)

---

### 12. Дополнительные улучшения

#### 12.1 Логирование
- [ ] Рассмотреть добавление `tracing` вместо `log`
- [ ] Добавить structured logging для отладки

#### 12.2 Async рассмотрение
- [ ] Проанализировать возможность async для event_queue
- [ ] Рассмотреть использование tokio если нужна параллельность

---

## 📋 План выполнения

### Фаза 1: Критические исправления (v0.1.3 - patch)
1. Исправить баг в `capture_outputs` (#4)
2. Заменить `.unwrap()` на обработку ошибок (#2)
3. Удалить `impl Default` или документировать (#3)

**ETA:** 1-2 дня  
**Риск:** Низкий (не ломает API)

---

### Фаза 2: API Breaking Changes (v0.2.0 - minor)
1. Сделать поля структур приватными (#1)
2. Добавить геттеры и builder patterns
3. Пометить старые публичные поля как deprecated
4. Создать MIGRATION.md

**ETA:** 3-5 дней  
**Риск:** Высокий (breaking change)

---

### Фаза 3: Качество кода (v0.2.1 - patch)
1. Исправить все Clippy warnings (#7)
2. Удалить dead code (#5, #6)
3. Рефакторинг CLI (#8, #9)

**ETA:** 2-3 дня  
**Риск:** Низкий

---

### Фаза 4: Оптимизации (v0.3.0 - minor)
1. Оптимизация clone() (#10)
2. Профилирование и оптимизация
3. Улучшение документации и тестов (#11)

**ETA:** 5-7 дней  
**Риск:** Средний

---

## 🎯 Метрики качества

### Текущее состояние
- ❌ Clippy warnings: 13
- ❌ Публичных полей: 15+
- ❌ `.unwrap()` в production: 21
- ❌ Dead code функций: 2
- ✅ Тесты: Есть
- ✅ Документация: Хорошая

### Целевое состояние
- ✅ Clippy warnings: 0
- ✅ Публичных полей: 0
- ✅ `.unwrap()` в production: 0 (только в тестах)
- ✅ Dead code: 0
- ✅ Test coverage: >80%
- ✅ Документация: Отличная

---

## 📝 Заметки

- Все изменения должны сопровождаться обновлением CHANGELOG.md
- Breaking changes требуют major/minor version bump согласно semver
- Перед каждым коммитом запускать: `cargo fmt && cargo clippy && cargo test`
- Использовать conventional commits для истории изменений

---

**Создано:** 2025-01-10  
**Обновлено:** 2025-01-10  
**Статус:** Требуется согласование приоритетов

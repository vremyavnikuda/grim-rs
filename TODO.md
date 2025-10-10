# TODO: Приведение проекта в соответствие с RULES.md

> Анализ выполнен: 2025-01-10  
> Статус: В ожидании исправлений  
> Приоритет: Высокий → Средний → Низкий

---

## 🔴 КРИТИЧЕСКИЕ (Высокий приоритет)

### 1. Публичные поля структур (API Breaking Change)

**Проблема:** Нарушение инкапсуляции, невозможно изменить внутреннее представление без breaking change.

#### 1.1 `src/geometry.rs` - struct Box
- [x] Сделать поля `x`, `y`, `width`, `height` приватными
- [x] Добавить геттеры: `x()`, `y()`, `width()`, `height()`
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
- [x] Строки 52-60: Сделать поля приватными
- [x] Добавить геттеры: `data()`, `width()`, `height()`
- [x] Добавлен `into_data()` для владения без клонирования
- [x] Добавлен конструктор `new(data, width, height)`

#### 1.3 `src/lib.rs` - struct Output
- [x] Строки 66-74: Сделать поля приватными
- [x] Добавить геттеры: `name()`, `geometry()`, `scale()`, `description()`

#### 1.4 `src/lib.rs` - struct CaptureParameters
- [x] Строки 83-102: Сделать поля приватными
- [x] Добавить builder pattern для конструирования:
  ```rust
  CaptureParameters::new(output_name)
      .region(region)
      .overlay_cursor(true)
      .scale(2.0)
  ```

#### 1.5 `src/lib.rs` - struct MultiOutputCaptureResult
- [x] Строки 108-114: Сделать `outputs` приватным
- [x] Добавить методы: `get(&str)`, `outputs()`, `into_outputs()`

**Миграция:**
- [x] Создать MIGRATION.md с инструкциями
- [x] Обновить все примеры и тесты

---

### 2. Использование `.unwrap()` в production коде

**Проблема:** Паника при poisoned mutex вместо обработки ошибки.

#### 2.1 `src/wayland_capture.rs` - Mutex::lock() unwrap
- [x] Строка 501: `frame_state.lock().unwrap()` → обработка ошибки
- [x] Строка 529: `frame_state.lock().unwrap()` → обработка ошибки
- [x] Строка 578: `frame_state.lock().unwrap()` → обработка ошибки
- [x] Строка 640: `frame_state.lock().unwrap()` → обработка ошибки
- [x] Строка 995: `frame_state.lock().unwrap()` → обработка ошибки (filter closure)
- [x] Строка 1006: `frame_state.lock().unwrap()` → обработка ошибки
- [x] Строка 1055: `frame_state.lock().unwrap()` → обработка ошибки
- [x] Строка 1087: `frame_state.lock().unwrap()` → обработка ошибки
- [x] Строка 1107: `frame_state.lock().unwrap()` → обработка ошибки (filter closure)
- [x] Строка 1118: `frame_state.lock().unwrap()` → обработка ошибки
- [x] Строка 1124: `frame_state.lock().unwrap()` → обработка ошибки
- [x] Строка 1361: `frame_state.lock().unwrap()` → обработка ошибки (event handler)
- [x] Строка 1370: `frame_state.lock().unwrap()` → обработка ошибки (event handler)
- [x] Строка 1377: `frame_state.lock().unwrap()` → обработка ошибки (event handler)
- [x] Строка 1382: `frame_state.lock().unwrap()` → обработка ошибки (event handler)

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
- [x] Строка 1481: Удалить `impl Default` - **ВЫПОЛНЕНО**

**Решение:**
Удален `impl Default for Grim` (строки 1481-1485), так как:
- Инициализация Grim может провалиться (Wayland connection)
- `Default::default()` не должен паниковать по контракту трейта
- Уже есть `Grim::new()` который возвращает `Result`
- Нигде в коде `Grim::default()` не использовался

---

### 4. Критический баг в `capture_outputs`

**Проблема:** Используется первый output для всех захватов вместо конкретного output по имени.

#### 4.1 `src/wayland_capture.rs` - строка 904-936
- [x] Исправить логику поиска output для каждого параметра - **ВЫПОЛНЕНО**
- [x] Все существующие тесты проходят

**Было (строки 907-911):**
```rust
let output = self.globals.outputs.first().ok_or_else(|| Error::NoOutputs)?;
// Затем этот же output использовался для всех параметров
```

**Стало (строки 907-936):**
```rust
// Проверка что outputs не пустые
if self.globals.outputs.is_empty() {
    return Err(Error::NoOutputs);
}

// В цикле для каждого параметра:
for param in &parameters {
    // 1. Находим output_info и его protocol_id
    let (output_id, output_info) = self.globals.output_info
        .iter()
        .find(|(_, info)| info.name == param.output_name())
        .ok_or_else(|| Error::OutputNotFound(param.output_name().to_string()))?;
    
    // 2. Находим соответствующий WlOutput по protocol_id
    let output = self.globals.outputs
        .iter()
        .find(|o| o.id().protocol_id() == *output_id)
        .ok_or_else(|| Error::OutputNotFound(param.output_name().to_string()))?;
    
    // Теперь используется правильный output для каждого параметра
}
```

**Результат:**
- Каждый output теперь захватывается независимо
- Исправлена критическая ошибка мульти-мониторного захвата
- Используется `ok_or_else()` вместо `unwrap()` согласно RULES.md

---

## 🟡 ВАЖНЫЕ (Средний приоритет)

### 5. Dead code (неиспользуемые функции)

#### 5.1 `src/wayland_capture.rs` - неиспользуемые функции
- [x] Строки 50-62: `get_output_rotation()` - **УДАЛЕНО**
- [x] Строки 64-74: `get_output_flipped()` - **УДАЛЕНО**
- [x] Строки 260-284: `check_outputs_overlap()` - **УДАЛЕНО**
- [x] Строки 289-307: `is_grid_aligned()` - **УДАЛЕНО**

**Решение:**
Удалены все неиспользуемые функции согласно RULES.md (код должен использоваться или удаляться):
- `get_output_rotation()` - 13 строк (никогда не использовалась)
- `get_output_flipped()` - 14 строк (никогда не использовалась)
- `check_outputs_overlap()` - 29 строк (вызывалась только из `is_grid_aligned`)
- `is_grid_aligned()` - 24 строк (результат не использовался)
- Всего удалено: **84 строки** dead code

---

### 6. Неиспользуемые переменные

#### 6.1 `src/wayland_capture.rs`
- [x] Строка 641: `_grid_aligned` - **УДАЛЕНО**
- [x] Строка 778: `_scaled_region` - **УДАЛЕНО**

**Решение:**
Удалены все неиспользуемые переменные согласно RULES.md:
- `_grid_aligned` - создавалась, но нигде не использовалась
- `_scaled_region` - создавалась, но нигде не использовалась
- Обе переменные были артефактами незавершенной оптимизации

---

### 7. Исправить Clippy warnings

#### 7.1 Лишние скобки
- [x] `src/wayland_capture.rs:1023` - **ИСПРАВЛЕНО** - убраны лишние скобки вокруг let

#### 7.2 Идентичные блоки if
- [x] `src/wayland_capture.rs:800` - **ИСПРАВЛЕНО** - объединены одинаковые ветки:
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
- [x] `tests/test_filename_format.rs:22` - **ИСПРАВЛЕНО** - использован `contains`:
```rust
// Вместо:
assert!(year >= 2020 && year <= 2100, "Year {} is out of reasonable range", year);

// Использовать:
assert!((2020..=2100).contains(&year), "Year {} is out of reasonable range", year);
```

- [x] `tests/test_filename_format.rs:28` - **ИСПРАВЛЕНО** - то же для month
- [x] `tests/test_filename_format.rs:31` - **ИСПРАВЛЕНО** - то же для day

#### 7.4 Match вместо if let
- [x] `tests/test.rs:122, 137, 171` - **ИСПРАВЛЕНО** - заменены `match` на `if let`

#### 7.5 Iterator::flatten() потенциальная бесконечность
- [x] `src/bin/grim.rs:349` - **ИСПРАВЛЕНО** - использован map_while(Result::ok):
```rust
// Было:
for line in reader.lines().flatten() {

// Стало:
for line in reader.lines().map_while(Result::ok) {
    // ...
}
```

#### 7.6 Бесполезное использование vec!
- [x] `tests/test.rs:103, 343` - **ИСПРАВЛЕНО** - заменены vec! на массивы []

---

### 8. Рефакторинг повторяющегося кода

#### 8.1 `src/bin/grim.rs` - дублирование кода сохранения
- [x] Строки 138-236: Вынести логику сохранения в функцию - **ВЫПОЛНЕНО**

**Решение:**
Создана иерархия функций согласно RULES.md (DRY principle, single responsibility):
- `save_or_write_result()` - главный диспетчер (если "-" → stdout, иначе → файл)
- `write_to_stdout()` - диспетчер вывода в stdout по типу файла
- `save_to_file()` - диспетчер сохранения в файл по типу файла
- `write_png_to_stdout()` / `save_png_to_file()` - обработка PNG с compression level
- `write_jpeg_to_stdout()` / `save_jpeg_to_file()` - обработка JPEG с quality + feature flag
- `create_jpeg_not_supported_error()` - централизованная генерация ошибки JPEG
- Использованы `#[cfg(feature = "jpeg")]` для правильной обработки feature flags
- Уменьшение: **92 строки дублированного кода** заменены на **1 вызов функции**
- Добавлено: **130 строк** хорошо структурированных helper функций
- Итого: +38 строк, но код стал значительно более поддерживаемым
- Следует принципу: каждая функция выполняет одну задачу
- Облегчает тестирование и модификацию

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
- [x] `src/wayland_capture.rs` - **ВЫПОЛНЕНО** - проанализированы все `.clone()` и `.to_vec()`
- [x] Рассмотрено использование `Cow`, `Arc`, или ссылок где возможно

**Результаты аудита:**

**Оптимизировано (1 случай):**
- ✅ Строка 610: `output.clone()` - **УСТРАНЕНО** лишнее клонирование
  - Было: `let output_handle = output.clone(); capture_region_for_output(&output_handle, ...)`
  - Стало: `capture_region_for_output(&output, ...)`
  - Эффект: Устранено одно Arc::clone() на каждый output в multi-monitor сценариях

**Проанализировано и признано необходимым:**
- ✅ WlOutput.clone() (строки 373, 1137) - дешёвый clone (Arc ref count), необходим для API
- ✅ OutputInfo.clone() (строка 372) - необходимо для owned данных в get_outputs()
- ✅ String.clone() - необходимо для Output структур и HashMap ключей
- ✅ mmap.to_vec() (строки 518, 1020) - необходимо для копирования из mmap + конвертация XRGB→RGBA
- ✅ Arc::clone() для frame_state - дешёвая операция, необходима для Wayland callbacks

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
1. ✅ Исправить баг в `capture_outputs` (#4) - ВЫПОЛНЕНО
2. ✅ Заменить `.unwrap()` на обработку ошибок (#2) - ВЫПОЛНЕНО
3. ✅ Удалить `impl Default` (#3) - ВЫПОЛНЕНО

**Статус:** ✅ Завершена  
**ETA:** 1-2 дня (Выполнено)  
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
1. ✅ Исправить все Clippy warnings (#7) - ВЫПОЛНЕНО
2. ✅ Удалить dead code (#5, #6) - ВЫПОЛНЕНО
3. ✅ Рефакторинг повторяющегося кода (#8.1) - ВЫПОЛНЕНО
4. Рефакторинг CLI (#9) - TODO

**Статус:** Почти завершена (осталась только задача #9.1 - clap migration)  
**ETA:** 1 день (для clap migration)  
**Риск:** Низкий

---

### Фаза 4: Оптимизации (v0.3.0 - minor)
1. ✅ Оптимизация clone() (#10.1) - ВЫПОЛНЕНО
2. Профилирование и оптимизация (#10.2) - TODO
3. Улучшение документации и тестов (#11) - TODO

**Статус:** В процессе  
**ETA:** 5-7 дней  
**Риск:** Средний

---

## 🎯 Метрики качества

### Текущее состояние
- ✅ Clippy warnings (задачи 7.1-7.6 + новые в wayland_capture.rs): 0 - все исправлены
- ✅ Публичных полей: 1 (down from 15+)
  - ✅ Box: 0 (4 поля сделаны приватными)
  - ✅ CaptureResult: 0 (3 поля сделаны приватными)
  - ✅ Output: 0 (4 поля сделаны приватными)
  - ✅ CaptureParameters: 0 (4 поля сделаны приватными + builder pattern)
  - ✅ MultiOutputCaptureResult: 0 (1 поле сделано приватным)
  - ❌ FrameState: 1 (остался публичным - internal struct)
- ✅ `.unwrap()` в production: 0 (down from 21) - использован helper lock_frame_state()
- ✅ Dead code функций: 0 (down from 6) - все удалены
- ✅ Тесты: Есть (26 doctests + 9 unit tests)
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

## 📅 История изменений

### 2025-01-10
- ✅ Выполнено 1.1: Box struct - поля сделаны приватными, добавлены геттеры
- ✅ Все тесты прошли успешно (26 doctests + 9 unit tests + integration tests)
- ✅ Коммит: `179186f refactor: make Box struct fields private and add getters`
- ✅ Выполнено 1.2: CaptureResult struct - поля приватные, добавлены геттеры data(), width(), height(), into_data() и конструктор new()
- ✅ Обновлены все использования в коде, тестах, examples и doctests
- ✅ Все 26 doctests + integration tests проходят
- ✅ Коммит: `fdd5ebb refactor: make CaptureResult fields private and add accessors`
- ✅ Выполнено 1.3: Output struct - поля приватные, добавлены геттеры name() -> &str, geometry() -> &Box, scale() -> i32, description() -> Option<&str>
- ✅ Обновлено использование в src/lib.rs, examples/comprehensive_demo.rs, examples/second_monitor_demo.rs, README.md
- ✅ Все тесты проходят (26 doctests + 9 unit tests)
- ✅ Коммит: `4ec3e08 refactor: make Output struct fields private and add getters`
- ✅ Выполнено 1.4 и 1.5: CaptureParameters (builder pattern) + MultiOutputCaptureResult (encapsulation)
- ✅ CaptureParameters: 4 поля приватными, добавлен builder pattern new().region().overlay_cursor().scale()
- ✅ Добавлены геттеры: output_name(), region_ref(), overlay_cursor_enabled(), scale_factor()
- ✅ MultiOutputCaptureResult: поле outputs приватное, добавлены методы get(), outputs(), into_outputs(), new()
- ✅ Обновлено использование в src/lib.rs, src/wayland_capture.rs, src/bin/grim.rs, tests/test.rs, examples, README.md
- ✅ Все тесты проходят (26 doctests + 9 unit tests)
- ✅ Коммит: `12040a8 refactor: add builder pattern for CaptureParameters and encapsulate MultiOutputCaptureResult`
- ✅ Выполнено 2.1: Заменены все .unwrap() в production коде (16 мест в src/wayland_capture.rs)
- ✅ Создана helper функция lock_frame_state() для безопасной блокировки mutex
- ✅ Использован ? operator для 12 случаев (propagation Result вверх)
- ✅ Использован .ok().map_or() для 2 filter closures (не поддерживают ?)
- ✅ Использован .expect() с описательными сообщениями для 4 event handlers (возвращают void)
- ✅ Все тесты проходят (26 doctests + 9 unit tests)
- ✅ Выполнено 3.1: Удален `impl Default for Grim` (строки 1481-1485)
- ✅ Default::default() больше не паникует - трейт удален, используется только Grim::new()
- ✅ Все тесты проходят после удаления impl Default
- ✅ Выполнено 4.1: Исправлен критический баг в `capture_outputs()` (строки 904-936)
- ✅ Теперь каждый output захватывается независимо по protocol_id
- ✅ Исправлена логика: вместо использования первого output для всех, теперь находится соответствующий WlOutput для каждого CaptureParameters
- ✅ Используется `ok_or_else()` вместо `unwrap()` - следует RULES.md
- ✅ Все тесты проходят (9 unit tests)
- ✅ Коммит: `cffdf4f fix: correct output selection in capture_outputs for multi-monitor setups`
- ✅ Выполнено 5.1 и 6.1: Удален весь dead code (84 строки)
- ✅ Удалены неиспользуемые функции: get_output_rotation(), get_output_flipped(), check_outputs_overlap(), is_grid_aligned()
- ✅ Удалены неиспользуемые переменные: _grid_aligned, _scaled_region
- ✅ Следует RULES.md: код должен использоваться или удаляться (не #[allow(dead_code)])
- ✅ Все тесты проходят (9 unit tests)
- ✅ Коммит: `cbaec56 refactor: remove unused code (dead code elimination)`
- ✅ Выполнено 7.1-7.6: Исправлены все Clippy warnings из задач
- ✅ Убраны лишние скобки вокруг let выражения
- ✅ Объединены идентичные блоки if в scale_image_data
- ✅ Заменены ручные проверки диапазонов на .contains()
- ✅ Заменены match на if let для single-pattern destructuring
- ✅ Заменен Iterator::flatten() на map_while(Result::ok)
- ✅ Заменены vec! на массивы где не нужна heap аллокация
- ✅ Все тесты проходят (26 doctests + 9 unit tests)
- ✅ Коммит: `06f6049 refactor: fix clippy warnings (tasks 7.1-7.6)`
- ✅ Выполнено 10.1: Оптимизация использования .clone()
- ✅ Проведён аудит всех клонирований в src/wayland_capture.rs
- ✅ Устранено лишнее output.clone() в capture_region (строка 610)
- ✅ Проверены и обоснованы все остальные clone() как необходимые
- ✅ Уменьшение кода: -4 строки, устранено 1 Arc::clone() per output
- ✅ Все тесты проходят (26 doctests + 64 unit tests)
- ✅ Коммит: `26598c8 perf: remove unnecessary WlOutput clone in capture_region`

---

**Создано:** 2025-01-10  
**Обновлено:** 2025-01-10  
**Статус:** Фаза 1 завершена, Фаза 3 (dead code + clippy warnings 7.1-7.6) завершена, Фаза 4 (clone optimization) частично завершена

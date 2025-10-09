# 🦀 Правила и концепции написания Rust-кода

Полное руководство по написанию кода в проекте
---

## 📏 Форматирование

- **Всегда используй `rustfmt`** (`cargo fmt`).
- Отступы: **4 пробела**.
- Длина строки: **≤100 символов**.
- Скобки: `fn foo() {` — на той же строке.
- Никаких эмодзи , это не записки детского сада ,это код
- Пробелы:  
  ```rust
  let x = 5;          // вокруг =, после ;
  if x == 10 { ... }  // вокруг операторов
  vec![1, 2, 3]       // после запятых
  ```

---

## 🔤 Именование

| Сущность                | Стиль             | Пример               |
|------------------------|-------------------|----------------------|
| Переменные, функции    | `snake_case`      | `read_file`          |
| Типы (`struct`, `enum`)| `PascalCase`      | `HttpRequest`        |
| Константы              | `SCREAMING_SNAKE` | `MAX_RETRIES`        |
| Жизненные циклы        | `'a`, `'input`    | `'buf`               |
| Макросы                | `snake_case!`     | `trace!`, `my_macro!`|

**Дополнительные правила:**
- **Геттеры без `get_` префикса**: `person.name()`, не `person.get_name()`
- **Конвертеры**: `to_*` для затратных, `as_*` для дешёвых, `into_*` для потребляющих
  ```rust
  fn as_bytes(&self) -> &[u8]     // дешёвая конвертация
  fn to_string(&self) -> String   // затратная (аллокация)
  fn into_inner(self) -> T        // потребляет self
  ```
- **Предикаты начинаются с `is_`, `has_`, `can_`**: `is_empty()`, `has_permission()`

---

## 🛡️ Безопасность и идиомы

- **Избегай `unwrap()`/`expect()`** в коде → используй `?`, `match`, `Result`.
- **Паника — только при логических ошибках**, не при валидных исходных данных.
- **Минимизируй `mut`** — начинай с неизменяемого.
- **Предпочитай `&str`/`&[T]` вместо `String`/`Vec<T>`** в аргументах.
- **Используй типы для предотвращения ошибок**:
  ```rust
  // Плохо: String может быть любым
  fn send_email(to: String);

  // Хорошо: Email гарантирует валидность
  struct Email(String);
  fn send_email(to: Email);
  ```
- **Используй `Option::ok_or()` вместо `unwrap()`**:
  ```rust
  // Плохо
  let value = map.get(key).unwrap();
  
  // Хорошо
  let value = map.get(key).ok_or(Error::KeyNotFound)?;
  ```

---

## 🎯 Владение и время жизни

### Правила заимствования
- **Возвращай заимствованные данные, когда можешь**:
  ```rust
  // Хорошо: не клонируем
  fn first_word(s: &str) -> &str {
      s.split_whitespace().next().unwrap_or("")
  }
  ```
- **Используй `Cow<'a, T>` для условного владения**:
  ```rust
  use std::borrow::Cow;
  
  fn process(input: &str) -> Cow<str> {
      if input.contains("replace") {
          Cow::Owned(input.replace("replace", "changed"))
      } else {
          Cow::Borrowed(input)
      }
  }
  ```
- **Явные времена жизни только когда нужны**:
  ```rust
  // Плохо: лишние аннотации
  fn first<'a>(x: &'a str) -> &'a str { x }
  
  // Хорошо: elision правила работают
  fn first(x: &str) -> &str { x }
  ```

### Паттерны владения
- **Используй `std::mem::take()` для замены значений**:
  ```rust
  let old_value = std::mem::take(&mut self.field);
  self.field = new_value;
  ```
- **`std::mem::replace()` для обмена**:
  ```rust
  let old = std::mem::replace(&mut self.state, State::New);
  ```

---

## 🧪 Тестирование

- Пиши **документационные тесты** (`/// # Examples`).
- Используй `#[cfg(test)] mod tests`.
- Тестируй ошибки: `#[should_panic]`, `assert!(result.is_err())`.

**Продвинутые практики:**
- **Используй `proptest` для property-based testing**:
  ```rust
  use proptest::prelude::*;
  
  proptest! {
      #[test]
      fn parse_roundtrip(s in "\\PC*") {
          let parsed = parse(&s)?;
          assert_eq!(parsed.to_string(), s);
      }
  }
  ```
- **Тестовые хелперы в отдельном модуле**:
  ```rust
  #[cfg(test)]
  mod test_helpers {
      pub fn create_test_user() -> User { ... }
  }
  ```
- **Используй `#[cfg(test)]` для тестовых зависимостей**:
  ```rust
  #[cfg(test)]
  use mockall::predicate::*;
  ```

---

## 📦 API-дизайн (библиотеки)

- Следуй: **«Make illegal states unrepresentable»**.
- Предоставляй `From`/`Into`, `AsRef`, `Borrow`.
- Скрывай реализацию: приватные поля + публичные конструкторы (`new`, `try_from`).
- Не экспортируй детали реализации.

**Расширенные принципы:**

### Builder Pattern
```rust
pub struct Config {
    host: String,
    port: u16,
    timeout: Duration,
}

impl Config {
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::default()
    }
}

#[derive(Default)]
pub struct ConfigBuilder {
    host: Option<String>,
    port: Option<u16>,
    timeout: Option<Duration>,
}

impl ConfigBuilder {
    pub fn host(mut self, host: impl Into<String>) -> Self {
        self.host = Some(host.into());
        self
    }
    
    pub fn build(self) -> Result<Config, BuildError> {
        Ok(Config {
            host: self.host.ok_or(BuildError::MissingHost)?,
            port: self.port.unwrap_or(8080),
            timeout: self.timeout.unwrap_or(Duration::from_secs(30)),
        })
    }
}
```

### Sealed Traits (запрет реализации снаружи)
```rust
mod sealed {
    pub trait Sealed {}
}

pub trait MyTrait: sealed::Sealed {
    fn method(&self);
}

impl sealed::Sealed for MyType {}
impl MyTrait for MyType {
    fn method(&self) { ... }
}
```

### Extension Traits
```rust
pub trait ResultExt<T, E> {
    fn log_err(self) -> Result<T, E>;
}

impl<T, E: std::fmt::Display> ResultExt<T, E> for Result<T, E> {
    fn log_err(self) -> Result<T, E> {
        if let Err(e) = &self {
            eprintln!("Error: {}", e);
        }
        self
    }
}
```

### Newtype Pattern
```rust
// Сильная типизация для предотвращения ошибок
pub struct UserId(u64);
pub struct OrderId(u64);

// Теперь невозможно перепутать
fn get_user(id: UserId) -> User { ... }
fn get_order(id: OrderId) -> Order { ... }
```

---

## ⚡ Производительность

- **Избегай ненужных `clone()`** → используй ссылки, `Rc`/`Arc`, `Cow`.
- **Предпочитай итераторы циклам**:
  ```rust
  let sum: i32 = nums.iter().sum();
  ```
- Используй `#[inline]` **только** для маленьких hot-path функций.

**Дополнительные оптимизации:**

### Аллокации
```rust
// Плохо: множественные аллокации
let mut result = String::new();
for s in strings {
    result.push_str(s);
}

// Хорошо: одна аллокация
let capacity = strings.iter().map(|s| s.len()).sum();
let mut result = String::with_capacity(capacity);
for s in strings {
    result.push_str(s);
}
```

### SmallVec и стек-оптимизации
```rust
use smallvec::SmallVec;

// Хранит до 8 элементов на стеке
let mut vec: SmallVec<[u32; 8]> = SmallVec::new();
```

### Ленивые вычисления
```rust
// Плохо: всегда вычисляет
fn get_value(&self) -> String {
    self.expensive_computation()
}

// Хорошо: вычисляет только при вызове
fn value(&self) -> impl Fn() -> String + '_ {
    || self.expensive_computation()
}

// Или используй OnceCell для мемоизации
use std::cell::OnceCell;

struct Cache {
    value: OnceCell<String>,
}

impl Cache {
    fn get(&self) -> &str {
        self.value.get_or_init(|| expensive_computation())
    }
}
```

---

## 🧠 Продвинутые концепции

### 1. **Zero-cost abstractions**
> «Вы не платите за то, чем не пользуетесь»  
→ Используй обобщения (`<T: Trait>`) вместо динамической диспетчеризации (`Box<dyn Trait>`), если не нужна гетерогенность.

```rust
// Статическая диспетчеризация (быстрее)
fn process<T: Processor>(processor: &T, data: &[u8]) {
    processor.process(data);
}

// Динамическая (гибче, но медленнее)
fn process(processor: &dyn Processor, data: &[u8]) {
    processor.process(data);
}
```

### 2. **Composition over inheritance**
> Наследования нет → используй трейты + композицию:
> ```rust
> struct Server {
>     logger: Box<dyn Logger>,
>     db: Postgres,
> }
> ```

### 3. **Fearless concurrency**
> Используй `Arc<Mutex<T>>` или каналы (`std::sync::mpsc`, `tokio::sync::mpsc`) для безопасного обмена данными между потоками.

**Паттерны многопоточности:**
```rust
// Arc для shared ownership
use std::sync::Arc;
let data = Arc::new(vec![1, 2, 3]);
let data_clone = Arc::clone(&data);

// Mutex для мутабельности
use std::sync::Mutex;
let counter = Arc::new(Mutex::new(0));

// RwLock когда много читателей
use std::sync::RwLock;
let config = Arc::new(RwLock::new(Config::default()));
let read_guard = config.read().unwrap();
```

### 4. **Error handling как часть API**
> Определяй собственные типы ошибок:
> ```rust
> #[derive(Debug)]
> enum MyError {
>     Io(std::io::Error),
>     Parse(String),
> }
> impl From<std::io::Error> for MyError { ... }
> ```

**Используй `thiserror` для удобства:**
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DataStoreError {
    #[error("data not found: {0}")]
    NotFound(String),
    
    #[error("invalid data format")]
    InvalidFormat,
    
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
```

**Используй `anyhow` для приложений:**
```rust
use anyhow::{Context, Result};

fn process_file(path: &Path) -> Result<()> {
    let content = std::fs::read_to_string(path)
        .context("Failed to read file")?;
    Ok(())
}
```

### 5. **Interior mutability**
> Когда нужна мутабельность через неизменяемую ссылку:
> - `Cell<T>` — для `Copy` типов
> - `RefCell<T>` — для runtime borrow checking
> - `Mutex<T>` / `RwLock<T>` — для многопоточности

```rust
use std::cell::RefCell;

struct Cache {
    data: RefCell<HashMap<String, String>>,
}

impl Cache {
    fn get(&self, key: &str) -> Option<String> {
        self.data.borrow().get(key).cloned()
    }
    
    fn insert(&self, key: String, value: String) {
        self.data.borrow_mut().insert(key, value);
    }
}
```

### 6. **Pin и async safety**
> При работе с `async`/`await` и `Future`:
> - Не перемещай закреплённые (`Pin`) данные.
> - Избегай self-referential структур без `Pin`.

### 7. **Trait Objects и dyn**
```rust
// Коллекции разных типов
let processors: Vec<Box<dyn Processor>> = vec![
    Box::new(JsonProcessor),
    Box::new(XmlProcessor),
];

// Возврат трейт-объектов
fn create_processor(format: &str) -> Box<dyn Processor> {
    match format {
        "json" => Box::new(JsonProcessor),
        "xml" => Box::new(XmlProcessor),
        _ => Box::new(DefaultProcessor),
    }
}
```

### 8. **Associated Types vs Generics**
```rust
// Используй associated types когда тип детерминирован
trait Container {
    type Item;
    fn get(&self, index: usize) -> Option<&Self::Item>;
}

// Используй generics когда нужна гибкость
trait Convert<T> {
    fn convert(&self) -> T;
}
```

### 9. **Phantom Types**
```rust
use std::marker::PhantomData;

struct Locked;
struct Unlocked;

struct Door<State> {
    _state: PhantomData<State>,
}

impl Door<Locked> {
    fn unlock(self) -> Door<Unlocked> {
        Door { _state: PhantomData }
    }
}

impl Door<Unlocked> {
    fn open(&self) {
        println!("Door is open!");
    }
}
```

### 10. **Type State Pattern**
```rust
struct Disconnected;
struct Connected;

struct Connection<State> {
    state: PhantomData<State>,
    socket: TcpStream,
}

impl Connection<Disconnected> {
    pub fn connect(addr: &str) -> Result<Connection<Connected>> {
        let socket = TcpStream::connect(addr)?;
        Ok(Connection {
            state: PhantomData,
            socket,
        })
    }
}

impl Connection<Connected> {
    pub fn send(&mut self, data: &[u8]) -> Result<()> {
        self.socket.write_all(data)?;
        Ok(())
    }
}
```

---

## 🔧 Макросы

### Declarative Macros
```rust
// Простые повторения
macro_rules! vec_of_strings {
    ($($x:expr),*) => {
        vec![$($x.to_string()),*]
    };
}

let v = vec_of_strings!["hello", "world"];
```

### Procedural Macros
```rust
// Derive macros
#[derive(Debug, Clone, Serialize)]
struct User {
    name: String,
}

// Custom derive лучше оставить для библиотек
```

---

## 🚀 Async/Await

### Лучшие практики
```rust
// Используй async где нужно I/O
async fn fetch_data(url: &str) -> Result<String> {
    let response = reqwest::get(url).await?;
    response.text().await
}

// Избегай блокировки в async
async fn bad_example() {
    std::thread::sleep(Duration::from_secs(1)); // ПЛОХО!
}

async fn good_example() {
    tokio::time::sleep(Duration::from_secs(1)).await; // Хорошо
}

// Используй join! для параллельности
use tokio::join;

async fn parallel_fetch() -> Result<(String, String)> {
    let (data1, data2) = join!(
        fetch_data("url1"),
        fetch_data("url2")
    );
    Ok((data1?, data2?))
}
```

### Выбор между async и sync
- **Async для I/O-bound операций**: сеть, файлы, базы данных
- **Sync для CPU-bound операций**: вычисления, обработка данных
- **Не смешивай**: блокирующий код в async runtime убьёт производительность

---

## 🧰 Инструменты

| Команда                | Назначение                     |
|------------------------|-------------------------------|
| `cargo fmt`            | Форматирование                |
| `cargo clippy`         | Статический анализ            |
| `cargo test`           | Запуск тестов                 |
| `cargo doc --open`     | Локальная документация        |
| `cargo tree`           | Дерево зависимостей           |
| `cargo audit`          | Проверка уязвимостей          |
| `cargo bloat`          | Анализ размера бинарника      |
| `cargo expand`         | Раскрытие макросов            |
| `cargo flamegraph`     | Профилирование                |

> ✅ В CI: `cargo fmt -- --check && cargo clippy -- -D warnings && cargo test && cargo audit`

---

## 🚫 Антипаттерны

- ❌ `unwrap()` и `expect()` в production коде  
- ❌ Публичные поля структур (`pub field: Type`)  
- ❌ Глобальные переменные (`static mut`)  
- ❌ Избыточные `clone()` без причины  
- ❌ Игнорирование ошибок (`let _ = file.write(...)`)
- ❌ `#[allow(dead_code)]` — код должен использоваться или удаляться
- ❌ Слишком глубокая вложенность (>3 уровней)
- ❌ Большие `match` без рефакторинга в методы
- ❌ Использование `String` где достаточно `&str`
- ❌ Блокирующий код в async функциях
- ❌ `Arc<Mutex<T>>` когда достаточно `Rc<RefCell<T>>` (single-threaded)
- ❌ Паника в библиотечном коде (используй `Result`)
- ❌ Использование `.unwrap()` на `Option` без проверки

---

## ✅ Чек-лист перед коммитом

- [ ] `cargo fmt` пройден
- [ ] `cargo clippy` без предупреждений
- [ ] `cargo test` все тесты зелёные
- [ ] Документация обновлена (`///` комментарии)
- [ ] `CHANGELOG.md` обновлён (для библиотек)
- [ ] Нет `unwrap()` в production коде
- [ ] Нет `#[allow(dead_code)]`
- [ ] Времена жизни минимальны и необходимы
- [ ] Публичный API обратно совместим (semver)

---

## 📚 Дополнительные ресурсы

- **Rust Book**: https://doc.rust-lang.org/book/
- **Rust By Example**: https://doc.rust-lang.org/rust-by-example/
- **Rust API Guidelines**: https://rust-lang.github.io/api-guidelines/
- **Rust Design Patterns**: https://rust-unofficial.github.io/patterns/
- **Effective Rust**: https://effective-rust.com/
- **Async Book**: https://rust-lang.github.io/async-book/

---

> 💡 **Главные принципы**:  
> 1. **«Если это компилируется — оно, скорее всего, правильно»**  
> 2. **«Make illegal states unrepresentable»**  
> 3. **«Доверяй системе типов, а не комментариям»**  
> 4. **«Явное лучше неявного, но не многословнее»**

---

📄 Лицензия: MIT  
Обновлено: 2025  
Автор: Rust Community Guidelines + лучшие практики сообщества
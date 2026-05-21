# broken-app — отчёт по исправлениям

Проверки до фиксов можно посмотреть в отдельной ветке `before_fix`:  
[https://github.com/ivanYudushkin/broken-app/tree/before-fix](https://github.com/ivanYudushkin/broken-app/tree/before-fix)

---

## 1. `sum_even` — UB и отладка

В первую очередь поправлю UB в `sum_even`.  
Воспользуюсь для этого VSCode CodeLLDB.

Точку останова поставлю на:

```rust
let v = *values.get_unchecked(idx);
```

В дебаг-консоли после нескольких итераций цикла:

```text
p idx
(unsigned long long) 3
p values.length
(unsigned long long) 4
```

Проблема в том, что пытаемся получить доступ по индексу:

```rust
for idx in 0..=values.len()
```

`values.len()` — индексы: `[0-3]`.

Когда `idx` принимает значение `4`, то происходит выход за границу индекса.

**Исправление цикла:**

```rust
for idx in 0..=values.len() - 1 {
    ...
}
```

```bash
cargo test --test integration sums_even_numbers -- --exact
```

```text
running 1 test
test sums_even_numbers ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out; finished in 0.00s
```

---

## 2. `use_after_free`

Теперь разберёмся с сырым указателем, на который ругается `cargo check`.  
Добавим тест:

```rust
#[test]
fn test_use_after_free() {
    assert_eq!(unsafe { use_after_free() }, 84);
}
```

Заменил `into_raw` на `into_pin`, который закрепляет значение и автоматически освобождает память после выхода из scope.  
Проверим ещё раз при помощи Miri.

На `use_after_free` больше не ругается.

---

## 3. `leak_buffer` — Miri и Valgrind

### Miri: утечка

```text
   Compiling broken-app v0.1.0 (/mnt/c/Users/ivany/Desktop/Rust_YP/broken-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.69s
     Running `.../cargo-miri runner .../debug/demo`
sum_even: 6
non-zero bytes: 3
normalize: helloworld
fib(20): 6765
dedup: [1, 2, 3, 4]
error: memory leaked: alloc1089 (Rust heap, size: 4, align: 1), allocated here:
   --> .../alloc/src/raw_vec/mod.rs:465:41
    |
465 |             AllocInit::Uninitialized => alloc.allocate(layout),
    |                                         ^^^^^^^^^^^^^^^^^^^^^^
    |
    = note: stack backtrace:
            ...
            7: broken_app::leak_buffer
                at src/lib.rs:23:17: 23:31
            8: main
                at src/bin/demo.rs:8:36: 8:54

note: some details are omitted, run with `MIRIFLAGS=-Zmiri-backtrace=full` for a verbose backtrace

note: set `MIRIFLAGS=-Zmiri-ignore-leaks` to disable this check

error: aborting due to 1 previous error
```

Но ругается на `leak_buffer`, поправим его.

Тут проблема обычная: `into_raw` требует ручной очистки памяти через `drop(Box::from_raw(raw))`, иначе будет утечка памяти. Добавим `drop` и проверим снова:

```rust
pub fn leak_buffer(input: &[u8]) -> usize {
    let boxed = input.to_vec().into_boxed_slice();
    let len = input.len();
    let raw = Box::into_raw(boxed) as *mut u8;

    let mut count = 0;
    unsafe {
        for i in 0..len {
            if *raw.add(i) != 0_u8 {
                count += 1;
            }
        }
        drop(Box::from_raw(raw));
    }
    count
}
```

### Miri: UB при dealloc

Но тогда Miri всё равно ругается на UB, но уже на этапе очистки памяти:

```text
   Compiling broken-app v0.1.0 (/mnt/c/Users/ivany/Desktop/Rust_YP/broken-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.37s
     Running `.../cargo-miri runner .../debug/demo`
sum_even: 6
error: Undefined Behavior: incorrect layout on deallocation: alloc1089 has size 4 and alignment 1, but gave size 1 and alignment 1
    --> .../alloc/src/boxed.rs:1956:17
     |
1956 |                 self.1.deallocate(From::from(ptr.cast()), layout);
     |                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Undefined Behavior occurred here
     ...
             3: broken_app::leak_buffer
                 at src/lib.rs:36:9: 36:33
             4: main
                 at src/bin/demo.rs:8:36: 8:54

error: aborting due to 1 previous error
```

Проблема в том, что использую `drop(Box::from_raw(raw))` — пытаюсь очистить память по **тонкому** указателю (преобразование `as *mut u8`), в результате освобождается только 1 байт из 4-х.

### Исправление: толстый и тонкий указатель

Если сделать отдельно тонкий указатель для того, чтобы шагать по байтам, и толстый указатель на срез в куче, то мы сможем очистить выделенную память полностью по толстому указателю:

```rust
pub fn leak_buffer(input: &[u8]) -> usize {
    let boxed = input.to_vec().into_boxed_slice();
    let len = input.len();
    let raw_slice = Box::into_raw(boxed);
    let raw = raw_slice as *mut u8;

    let mut count = 0;
    unsafe {
        for i in 0..len {
            if *raw.add(i) != 0_u8 {
                count += 1;
            }
        }
        drop(Box::from_raw(raw_slice));
    }
    count
}
```

Запускаем проверку Miri:

```bash
cargo +nightly miri run
```

Miri больше ни на что не ругается:

```text
ivany@PC-IY:/mnt/c/Users/ivany/Desktop/Rust_YP/broken-app$ cargo +nightly miri run
   Compiling broken-app v0.1.0 (/mnt/c/Users/ivany/Desktop/Rust_YP/broken-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.37s
     Running `.../cargo-miri runner .../debug/demo`
sum_even: 6
non-zero bytes: 3
normalize: helloworld
fib(20): 6765
dedup: [1, 2, 3, 4]
```

### Valgrind

Проверил Valgrind:

```bash
valgrind --leak-check=full --show-leak-kinds=all ./target/debug/demo
```

Утечек нет:

```text
==2269== Memcheck, a memory error detector
==2269== Copyright (C) 2002-2022, and GNU GPL'd, by Julian Seward et al.
==2269== Using Valgrind-3.22.0 and LibVEX; rerun with -h for copyright info
==2269== Command: ./target/debug/demo
==2269==
sum_even: 6
non-zero bytes: 3
normalize: helloworld
fib(20): 6765
dedup: [1, 2, 3, 4]
==2269==
==2269== HEAP SUMMARY:
==2269==     in use at exit: 544 bytes in 1 blocks
==2269==   total heap usage: 16 allocs, 15 frees, 3,718 bytes allocated
==2269==
==2269== 544 bytes in 1 blocks are still reachable in loss record 1 of 1
==2269==    at 0x4846828: malloc (in /usr/libexec/valgrind/vgpreload_memcheck-amd64-linux.so)
==2269==    ...
==2269==    by 0x12465D: main (in .../target/debug/demo)
==2269==
==2269== LEAK SUMMARY:
==2269==    definitely lost: 0 bytes in 0 blocks
==2269==    indirectly lost: 0 bytes in 0 blocks
==2269==      possibly lost: 0 bytes in 0 blocks
==2269==    still reachable: 544 bytes in 1 blocks
==2269==         suppressed: 0 bytes in 0 blocks
==2269==
==2269== For lists of detected and suppressed errors, rerun with: -s
==2269== ERROR SUMMARY: 0 errors from 0 contexts (suppressed: 0 from 0)
```

---

## 4. Оптимизация: `slow_dedup` и `fast_dedup`

Далее к оптимизации производительно `slow_fib` и `slow_dedup`.

Начну с того, что избавлюсь от широкого блока `sort_unstable` (~20% для `slow_dedup`).  
Убираю ненужную сортировку и смотрю на flamegraph:

Flamegraph: slow_dedup без sort_unstable

Теперь нет тяжёлых лишних операций внутри `slow_dedup`, но `slow_dedup` всё ещё занимает 50% времени `main`.

Сделал быструю реализацию через `HashSet`. В случае с вектором для проверке значения в `out` в худшем случае сложность O(n).  
Поиск значения в `HashSet` — O(1).

```rust
pub fn fast_dedup(values: &[u64]) -> HashSet<u64> {
    let mut vals = HashSet::new();

    for v in values {
        vals.insert(*v);
    }

    let mut out: Vec<u64> = vals.into_iter().collect();
    out.sort();
    out
}
```

Проверим результат:

```bash
cargo bench --bench baseline
```

```text
slow_dedup: 5.895438ms
fast_dedup: 232.34µs
```

Значительный прирост в скорости.

Посмотрим flamegraph:

Flamegraph: fast_dedup

Основную часть теперь занимает `slow_fib`. Оптимизируем её.

---

## 5. Оптимизация: `slow_fib` и `fast_fib`

Проблема в том, что при вызове функция репкурсивная вызвает 2 `slow_fib`.  
Сделаем быструю реализацию без рекурсии и с мемоизацией:

```rust
pub fn fast_fib(n: u64) -> u64 {
    if n == 0 {
        return 0;
    }
    let mut a = 0u64;
    let mut b = 1u64;
    for _ in 2..=n {
        let next = a + b;
        a = b;
        b = next;
    }
    b
}
```

И проверим:

```bash
cargo bench --bench baseline
```

```text
slow_fib: 4.42106ms
fast_fib: 72ns
```

Разительный прирост скорости.

В результате флеймграф теперь выглядит вот так:

Flamegraph: итоговый профиль

---

## 6. Логические ошибки: `normalize` и `average_positive`

Проверим логические ошибки в `normalize` и `average_positive`.

Поправим тесты для наглядности:

```rust
#[test]
fn normalize_simple() {
    assert_eq!(normalize("\t   Hello  \n  \n     World   "), "helloworld");
}
```

Сейчас тест падает с ошибкой:

```text
running 1 test

thread 'normalize_simple' (20108) panicked at tests\integration.rs:34:5:
assertion `left == right` failed
  left: "\thello\n\nworld"
 right: "helloworld"
```

Не обрабатывается корректно табуляция и переносы.  
Сделаем не через реплейс, а через посимвольную очистку:

```rust
pub fn normalize(input: &str) -> String {
    input.chars()
        .filter(|c| !c.is_whitespace())  // пробел, \n, \t, \r, …
        .collect::<String>()
        .to_lowercase()
}
```

Запустим проверку. Теперь всё работает корректно:

```text
running 1 test
test normalize_simple ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 0.00s
```

### `average_positive`

Для `average_positive` поправим тест:

```rust
#[test]
fn averages_only_positive() {
    let nums = [-5, -10, 5, 15];
    // Ожидается (5 + 15) / 2 = 10, но текущая реализация делит на все элементы.
    assert!((broken_app::average_positive(&nums) - 10.0).abs() < f64::EPSILON);
}
```

```text
running 1 test

thread 'averages_only_positive' (20168) panicked at tests\integration.rs:41:5:
assertion failed: (broken_app::average_positive(&nums) - 10.0).abs() < f64::EPSILON
```

Поправим функцию. Переписал вот так:

```rust
pub fn average_positive(values: &[i64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let mut sum_positive = 0;
    let mut len_positive = 0;

    for v in values {
        if v.is_positive() {
            sum_positive += *v;
            len_positive += 1
        }
    }

    sum_positive as f64 / len_positive as f64
}
```

Теперь тест проходит успешно.

---

## 7. Criterion и финальные проверки

Для сравнения бэнчмарков Criterion до и после заменю код slow функций их быстрыми реализациями:

```bash
cargo bench --bench criterion
```

```text
slow_fib_broken         time:   [13.897 ns 13.908 ns 13.923 ns]
                        change: [-100.000% -100.000% -100.000%] (p = 0.00 < 0.05)
                        Performance has improved.
Found 1 outliers among 100 measurements (1.00%)
  1 (1.00%) high mild

slow_dedup_broken       time:   [204.51 µs 205.23 µs 205.86 µs]
                        change: [-98.112% -98.107% -98.102%] (p = 0.00 < 0.05)
                        Performance has improved.
```

Отчёты Criterion:

Criterion: fast_fib
Criterion: fust_dedup

### Финально: Miri и Valgrind

```bash
cargo +nightly miri run
```

```text
ivany@PC-IY:/mnt/c/Users/ivany/Desktop/Rust_YP/broken-app$ cargo +nightly miri run
   Compiling broken-app v0.1.0 (/mnt/c/Users/ivany/Desktop/Rust_YP/broken-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.54s
     Running `.../cargo-miri runner .../debug/demo`
sum_even: 6
non-zero bytes: 3
normalize: helloworld
fib(20): 6765
dedup: [1, 2, 3, 4]
```

```bash
valgrind --leak-check=full --show-leak-kinds=all ./target/debug/demo
```

```text
ivany@PC-IY:/mnt/c/Users/ivany/Desktop/Rust_YP/broken-app$ valgrind --leak-check=full --show-leak-kinds=all ./target/debug/demo
==4540== Memcheck, a memory error detector
==4540== Copyright (C) 2002-2022, and GNU GPL'd, by Julian Seward et al.
==4540== Using Valgrind-3.22.0 and LibVEX; rerun with -h for copyright info
==4540== Command: ./target/debug/demo
==4540==
sum_even: 6
non-zero bytes: 3
normalize: helloworld
fib(20): 6765
dedup: {3, 1, 2, 4}
==4540==
==4540== HEAP SUMMARY:
==4540==     in use at exit: 544 bytes in 1 blocks
==4540==   total heap usage: 17 allocs, 16 frees, 3,826 bytes allocated
==4540==
==4540== 544 bytes in 1 blocks are still reachable in loss record 1 of 1
==4540==    at 0x4846828: malloc (in /usr/libexec/valgrind/vgpreload_memcheck-amd64-linux.so)
==4540==    ...
==4540==    by 0x125A3D: main (in .../target/debug/demo)
==4540==
==4540== LEAK SUMMARY:
==4540==    definitely lost: 0 bytes in 0 blocks
==4540==    indirectly lost: 0 bytes in 0 blocks
==4540==      possibly lost: 0 bytes in 0 blocks
==4540==    still reachable: 544 bytes in 1 blocks
==4540==         suppressed: 0 bytes in 0 blocks
==4540==
==4540== For lists of detected and suppressed errors, rerun with: -s
==4540== ERROR SUMMARY: 0 errors from 0 contexts (suppressed: 0 from 0)
```

### `cargo check`

```bash
cargo check
```

```text
ivany@PC-IY:/mnt/c/Users/ivany/Desktop/Rust_YP/broken-app$ cargo check
    Checking broken-app v0.1.0 (/mnt/c/Users/ivany/Desktop/Rust_YP/broken-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.62s
```

---

## 8. Доработки: `concurrency`

### `race_increment` — тест

Добавим тест:

```rust
#[test]
fn test_race_increment() {
    assert_eq!(concurrency::race_increment(2, 2), 4);
}
```

```bash
cargo test --test integration test_race_increment -- --exact
```

```text
running 1 test
test test_race_increment ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.00s
```

Добавим в `demo` и запустим Miri (**без** `RUSTFLAGS` с санитайзером):

```bash
unset RUSTFLAGS
cargo +nightly miri run
```

Miri ругается на гонку:

```text
error: Undefined Behavior: Data race detected between (1) non-atomic write on thread `unnamed-2` and (2) non-atomic read on thread `unnamed-1` at alloc8614
  --> src/concurrency.rs:15:21
   |
15 |                     COUNTER += 1;
   |                     ^^^^^^^^^^^^ (2) just happened here
   |
help: and (1) occurred earlier here
  --> src/concurrency.rs:15:21
   |
15 |                     COUNTER += 1;
   |                     ^^^^^^^^^^^^
   = help: this indicates a bug in the program: it performed an invalid operation, and caused Undefined Behavior
   = help: see https://doc.rust-lang.org/nightly/reference/behavior-considered-undefined.html for further information
   = note: this is on thread `unnamed-1`
note: the current function got called indirectly due to this code
  --> src/concurrency.rs:12:22
   |
12 |           handles.push(thread::spawn(move || {
   |  ______________________^
13 | |             for _ in 0..iterations {
14 | |                 unsafe {
15 | |                     COUNTER += 1;
...  |
18 | |         }));
   | |__________^

note: some details are omitted, run with `MIRIFLAGS=-Zmiri-backtrace=full` for a verbose backtrace

error: aborting due to 1 previous error
```

### TSan (до исправления)

```bash
RUSTFLAGS="-Zsanitizer=thread -Cpanic=abort" \
  cargo +nightly build --bin demo \
  -Zbuild-std=std,panic_abort \
  --target x86_64-unknown-linux-gnu

./target/x86_64-unknown-linux-gnu/debug/demo
```

```text
SUMMARY: ThreadSanitizer: data race .../src/concurrency.rs:15 in broken_app::concurrency::race_increment::{closure#0}
==================
race: 4
ThreadSanitizer: reported 1 warnings
```

Тоже сообщение о data race (значение `race: 4` может совпасть, но поведение не определено).

### Исправление: счётчик через атомик

Сделаю реализацию счетчика через атомик:

```rust
static COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn race_increment(iterations: usize, threads: usize) -> u64 {
    COUNTER.store(0, Ordering::Relaxed);
    let mut handles = Vec::new();
    for _ in 0..threads {
        handles.push(thread::spawn(move || {
            for _ in 0..iterations {
                COUNTER.fetch_add(1, Ordering::Relaxed);
            }
        }));
    }
    for h in handles {
        let _ = h.join();
    }
    COUNTER.load(Ordering::Relaxed)
}
```

Снова проверим TSan:

```bash
RUSTFLAGS="-Zsanitizer=thread -Cpanic=abort" \
  cargo +nightly build --bin demo \
  -Zbuild-std=std,panic_abort \
  --target x86_64-unknown-linux-gnu

./target/x86_64-unknown-linux-gnu/debug/demo
```

```text
ivany@PC-IY:/mnt/c/Users/ivany/Desktop/Rust_YP/broken-app$ ./target/x86_64-unknown-linux-gnu/debug/demo
sum_even: 6
non-zero bytes: 3
normalize: helloworld
fib(20): 6765
dedup: [1, 2, 3, 4]
race: 4
```

Гонки нет.

### `read_after_sleep` и `reset_counter`

Так же убрал unsafe блоки — работа с тем же `COUNTER`:

Добавим в demo

```rust
pub fn read_after_sleep() -> u64 {
    thread::sleep(Duration::from_millis(10));
    COUNTER.load(Ordering::Relaxed)
}

pub fn reset_counter() {
    COUNTER.store(0, Ordering::Relaxed);
}
```

Добавим в demo

### Финальная проверка

#### Miri

```bash
unset RUSTFLAGS
cargo +nightly miri run
```

```text
ivany@PC-IY:/mnt/c/Users/ivany/Desktop/Rust_YP/broken-app$ cargo +nightly miri run
   Compiling broken-app v0.1.0 (/mnt/c/Users/ivany/Desktop/Rust_YP/broken-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.83s
     Running `.../cargo-miri runner target/miri/x86_64-unknown-linux-gnu/debug/demo`
sum_even: 6
non-zero bytes: 3
normalize: helloworld
fib(20): 6765
dedup: [1, 2, 3, 4]
race: 4
read_after_sleep: 4
```

Ошибок UB нет

#### TSan

```bash
unset RUSTFLAGS
cargo clean

RUSTFLAGS="-Zsanitizer=thread -Cpanic=abort" \
  cargo +nightly build --bin demo \
  -Zbuild-std=std,panic_abort \
  --target x86_64-unknown-linux-gnu

./target/x86_64-unknown-linux-gnu/debug/demo
```

```text
ivany@PC-IY:/mnt/c/Users/ivany/Desktop/Rust_YP/broken-app$ ./target/x86_64-unknown-linux-gnu/debug/demo
sum_even: 6
non-zero bytes: 3
normalize: helloworld
fib(20): 6765
dedup: [1, 2, 3, 4]
race: 4
read_after_sleep: 4
```

Предупреждений о data race нет

#### ASan

```bash
unset RUSTFLAGS
cargo clean

RUSTFLAGS="-Zsanitizer=address -Cpanic=abort" \
  cargo +nightly build --bin demo \
  -Zbuild-std=std,panic_abort \
  --target x86_64-unknown-linux-gnu

./target/x86_64-unknown-linux-gnu/debug/demo
```

```text
ivany@PC-IY:/mnt/c/Users/ivany/Desktop/Rust_YP/broken-app$ ./target/x86_64-unknown-linux-gnu/debug/demo
sum_even: 6
non-zero bytes: 3
normalize: helloworld
fib(20): 6765
dedup: [1, 2, 3, 4]
race: 4
read_after_sleep: 4
```

Ошибок ASan нет
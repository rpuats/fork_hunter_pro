# Fork Hunter Pro

Высокочастотный Rust-сканер арбитражных вилок для букмекерского рынка РФ/ЦУПИС с operator/API/UI слоем, execution/auth readiness контуром и groundwork под freebet / semi-auto workflows.

---

## 1. Что это за проект

`Fork Hunter Pro` — это не просто “поисковик вилок”, а растущая рабочая платформа, которая должна уметь:

- собирать линии с большого числа букмекеров,
- сопоставлять одинаковые матчи у разных БК,
- искать вилки, value, ошибки в коэффициентах и related opportunities,
- показывать оператору всё, что требует внимания,
- подсказывать по bankroll / accounts / funding,
- постепенно перейти к безопасному semi-auto execution.

Проект живёт в **Rust mainline** (`crates/`) и использует legacy Python/Playwright только как вспомогательный reverse-engineering / fallback слой там, где это ещё нужно.

---

## 2. Текущий этап

Проект находится не в стадии прототипа, а в **mid/late-stage implementation**:

- scanner/runtime core уже сильный,
- API/UI/operator surfaces уже полезные,
- execution/auth readiness уже выведены в продукт,
- большой рабочий состав БК уже есть,
- часть тяжёлых БК ещё требует отдельных волн добивки.

Главный remaining фронт сейчас — не “написать с нуля”, а:

1. удерживать и расширять рабочий active set букмекеров,
2. дожимать partial-парсеры,
3. развивать основной софт (operator queue, execution/auth surfaces, freebet bridge, safe semi-auto path).

---

## 3. Что уже сделано

### 3.1 Scanner / runtime

Уже реализовано:

- parser bulkhead / concurrency guard,
- post-fetch validator,
- per-parser caps,
- staleness TTL,
- live parser health / coverage,
- bookmaker runtime diagnostics,
- bounded anti-stall workflow для локальной разработки.

Это значит, что проект уже умеет не только “что-то парсить”, но и честно показывать, что реально живо, а что деградирует.

### 3.2 Основной софт

Уже есть:

- `Execution / Operator` UI,
- `Accounts / Bankroll Readiness` UI,
- parser deep-dive surface,
- `execution/state`, `execution/overview`, `execution/ledger`,
- `execution/operator-queue` backend + UI block,
- auth/session/balance readiness surfaced для оператора,
- freebet summary / funding readiness / next actions,
- API и UI в рабочем состоянии.

### 3.3 Git / workflow

- проект уже пушился на GitHub,
- в репо есть anti-stall helpers,
- bounded local queue-runner / workflow docs добавлены,
- память о сложных parser-попытках уже сохраняется в `artifacts/parser_memory/`.

---

## 4. Рабочий состав букмекеров

Ниже — **актуальный активный состав**, а не старый mock/placeholder список.

### 4.1 Полный PASS по текущему runtime diagnostics

Следующие БК уже проходят текущий KPI-run целиком:

| БК | Total | Live | Prematch |
|---|---:|---:|---:|
| Pari | 6787 | 3390 | 3397 |
| Marathon | 6792 | 3396 | 3396 |
| Bettery | 7018 | 3509 | 3509 |
| Fonbet | 6990 | 3495 | 3495 |
| Leon | 4181 | 680 | 3501 |
| Bet24 | 6781 | 3390 | 3391 |
| Zenit | 4063 | 150 | 3913 |
| Betcity | 4094 | 208 | 3886 |
| Baltbet | 3791 | 258 | 3533 |

### 4.2 Почти добитые / partial but strong

| БК | Total | Live | Prematch | Статус |
|---|---:|---:|---:|---|
| Tennisi | 3064 | 152 | 2912 | не хватает только prematch |
| Olimp | ~1470 | 148-216 | ~1322-1488 | live почти/фактически проходит, prematch слабый |

### 4.3 Временно shelved

Эти БК **не являются текущим активным фокусом**, чтобы не мешать основному продукту:

- `betboom`
- `melbet`
- `winline`
- `winline_json`

По ним уже есть существенный прогресс, но они вынесены “в ящик” до следующей отдельной волны.

### 4.4 Дубли и правила состава

В проекте договорённость такая:

- `olimpbet` считать дублем `olimp`
- `_24bet` считать дублем `bet24`

То есть в operational board / active set учитываем их как один БК.

---

## 5. Что происходит с shelved БК

### BetBoom

`betboom` уже **пробит как standalone path**, хотя пока не закреплён в clean Rust mainline runtime.

Лучший зафиксированный standalone результат:

- `241 total`
- `151 live`
- `90 prematch`

То есть по live KPI уже достижим, но prematch coverage пока не доведён.

Память попыток сохранена в:

- `artifacts/parser_memory/betboom_attempts_2026-04-21.md`

### Melbet

Главный текущий blocker локализован до runtime/bootstrap/navigation path.

### Winline

Главный текущий blocker локализован до headless/DOM path и unicode/python output issues в fallback цепочке.

---

## 6. Архитектура проекта

### Ключевые crates

- `crates/parsers` — все Rust-парсеры БК, parser factory, diagnostics
- `crates/scanner` — scanner/runtime pipeline
- `crates/engine` — calculator, normalizer, verifier, value, odds errors и related logic
- `crates/api` — HTTP API / operator-facing endpoints
- `crates/shared` — общие модели и API contracts
- `crates/auto_betting` — execution / auth / readiness / staged flow
- `crates/persistence` — execution/freebet state persistence
- `crates/bankroll_manager` — bankroll/accounts readiness logic
- `desktop-ui` — основной UI

### Важные проектные направления

1. **Парсеры букмекеров**
2. **Scanner / runtime core**
3. **Operator / execution surfaces**
4. **Freebet / funding / lifecycle**
5. **Auth / readiness / semi-auto groundwork**

---

## 7. Текущий продуктовый слой

### Уже есть

- operator dashboard,
- accounts/bankroll view,
- parser health / parser deep dive,
- execution state,
- execution ledger,
- execution operator queue,
- freebet summary / blockers / next actions,
- auth/session readiness surface.

### Это уже usable

То есть проект уже можно считать **операторской системой**, а не просто набором скриптов.

---

## 8. Что осталось сделать

### 8.1 По active bookmakers

1. **Tennisi**
   - добить недостающий prematch uplift
2. **Olimp**
   - сильно поднять prematch coverage

### 8.2 По основному софту

1. дальше развивать execution/auth/readiness
2. усиливать freebet/funding bridge
3. аккуратно подводить систему к safe semi-auto mode

### 8.3 Потом вернуться к shelved БК

Отдельными волнами:

- BetBoom
- Melbet
- Winline

Но только после того, как active core и основной продукт будут в более завершённом виде.

---

## 9. Запуск и проверки

### Rust scanner / backend

```powershell
cargo run -p fork_hunter_bin
```

### Parser diagnostics

Пример полного active состава:

```powershell
cargo run -p parsers --bin runtime_parser_diagnostics -- --json-stdout pari marathon bettery fonbet leon zenit betcity baltbet tennisi bet24 olimp
```

### API tests

```powershell
cargo test -p api --lib
```

### Desktop UI

```powershell
cd desktop-ui
npm run build
```

---

## 10. Практическое примечание по среде

В этой Windows-среде были recurring проблемы:

- long-running task stalls,
- file lock / linker issues,
- mixed rustc artifacts.

Поэтому рабочий режим такой:

- bounded rolling tasks,
- явный toolchain `1.94.1`,
- отдельный `CARGO_TARGET_DIR` при тяжёлых прогонах,
- сохранение памяти о parser-попытках в `artifacts/parser_memory/`.

---

## 11. Куда смотреть дальше

### Документы / память

- `artifacts/parser_memory/betboom_attempts_2026-04-21.md`
- `AGENTS.md`
- `docs/workflows/LOCAL_QUEUE_RUNNER.md`
- `docs/workflows/KILO_FAST_PATH.md`

### Ключевые файлы

- `crates/parsers/src/parser_factory.rs`
- `crates/parsers/src/diagnostics.rs`
- `crates/api/src/handlers.rs`
- `crates/api/src/routes.rs`
- `crates/shared/src/models.rs`
- `desktop-ui/src/pages/OperatorPage.tsx`
- `desktop-ui/src/hooks/useScanner.ts`

---

## 12. Текущая стадия одной фразой

`Fork Hunter Pro` уже является сильной Rust-платформой для арбитражного сканинга с большим рабочим составом БК, живым operator/API/UI слоем и execution/auth/readiness контуром; основной remaining фронт — дожать `tennisi` и `olimp`, а потом отдельными волнами вернуться к shelved hard bookmakers.

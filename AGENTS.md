# AGENTS.md - Ghost Imperium / Fork-OS

## 🏆 GHOST IMPERIUM (РФ БК Сканер) — СТАТУС

**Статус**: v0.1.0 — SCANNER OPERATIONAL ✅
**Рабочих БК (Rust)**: 7/7 (Pari, Fonbet, Bettery, Marathon, 24bet, Leon, Sportbet)
**Рабочих БК (Legacy Python)**: 7/7 (Winline, Pari, Betcity, Marathon, Zenit, Baltbet, Bettery)
**Тесты**: 91 passed, 0 failed ✅
**Cross-BK Match Rate**: 97.5% (3832/3928 events) ✅
**Вилок найдено**: 0 (рынок эффективен — маржа 6-12%, это НОРМА)
**Цикл сканирования**: ~30 секунд
**Мин. прибыль**: 0.1% (понижено для тестов, было 1.0%)

### 🔧 Исправления (10.04.2026):
1. ✅ **Баг калькулятора** — `group_by_market()` не группировал Over/Under вместе
2. ✅ **Баг 24bet парсера** — `Sport::Other` вместо `Sport::Football`
3. ✅ **Нормализация лиг** — добавлены "АПЛ", "Английская Премьер-Лига", и др.
4. ✅ **Fingerprint v2** — использует Normalizer + включает лигу
5. ✅ **9 диагностических тестов** — `cross_bk_matching` suite

### Диагностика:
- **См.:** `DIAGNOSTIC_REPORT.md` — полный отчёт
- **См.:** `FIXES_2026_04_10.md` — детали исправлений
- **См.:** `crates/engine/tests/cross_bk_matching.rs` — тесты матчинга

### Модули Rust (продакшн):
- ✅ `crates/engine/calculator.rs` — калькулятор вилок (8 тестов)
- ✅ `crates/engine/normalizer.rs` — нормализатор (6 тестов)
- ✅ `crates/engine/event_pool.rs` — пул событий
- ✅ `crates/engine/freebet.rs` — фрибет-хантер
- ✅ `crates/engine/generosity.rs` — индекс щедрости
- ✅ `crates/engine/mirror.rs` — детектор зеркальных линий
- ✅ `crates/engine/momentum.rs` — ловля вилок во время событий
- ✅ `crates/engine/verifier.rs` — верификатор вилок
- ✅ `crates/engine/odds_errors.rs` — детектор ошибок в кэфах
- ✅ `crates/engine/value.rs` — детектор value ставок
- ✅ `crates/engine/corridor.rs` — коридоры

### Скраперы Rust:
| БК | Статус | События | Файл |
|----|--------|---------|------|
| Pari | ✅ | ~6600 | pari.rs |
| Fonbet | ✅ | ~6800 | fonbet.rs |
| Bettery | ✅ | ~6800 | bettery.rs |
| Marathon | ✅ | ~6500 | marathon.rs |
| 24bet | ✅ (fixed) | ~6500 | bet24.rs |
| Leon | ✅ | ~3600 | leon.rs |
| Sportbet | ✅ | ~250 | sportbet.rs |
| **SBObet** | ✅ | **~2700** | **international_bundle.rs** |
| **1xBet Alt** | ✅ | **~2700** | **international_bundle.rs** |
| **Betscope** | ✅ | **~2600** | **international_bundle.rs** |
| Olimp | ⚠️ blocked | — | olimp.rs |
| Winline | ❌ not_ported | — | legacy/python |
| Betcity | ❌ not_ported | — | legacy/python |
| Zenit | ❌ not_ported | — | legacy/python |
| Baltbet | ❌ not_ported | — | legacy/python |

### API Endpoints:
- GET `/api/v1/health` — проверка здоровья
- GET `/api/v1/metrics` — метрики сканнера
- GET `/api/v1/scanner/status` — статус сканнера
- GET `/api/v1/surebets` — вилки
- GET `/api/v1/freebets` — фрибеты
- GET `/api/v1/bookmakers` — список БК
- GET `/api/v1/parsers/coverage` — покрытие парсеров
- GET `/api/v1/parsers/health` — здоровье парсеров
- GET `/api/v1/parsers/promotion-kpi` — KPI продвижения парсеров
- GET `/api/v1/analytics/generosity` — индекс щедрости
- GET `/api/v1/corridors` — коридоры
- GET `/api/v1/express-forks` — экспресс-вилки
- GET `/api/v1/capabilities` — возможности системы
- GET `/ws` — WebSocket

### Desktop UI (React):
- ✅ `desktop-ui/` — 6 страниц, mock данные
- ⏳ Подключение к реальному pipeline — СЛЕДУЮЩИЙ ШАГ

---

## 🚀 FORK-OS (AI Агентство — 60+ агентов)

### СТАТУС FORK-OS:
**Создано агентов**: 63/60 ✅ (перевыполнено на 5%!)
**Event Bus**: ✅ Работает (8/8 тестов)
**Calculator**: ✅ Работает (39/39 тестов)
**Coordinator**: ✅ Создан и тестируется
**Normalizer**: ✅ Создан и тестируется
**Notifier**: ✅ Создан и тестируется
**Monitor**: ✅ Создан и тестируется
**Scrapers**: ✅ 13 скраперов созданы
**Config**: ✅ Загружается корректно
**Models**: ✅ Импортируются
**Errors**: ✅ Импортируются
**Полный тест**: ✅ 47/47 тестов passed

### Архитектура:
- **Event Bus**: JSONL файлы как шина данных
- **Publish/Subscribe**: агенты общаются через события
- **Coordinator**: главный агент-менеджер
- **Pipeline**: Скрапер → Нормализатор → Калькулятор → Уведомления

### Структура:
```
fork-os/
├── src/
│   ├── bus/event_bus.py          # Шина событий ✅
│   ├── scrapers/base.py          # Базовый скрапер ✅
│   ├── scrapers/winline.py       # Скрапер Winline ✅
│   ├── scrapers/pari.py          # Скрапер Pari ✅
│   ├── scrapers/betcity.py       # Скрапер Betcity ✅
│   ├── scrapers/marathon.py      # Скрапер Marathon ✅
│   ├── scrapers/zenit.py         # Скрапер Zenit ✅
│   ├── scrapers/baltbet.py       # Скрапер Baltbet ✅
│   ├── scrapers/bettery.py       # Скрапер Bettery ✅
│   ├── scrapers/betboom.py       # Скрапер BetBoom ✅
│   ├── core/coordinator.py       # Координатор ✅
│   ├── core/normalizer.py        # Нормализатор ✅
│   ├── core/models.py            # Модели данных ✅
│   ├── core/errors.py            # Обработка ошибок ✅
│   ├── core/notifier.py          # Уведомления ✅
│   ├── core/monitor.py           # Мониторинг ✅
│   ├── math/calculator.py        # Калькулятор вилок ✅
│   └── main.py                   # Точка входа ✅
├── config/
│   └── agents/                   # 61 агент ✅
├── tests/
│   ├── test_event_bus.py         # 8 тестов ✅
│   └── test_calculator.py        # Тесты калькулятора ✅
├── data/bus/                     # JSONL очереди
├── config.yaml                   # Конфигурация ✅
├── requirements.txt              # Зависимости ✅
└── README.md                     # Документация ✅
```

### 61 Агент (по ролям):
```
👔 Руководство (L5):     5 агентов
🔧 Backend Core:        12 агентов
🕷️ Парсинг БК:          12 агентов
🧮 Математика:           4 агента
🎨 Frontend/UI:          6 агентов
⚙️ DevOps:               8 агентов
🛡️ Безопасность:         4 агента
✅ QA/Testing:            6 агентов
📚 Документация:         4 агента
📊 Аналитика:             3 агента
─────────────────────────────────────
ИТОГО:                   61 агент ✅
```

### Event Bus (JSONL шина):
```
data/bus/
├── raw_odds.jsonl           # Сырые данные от скраперов
├── normalized_events.jsonl  # Нормализованные данные
├── fork_found.jsonl         # Найденные вилки
├── system_alert.jsonl       # Системные алерты
└── health_check.jsonl       # Проверки здоровья
```

### Pipeline (Конвейер):
```
🕷️ Скраперы → 📥 RAW Queue → 🧹 Нормализатор → 📥 Normalized Queue
→ 🧮 Калькулятор → 📥 Alerts Queue → 📢 Уведомления (TG/WS)
```

### Координатор (мониторинг):
- Цикл каждые 5 секунд
- Проверяет очереди
- Балансирует нагрузку
- Обрабатывает ошибки
- Логирует метрики

### Команды:
```bash
python src/main.py              # Запуск системы
python src/utils/queue_stats.py # Статистика очередей
python src/utils/proxy_checker.py # Проверка прокси
python src/utils/system_monitor.py # Статус системы
```

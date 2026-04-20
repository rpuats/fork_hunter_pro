# 🎯 Fork Hunter Pro - Руководство по проекту

> Сканер вилок для РФ БК. Написано на Rust. ~35k LOC, 13 крейтов, 17 парсеров БК, 91 тест.

## 🚀 Быстрый старт

```bash
# Build
cargo build --release

# Test
cargo test -- --test-threads=1

# Run scanner (30 sec cycles)
./target/release/fork-hunter-bin

# API (localhost:8080)
curl http://localhost:8080/api/v1/health
curl http://localhost:8080/api/v1/metrics
```

## 📦 Структура проекта

```
fork_hunter_pro/
├── crates/
│   ├── engine/              # Основной движок вилок
│   │   ├── calculator.rs    # Поиск вилок (8 рынков)
│   │   ├── normalizer.rs    # Нормализация имен
│   │   ├── event_pool.rs    # Кеш событий
│   │   ├── freebet.rs       # Охота на freebets
│   │   ├── generosity.rs    # Индекс щедрости
│   │   ├── mirror.rs        # Зеркальные линии
│   │   ├── momentum.rs      # Live arbitrage
│   │   ├── verifier.rs      # Верификация вилок
│   │   ├── corridor.rs      # Line corridors
│   │   ├── odds_errors.rs   # Аномалии в коэффах
│   │   └── value.rs         # Value betting
│   │
│   ├── parsers/             # 17 парсеров БК
│   │   ├── pari.rs          # ✅ Pari (6600 событий)
│   │   ├── fonbet.rs        # ✅ Fonbet (6800)
│   │   ├── bettery.rs       # ✅ Bettery (6800)
│   │   ├── marathon.rs      # ✅ Marathon (6500)
│   │   ├── bet24.rs         # ✅ 24bet (6500, FIXED)
│   │   ├── leon.rs          # ✅ Leon (3600)
│   │   ├── sportbet.rs      # ✅ Sportbet (250)
│   │   ├── winline.rs       # ⚠️  Winline (5000, сложная)
│   │   ├── zenit.rs         # ⚠️  Zenit (4000, транзиент)
│   │   ├── betcity.rs       # ⚠️  Betcity (5000, транзиент)
│   │   ├── baltbet.rs       # ⚠️  Baltbet (5000)
│   │   ├── liga_stavok.rs   # ⚠️  Liga Stavok (headless)
│   │   ├── betboom.rs       # 📊 Betboom (экспер.)
│   │   ├── melbet.rs        # 📊 Melbet (экспер.)
│   │   ├── tennisi.rs       # 📊 Tennisi (экспер.)
│   │   ├── olimpbet.rs      # 📊 Olimpbet (экспер.)
│   │   └── olimp.rs         # ❌ Olimp (заблокирован HTTP 403)
│   │
│   ├── scanner/             # Оркестрация
│   │   └── runner.rs        # Main loop
│   │
│   ├── api/                 # REST/WebSocket (40+ endpoints)
│   ├── persistence/         # SQLite ledger
│   ├── auto_betting/        # Автоставки
│   ├── bankroll_manager/    # Kelly allocation
│   ├── bonus_hunter/        # Охота на бонусы
│   ├── corridor_scanner/    # Коридоры
│   ├── express_forks/       # Мультилег вилки
│   ├── bot/                 # Telegram
│   ├── shared/              # Общие типы
│   └── fork_hunter_bin/     # Бинарник (5 target'ов)
│
└── Cargo.toml               # Workspace
```

## 📊 Статистика

| Метрика | Значение |
|---------|----------|
| Строк кода | ~35k |
| Крейтов | 13 |
| Парсеров БК | 17 (7 продакшн) |
| Тестов | 91 (100% pass) |
| API endpoints | 40+ |
| Cross-BK match rate | 97.5% (3832/3928) |
| Cycle time | ~30 сек |
| Вилок найдено | 0 (рынок эффективен) |

## 🎯 Ключевые модули

### Engine (calculator.rs)
- **Функция:** Поиск вилок в событиях
- **Рынки:** 1X2 (3-way), Total O/U, BTTS, Handicap, Correct Score, Even/Odd, Double Chance
- **Status:** ✅ FIXED 04-10 (баг с группировкой Over/Under)
- **Минимальная прибыль:** 0.1% (для тестов, 1.0% в продакшене)
- **Тесты:** 8 основных + cross-BK matching tests

### Normalizer (normalizer.rs)
- **Функция:** Нормализация имен команд/лиг для матчинга
- **Покрытие:** 13 лиг, 20+ команд, префиксы (ФК, СК, ХК)
- **Status:** ⚠️ Базовая (нужен fuzzy matching)
- **Accuracy:** 97.5% cross-BK match

### Parser Factory
- **Паттерн:** Factory pattern с circuit breaker
- **Timeout:** 5 сек на парсер
- **Retry:** 3 попытки на ошибку
- **Cache:** BloomFilter (100k capacity, 0.001 false rate)

## 🔧 Текущие проблемы

### 🔴 Критичные
| БК | Проблема | Решение |
|---|----------|----------|
| Olimp | HTTP 403 blocked | Использовать прокси/VPN |

### 🟡 Высокий приоритет
| БК | Проблема | Решение |
|---|----------|----------|
| Zenit | 0 событий (транзиент) | Диагностика API |
| Betcity | 0 событий (транзиент) | Диагностика API |
| Winline | 1000 LOC, сложный парсер | Рефакторинг/оптимизация |

### 🟢 Низкий приоритет
| БК | Проблема | Решение |
|---|----------|----------|
| Liga Stavok | Headless Chrome bottleneck | Асинхронизация Playwright |
| Sportbet | Мало событий (250) | Добавить markets |

## 📈 Roadmap

### Phase 1: Стабилизация ✅
- ✅ 7 рабочих парсеров
- ✅ Движок вилок (8 рынков)
- ✅ Нормализация (базовая)
- ✅ 40+ API endpoints
- ✅ 91 тест

### Phase 2: Расширение парсеров (CURRENT)
- [ ] Разблокировать Olimp (прокси)
- [ ] Починить Zenit/Betcity
- [ ] Оптимизировать Winline
- [ ] Добавить диагностику здоровья БК

### Phase 3: Улучшение качества
- [ ] Fuzzy matching в нормализаторе
- [ ] Детектор ошибок в коэффах (odds_errors.rs)
- [ ] Correct Score (4+ outcomes)
- [ ] Midpoint surebets (2.5-leg)

### Phase 4: Автоматизация
- [ ] Account integration (ставки)
- [ ] Telegram alerts
- [ ] Desktop UI (React, уже есть mock)
- [ ] WebSocket real-time

### Phase 5: ML & Analytics
- [ ] Fuzzy team matching (Levenshtein)
- [ ] Pred odd movements
- [ ] Smart filtering (expected ROI)
- [ ] Freebet matching optimization

## 🎮 Агенты (параллельная работа)

Запускаем 10+ агентов в параллели:

```
🕷️ Парсеры (7):
  - Pari Agent
  - Fonbet Agent
  - Bettery Agent
  - Marathon Agent
  - 24bet Agent
  - Leon Agent
  - Sportbet Agent

🔧 Движок (2):
  - Калькулятор Agent
  - Нормализатор Agent

🔍 Анализ (3):
  - Cross-BK Matcher
  - Проблемный БК Debugger (Olimp, Zenit, Betcity)
  - Optimizer Agent
```

## 🚀 Как запустить все агенты

```bash
# Terminal 1: Основной сканер
cargo run --release --bin fork-hunter-bin

# Terminal 2: API
curl http://localhost:8080/api/v1/metrics (watch mode)

# Terminal 3: Диагностика (debug_matching, check_1x2, final_check)
cargo run --release --bin debug_matching

# Terminal 4: Оптимизация парсеров
python scripts/optimize_parsers.py
```

## 📚 Важные файлы

- **AGENTS.md** — Статус всех 63 агентов Fork-OS
- **DIAGNOSTIC_REPORT.md** — Диагностика 04-10 update
- **FIXES_2026_04_10.md** — Детали исправлений
- **Cargo.toml** — Dependencies (Tokio, Axum, SQLx, Teloxide, Moka)
- **crates/engine/tests/cross_bk_matching.rs** — 9 diagnostic tests

## 🎯 Минимум для старта

```bash
# Проверить что всё работает
cargo test --release

# Запустить сканер (30 sec cycles)
cargo run --release --bin fork-hunter-bin

# Проверить API
curl http://localhost:8080/api/v1/health
curl http://localhost:8080/api/v1/metrics
curl http://localhost:8080/api/v1/surebets
```

## 💡 Дальнейшие шаги

1. **Стабилизировать 7 БК** → Достичь стабильного цикла без ошибок
2. **Разблокировать Olimp** → Использовать прокси (список есть в проекте)
3. **Улучшить Zenit/Betcity** → Диагностировать транзиентные падения
4. **Экспортировать в Python** → Быстрое прототипирование нового функционала
5. **Запустить UI** → Подключить React UI к API (есть mock)

---

**Last Updated:** April 18, 2026  
**Status:** BETA (feature-complete, improving)

# 🚀 Quick Start — Fork Hunter Pro Scanner

## Запуск сканнера

```powershell
# 1. Собрать проект
cargo build --bin fork_hunter_bin

# 2. Запустить сканнер
.\target\debug\fork_hunter_bin.exe

# 3. Открыть API в браузере
http://localhost:8080/api/v1/scanner/status
http://localhost:8080/api/v1/surebets
http://localhost:8080/api/v1/bookmakers
http://localhost:8080/api/v1/capabilities
```

## Диагностика

```powershell
# Проверить матчинг событий между БК
cargo run --bin debug_matching

# Проверить покрытие 1X2 рынков
cargo run --bin check_1x2
```

## Тесты

```powershell
# Все тесты (91 тест)
cargo test --lib --test cross_bk_matching

# Только тесты матчинга (9 тестов)
cargo test --test cross_bk_matching

# Только калькулятор (8 тестов)
cargo test -p engine calculator::tests -- --nocapture
```

## API Endpoints

| Endpoint | Описание |
|----------|----------|
| `GET /api/v1/health` | Проверка здоровья |
| `GET /api/v1/metrics` | Метрики сканнера |
| `GET /api/v1/scanner/status` | Статус + цикл |
| `GET /api/v1/surebets` | Вилки |
| `GET /api/v1/freebets` | Фрибеты |
| `GET /api/v1/bookmakers` | Список БК |
| `GET /api/v1/capabilities` | Возможности системы |
| `GET /api/v1/corridors` | Коридоры |
| `GET /api/v1/express-forks` | Экспресс-вилки |
| `GET /ws` | WebSocket |

## Ключевые файлы

| Файл | Описание |
|------|----------|
| `DIAGNOSTIC_REPORT.md` | Полный диагностический отчёт |
| `FIXES_2026_04_10.md` | Детали исправлений |
| `AGENTS.md` | Статус проекта |
| `crates/engine/tests/cross_bk_matching.rs` | Тесты матчинга |
| `crates/engine/src/calculator.rs` | Калькулятор вилок |
| `crates/engine/src/normalizer.rs` | Нормализатор |
| `crates/scanner/src/engine.rs` | Сканнер (fingerprint + pipeline) |

## Текущий статус

| Метрика | Значение |
|---------|----------|
| **Статус** | ✅ OPERATIONAL |
| **Активных БК** | 7/7 |
| **Событий/цикл** | ~5000 |
| **Cross-BK Match** | 97.5% |
| **Тестов** | 91/91 ✅ |
| **Вилок** | 0 (рынок эффективен) |
| **Мин. прибыль** | 0.1% |

## Почему 0 вилок?

**Это НЕ баг!** Российские букмекеры имеют маржу 6-12% — рынок эффективен.

**Когда появятся вилки:**
- 🌙 Ночью/ранним утром (меньше трейдеров)
- ⚡ Во время live событий (быстрые изменения кэфов)
- 📊 С большим количеством БК (сейчас 7, нужно 10+)
- 🎯 На экзотических рынках (Asian Handicap, Correct Score)

## Следующие шаги

1. **Добавить БК**: Winline, Betcity, Zenit, Baltbet (currently not_ported)
2. **Live scanning**: Приоритизировать live события
3. **Больше рынков**: Asian Handicap, Correct Score
4. **Desktop UI**: Подключить к реальному pipeline
5. **24/7 мониторинг**: Запустить на VPS

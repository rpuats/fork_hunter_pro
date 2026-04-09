# 👻 Ghost Imperium - Fork Scanner v2.0

Профессиональная система арбитражного беттинга (вилок) для российского рынка

## 📊 Статус системы

| Компонент | Статус |
|-----------|--------|
| Сканер | ✅ Работает |
| API | ✅ 14 endpoints |
| Web UI | ✅ Cyberpunk dashboard |
| Telegram Bot | ✅ Готов |
| Аналитика | ✅ Phase 6 |
| Mock Mode | ✅ Для тестирования |
| Real APIs | ⏳ В разработке |

## 🚀 Быстрый старт

Сейчас репозиторий живёт в **dual-stack** режиме:
- **Rust workspace** в `crates/` — основная линия разработки
- **Legacy Python** в корне — старые скрипты, тесты и утилиты для справки/точечной проверки

### Рекомендуемый bootstrap (Windows / PowerShell)

```powershell
# Быстрая проверка окружения + cargo check
.\bootstrap.ps1 -Quick

# Полная локальная валидация окружения
.\bootstrap.ps1
```

`bootstrap.ps1` делает следующее:
- создаёт `.env` из `.env.example`, если файла ещё нет
- ставит Python-зависимости для legacy-слоя
- гоняет `cargo check --workspace`
- в полном режиме дополнительно собирает Python test collection и запускает фокусные Rust-тесты

### Ручной запуск по слоям

```powershell
# Rust mainline
cargo run -p fork_hunter_bin

# Legacy Python entrypoint (если нужен старый поток)
py ghost_imperium.py
```

## 📁 Структура проекта

```
ghost_imperium/
├── api/              # FastAPI + WebSocket
│   ├── main.py       # Приложение
│   ├── routes.py     # 14 endpoints
│   └── websocket.py  # Real-time updates
├── scanner/          # Сканер вилок
│   ├── engine.py     # GhostScanner
│   └── parsers/      # 12 парсеров + mock
├── core/             # Бизнес-логика
│   ├── cache.py      # TTLCache, RateLimiter
│   ├── finder.py     # SurebetCalculator
│   └── normalizer.py # TeamNormalizer
├── services/         # Сервисы
│   ├── database.py   # SQLite
│   ├── analytics.py  # AnalyticsEngine
│   ├── discovery.py  # API Discovery
│   └── mock_data.py  # Mock данные
├── bot/              # Telegram bot
├── web/              # Web UI
├── automation/       # Auto-betting
└── .opencode/        # OpenCode config
    ├── rules/        # Agent rules
    └── sessions.md   # Multi-session config
```

## 🌐 Интерфейсы

| Интерфейс | URL |
|-----------|-----|
| Web UI | http://localhost:8000/web/index.html |
| API Docs | http://localhost:8000/docs |
| Health | http://localhost:8000/health |

## 🔌 API Endpoints

### Вилки
| Метод | Endpoint | Описание |
|-------|----------|----------|
| GET | `/api/v1/surebets` | Список вилок |
| GET | `/api/v1/surebets/top` | Топ вилок |
| GET | `/api/v1/events` | События |
| GET | `/api/v1/search` | Поиск |

### Аналитика (NEW)
| Метод | Endpoint | Описание |
|-------|----------|----------|
| GET | `/api/v1/analytics/summary` | Сводка |
| GET | `/api/v1/analytics/history` | История |
| GET | `/api/v1/analytics/chart` | Данные для графика |
| GET | `/api/v1/analytics/bookmakers` | Сравнение БК |
| GET | `/api/v1/analytics/export` | Экспорт данных |

### Управление
| Метод | Endpoint | Описание |
|-------|----------|----------|
| GET | `/api/v1/stats` | Статистика |
| GET | `/api/v1/bookmakers` | Список БК |
| GET | `/api/v1/bonuses` | Бонусы |
| GET | `/api/v1/calculator` | Калькулятор |
| POST | `/api/v1/scanner/start` | Запуск |
| POST | `/api/v1/scanner/stop` | Остановка |

## 📱 Telegram Bot

Команды:
- `/start` - Старт
- `/help` - Помощь
- `/scanner` - Статус сканера
- `/surebets` - Список вилок
- `/top` - Топ вилок
- `/stats` - Статистика
- `/bonuses` - Бонусы
- `/calculator` - Калькулятор
- `/settings` - Настройки
- `/subscribe` - Уведомления
- `/bet` - Быстрая ставка

## 🏦 Поддерживаемые букмекеры

| # | БК | Слаг | Статус |
|---|-----|------|--------|
| 1 | Winline | winline | ✅ Mock |
| 2 | Olimp | olimp | ✅ Mock |
| 3 | Pari | pari | ✅ Mock |
| 4 | Marathon | marathon | ✅ Mock |
| 5 | BetBoom | betboom | ✅ Mock |
| 6 | Fonbet | fonbet | ✅ Mock |
| 7 | 1xStavka | 1xstavka | ✅ Mock |
| 8 | Leon | leon | ✅ Mock |
| 9 | Betcity | betcity | ✅ Mock |
| 10 | Pin-up | pinup | ✅ Mock |
| 11 | Zenit | zenit | ✅ Mock |
| 12 | Olimpbet | olimpbet | ✅ Mock |

## ⚙️ Конфигурация

Базовый шаблон лежит в `.env.example`. Bootstrap копирует его в `.env` автоматически.

```env
# Core scanner
RUST_LOG=info
APP_ENV=development
SCANNER_INTERVAL_MS=3000
MIN_PROFIT_PERCENT=0.5

# API
API_HOST=127.0.0.1
API_PORT=8000

# Legacy Python mode / local DB
USE_MOCK_DATA=true
DATABASE_URL=ghost_imperium.db

# Telegram
TELEGRAM_BOT_TOKEN=
TELEGRAM_CHAT_ID=
```

## 🧪 Тестирование

```powershell
# Rust workspace
cargo check --workspace
cargo test -p shared -p engine -p parsers -p scanner -p persistence

# Legacy Python
py -m pip install -r requirements.txt
py -m pytest --collect-only -q
```

Если нужна организация параллельной работы агентов и worktrees, см. `DEV_SETUP.md`, `OPENCLAW_WORKFLOW.md` и `AGENT_SWARM.md`.

## 📊 Формулы

### Проверка вилки (2-way)
```
S = 1/K1 + 1/K2
Вилка: S < 1
Прибыль: (1/S - 1) × 100%
```

### Расчёт ставок
```
Ставка1 = M / K1 / (1/K1 + 1/K2)
Ставка2 = M / K2 / (1/K1 + 1/K2)
```

## 🤖 OpenCode Multi-Session

Для параллельной разработки используйте `.opencode/sessions.md`:
- Сессия 1: Парсеры БК
- Сессия 2: API + Web UI
- Сессия 3: Telegram бот
- Сессия 4: База данных
- Сессия 5: Автоматизация

Правила для агентов: `.opencode/rules/parsers.md`

## 📈 Метрики

- Цикл сканирования: ~720ms (цель <3000ms)
- Парсинг 12 БК: <1 сек
- Задержка уведомлений: <1 сек
- Вилок найдено: 100+ (mock mode)

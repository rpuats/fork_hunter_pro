# 👻 GHOST IMPERIUM - Technical Specification

## Описание проекта
**Ghost Imperium** — профессиональная система арбитражного беттинга (вилок) для российского рынка.

## 🎯 Цели
- Сканирование 12+ российских букмекеров
- Поиск арбитражных ситуаций (вилок) в реальном времени
- Полуавтоматические и автоматические ставки
- Уведомления через Telegram

---

## 📁 Архитектура проекта

```
ghost_imperium/
│
├── api/                        # FastAPI REST API
│   ├── __init__.py
│   ├── main.py                # Точка входа приложения
│   ├── routes/                # Роуты API
│   │   ├── __init__.py
│   │   ├── surebets.py        # Эндпоинты вилок
│   │   ├── events.py          # Эндпоинты событий
│   │   ├── scanner.py          # Управление сканером
│   │   ├── bonuses.py         # Бонусы
│   │   └── search.py          # Поиск
│   ├── websocket/             # WebSocket
│   │   ├── __init__.py
│   │   └── manager.py         # WebSocket connection manager
│   └── dependencies.py        # Зависимости FastAPI
│
├── scanner/                    # Ядро сканера
│   ├── __init__.py
│   ├── engine.py              # Главный движок сканирования
│   ├── scheduler.py           # Планировщик циклов
│   ├── parsers/               # Парсеры букмекеров
│   │   ├── __init__.py
│   │   ├── base.py           # Базовый класс парсера
│   │   ├── mixins/           # Миксины для парсеров
│   │   │   ├── __init__.py
│   │   │   ├── http.py       # HTTP миксин
│   │   │   ├── websocket.py  # WebSocket миксин
│   │   │   └── playwright.py # Playwright миксин
│   │   ├── winline.py        # Winline
│   │   ├── Olimp.py          # Olimp
│   │   ├── pari.py           # Pari
│   │   ├── marathon.py       # Marathonbet
│   │   ├── betboom.py        # BetBoom
│   │   ├── fonbet.py         # Fonbet
│   │   ├── leon.py           # Leon
│   │   ├── betcity.py        # Betcity
│   │   ├── pinup.py          # Pin-up
│   │   ├── zenit.py          # Zenit
│   │   └── liga Stavok.py    # Лига Ставок
│   ├── cache.py              # Кэширование событий
│   └── rate_limiter.py       # Ограничение запросов
│
├── core/                      # Бизнес-логика
│   ├── __init__.py
│   ├── calculator.py         # Калькулятор вилок
│   ├── filters.py           # Фильтры
│   ├── normalizer.py        # Нормализатор названий
│   └── aggregator.py        # Агрегатор событий
│
├── models/                    # Модели данных
│   ├── __init__.py
│   ├── event.py             # Событие
│   ├── surebet.py           # Вилка
│   ├── bookmaker.py         # Букмекер
│   ├── stake.py             # Ставка
│   ├── bonus.py             # Бонус
│   └── user.py              # Пользователь
│
├── services/                  # Сервисы
│   ├── __init__.py
│   ├── database.py          # SQLite/PostgreSQL
│   ├── cache.py             # Redis/кэш
│   ├── notifications.py     # Уведомления
│   └── analytics.py         # Аналитика
│
├── bot/                      # Telegram Bot
│   ├── __init__.py
│   ├── main.py              # Точка входа
│   ├── handlers/            # Обработчики
│   │   ├── __init__.py
│   │   ├── start.py
│   │   ├── surebets.py
│   │   ├── settings.py
│   │   ├── calculator.py
│   │   └── bonuses.py
│   ├── keyboards.py         # Клавиатуры
│   └── filters.py           # Фильтры
│
├── web/                      # Web Dashboard
│   ├── __init__.py
│   ├── app.py               # Starlette/FastAPI
│   ├── templates/            # HTML шаблоны
│   │   ├── base.html
│   │   ├── dashboard.html
│   │   ├── calculator.html
│   │   └── settings.html
│   └── static/              # CSS, JS, images
│       ├── css/
│       ├── js/
│       └── images/
│
├── config/                   # Конфигурация
│   ├── __init__.py
│   ├── settings.py          # Настройки
│   ├── bookmakers.py        # Конфиг БК
│   └── limits.py            # Лимиты
│
├── tests/                    # Тесты
│   ├── __init__.py
│   ├── test_calculator.py
│   ├── test_parsers.py
│   └── test_api.py
│
├── scripts/                  # Скрипты
│   ├── install_deps.sh
│   └── init_db.py
│
├── docs/                     # Документация
├── logs/                     # Логи
├── data/                     # Данные
│
├── main.py                  # Точка входа
├── requirements.txt
├── pyproject.toml
├── Dockerfile
├── docker-compose.yml
├── .env.example
└── README.md
```

---

## 🏦 Российские букмекеры (12)

| # | Название | Слаг | API | Особенности |
|---|----------|------|-----|-------------|
| 1 | Winline | winline | ⚠️ Reverse | WebSocket live |
| 2 | Olimp | olimp | ✅ Есть | REST API |
| 3 | Olimpbet | OlimpBet | ✅ Есть | REST API |
| 4 | Pari | pari | ⚠️ Reverse | REST API |
| 5 | Marathonbet | marathon | ❌ Парсинг | Playwright |
| 6 | BetBoom | betboom | ⚠️ Reverse | REST API |
| 7 | Fonbet | fonbet | ❌ Парсинг | Playwright |
| 8 | 1xStavka | 1xstavka | ⚠️ Reverse | REST API |
| 9 | Leon | leon | ⚠️ Reverse | REST API |
| 10 | Betcity | betcity | ❌ Парсинг | Playwright |
| 11 | Pin-up | pinup | ⚠️ Reverse | REST API |
| 12 | Zenit | zenit | ❌ Парсинг | Playwright |

---

## 🔧 Технологии

| Компонент | Технология |
|-----------|------------|
| Backend | Python 3.11+, FastAPI, asyncio |
| Database | SQLite → PostgreSQL |
| Parser HTTP | aiohttp, httpx |
| Parser Browser | Playwright, undetected-chromedriver |
| Bot | Aiogram 3.x |
| Frontend | Vanilla JS, CSS |
| Cache | In-memory, optional Redis |
| Queue | asyncio, optional Celery |

---

## 📊 Модель данных

### Event (Событие)
```python
{
    "id": str,
    "bookmaker": str,
    "sport": str,           # football, hockey, etc.
    "league": str,          # Лига/турнир
    "home_team": str,
    "away_team": str,
    "start_time": datetime,
    "is_live": bool,
    "markets": {
        "1x2": {"home": float, "draw": float, "away": float},
        "total": {...},
        "handicap": {...}
    }
}
```

### Surebet (Вилка)
```python
{
    "id": str,
    "event_id": str,
    "event_name": str,
    "sport": str,
    "is_live": bool,
    "market_type": str,     # 1x2, total, handicap
    "profit_percent": float,
    "legs": [
        {
            "bookmaker": str,
            "market": str,
            "selection": str,  # "П1", "П2", "ТБ 2.5"
            "odds": float,
            "calculated_stake": float
        }
    ],
    "total_stake": float,
    "estimated_profit": float,
    "found_at": datetime,
    "expires_at": datetime
}
```

---

## 🔌 API Endpoints

### Surebets
- `GET /api/v1/surebets` — список вилок (пагинация, фильтры)
- `GET /api/v1/surebets/{id}` — детали вилки
- `GET /api/v1/surebets/top` — топ вилки

### Events
- `GET /api/v1/events` — события
- `GET /api/v1/events/{id}` — детали события
- `GET /api/v1/events/search` — поиск

### Scanner
- `POST /api/v1/scanner/start` — запустить
- `POST /api/v1/scanner/stop` — остановить
- `GET /api/v1/scanner/status` — статус
- `POST /api/v1/scanner/cycle` — ручной цикл

### Stats
- `GET /api/v1/stats` — общая статистика
- `GET /api/v1/stats/bookmakers` — по БК
- `GET /api/v1/stats/history` — история

### Bonuses
- `GET /api/v1/bonuses` — список бонусов
- `GET /api/v1/bonuses/{id}` — детали

### WebSocket
- `WS /ws/v1/surebets` — real-time вилки
- `WS /ws/v1/stats` — real-time статистика

---

## 📱 Telegram Bot команды

| Команда | Описание |
|---------|----------|
| /start | Старт |
| /help | Помощь |
| /scanner | Статус сканера |
| /surebets | Список вилок |
| /top | Топ вилки |
| /stats | Статистика |
| /bonuses | Бонусы |
| /calculator | Калькулятор |
| /settings | Настройки |
| /subscribe | Подписка на уведомления |

---

## 🔄 Цикл работы сканера

```
1. Scheduler триггерит цикл (каждые 5 сек)
2. Параллельно опрашиваем все БК:
   ├── Winline ──→ HTTP/WebSocket
   ├── Olimp ───→ REST API
   ├── Pari ─────→ HTTP
   ├── Marathon ─→ Playwright
   └── ...
3. Полученные события нормализуем
4. Дедиублицируем (кэш)
5. Агрегатор группирует по событиям
6. Calculator ищет вилки
7. Фильтруем по критериям
8. Сохраняем в БД
9. WebSocket → клиенты
10. Telegram → подписчики (>5%)
```

---

## ⚙️ Конфигурация

### .env
```env
# App
APP_NAME=Ghost Imperium
DEBUG=false
LOG_LEVEL=INFO

# API
API_HOST=0.0.0.0
API_PORT=8000
API_SECRET_KEY=your-secret-key

# Database
DATABASE_URL=sqlite:///ghost_imperium.db
# DATABASE_URL=postgresql://user:pass@localhost/ghost

# Scanner
SCANNER_INTERVAL=5
MIN_PROFIT_PERCENT=0.5
MAX_EVENTS_PER_BK=200

# Telegram
TELEGRAM_BOT_TOKEN=
TELEGRAM_CHAT_IDS=

# Limits
RATE_LIMIT_PER_MINUTE=60
MAX_CONCURRENT_PARSERS=12
```

---

## 🚀 Запуск

```bash
# Development
pip install -r requirements.txt
python main.py

# Production
docker-compose up -d

# API Docs
http://localhost:8000/docs
```

---

## 📈 Формулы

### Проверка вилки (2-way)
```
S = 1/K1 + 1/K2
Если S < 1 → ВИЛКА!
Прибыль = (1/S - 1) × 100%
```

### Расчёт ставок
```
Сумма = K1 × K2 / (K1 + K2) × M
Ставка1 = M / K1 / (1/K1 + 1/K2)
Ставка2 = M / K2 / (1/K1 + 1/K2)
```

Где M — общая сумма ставки, K1, K2 — коэффициенты.

---

## 🔒 Безопасность

- Rate limiting на API
- Защита от парсинга (задержки, User-Agent rotation)
- Не хранить пароли БК (только в зашифрованном виде)
- Rate limit для автоматических ставок
- Логирование всех действий

---

## 📊 Метрики успеха

- Сканирование 12 БК за < 5 секунд
- False positive rate < 1%
- Uptime > 99.9%
- Задержка уведомлений < 1 сек

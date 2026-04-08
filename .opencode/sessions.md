# Ghost Imperium - Multi-Session Config
# Use this to run multiple agents in parallel

## Session 1: Парсеры букмекеров
**Задача:** Разработка и улучшение парсеров для 12 БК
**Файлы:** `scanner/parsers/*.py`
**Фокус:**
- Winline, Olimp, Pari, Marathon, BetBoom, Fonbet
- 1xStavka, Leon, Betcity, Pin-up, Zenit, Olimpbet
- Реальные API endpoints
- Rate limiting и retry logic

## Session 2: API + Веб-дашборд
**Задача:** Разработка API endpoints и веб-интерфейса
**Файлы:** `api/*.py`, `web/index.html`
**Фокус:**
- REST API endpoints
- WebSocket real-time updates
- Cyberpunk dashboard
- Аналитика и графики

## Session 3: Telegram бот + Уведомления
**Задача:** Разработка Telegram бота и системы уведомлений
**Файлы:** `bot/*.py`, `services/analytics.py`
**Фокус:**
- Команды бота
- Push-уведомления о вилках
- Интеграция с аналитикой
- Калькулятор вилок

## Session 4: База данных + Кэширование
**Задача:** Оптимизация БД и кэширования
**Файлы:** `services/database.py`, `core/cache.py`
**Фокус:**
- SQLite оптимизация
- TTL кэширование
- Rate limiting
- История вилок

## Session 5: Автоматизация ставок
**Задача:** Разработка системы авто-ставок
**Файлы:** `automation/*.py`
**Фокус:**
- Playwright browser automation
- Auto-betting logic
- 2FA handling
- Bet confirmation

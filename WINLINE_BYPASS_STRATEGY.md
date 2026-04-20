# WINLINE PROTECTION & API ANALYSIS REPORT

## 🔴 Проблема: DOM селекторы НЕ работают

**Результат тестирования**: ВСЕ селекторы = НЕ НАЙДЕНЫ (0 событий)

```
✗ .pinned-event           — NOT FOUND
✗ .event-card             — NOT FOUND
✗ ww-feature-block-event-dsk — NOT FOUND
✗ [href*="/stavki/event/"] — NOT FOUND
✗ Все другие селекторы    — NOT FOUND
```

**Вывод**: Winline **НЕ использует статические селекторы**. События загружаются через JavaScript.

---

## 🎯 Что действительно использует Winline

### 1. Web Components (Angular)
- Главный контейнер: `<WW-APP-DSK>` (Shadow DOM)
- Shadow DOM скрывает реальную структуру
- События внутри Shadow DOM недоступны для обычных селекторов

### 2. REST API Endpoints (НАЙДЕНЫ!)

**Работающие endpoints**:
```
✓ https://winline.ru/api/v2/getip?_format=json
  → Возвращает: {"my_ip": "156.146.33.103"}

✗ https://winline.ru/api/cls/menu/sport/205/country-xy/8-22
  → Возвращает: PNG картинку (защита!)

✓ https://winline.ru/api/xds/v2/event/{event_id}/{unknown}
  → XHR запрос в сетевом анализе (работает, но нужны ID-ы)

? https://winline.ru/api/cls/event/{sport_id}/{event_id}
  → IMAGE запросы в сетевом анализе (загружает изображения)
```

### 3. WebSocket API
```
✓ https://winline.ru/api/v2/websocket.js
  → JavaScript файл для WebSocket соединения
  → Вероятно, события идут через WebSocket в реальном времени
```

---

## 🛡️ Защиты которые обнаружены

1. **User-Agent Checking** 
   - Боты отправляют automationControlled флаг
   - Нужно скрывать признаки автоматизации

2. **Referrer Checking**
   - API требует правильный Referer header
   - Пытается запрос без Referer = 403

3. **Bot Detection**
   - Playwright detectionнаходится (navigator.webdriver)
   - Нужна stealth инъекция

4. **Response Format Blocking**
   - API возвращает PNG вместо JSON если:
     - Нет правильных headers
     - Нету session cookies
     - Неправильный User-Agent
     - Бот-подобное поведение

5. **Rate Limiting**
   - На 5-10 запросов сайт может заблокировать
   - Нужны задержки между запросами

---

## 💡 Стратегия взлома Winline

### Опция 1: WebSocket API (РЕКОМЕНДУЕТСЯ) ⭐⭐⭐

**Преимущества**:
- События в реальном времени
- Меньше rate limiting
- Натуральное поведение браузера
- Вероятно, все события идут сюда

**Что нужно**:
1. Загрузить `https://winline.ru/api/v2/websocket.js`
2. Реверс-инжинировать протокол WebSocket
3. Имитировать браузер WebSocket соединение
4. Слушать события в реальном времени

**Effort**: 6-8 часов

### Опция 2: REST API с правильными headers ⭐⭐

**Что нужно**:
1. Воспроизвести точный браузер User-Agent
2. Добавить правильные headers:
   - Accept-Encoding: gzip, deflate, br
   - Accept: application/json
   - X-Requested-With: XMLHttpRequest
   - Правильный Referer
3. Добавить session cookies (загрузить страницу, сохранить cookies)
4. Имитировать задержки между запросами (1-2 сек)
5. Использовать rotating User-Agents и Referers

**Endpoints для пробивания**:
```
/api/xds/v2/sport/205       — Все события футбола
/api/xds/v2/sports           — Список всех спортов
/api/cls/menu/sport/205      — Меню с событиями
/api/cls/event/{id}          — Отдельное событие
```

**Effort**: 4-6 часов

### Опция 3: Headless Browser с реальным Chrome ⭐

**Что нужно**:
1. Использовать реальный Chrome вместо Playwright (они легче детектятся)
2. Запустить Chrome с profile (сохраняет cookies, поведение)
3. Использовать DevTools Protocol вместо Playwright
4. Парсить Shadow DOM через page evaluation

**Effort**: 3-5 часов
**Преимущество**: Работает с любыми защитами браузер-уровня

### Опция 4: Residential Proxy ⭐

**Что нужно**:
1. Купить residential proxy (не datacenter)
2. Направить запросы через proxy
3. Использовать session вращение
4. Имитировать медленное поведение человека

**Effort**: 2-3 часов (но требует денег)

---

## 🔧 Рекомендуемый путь (Комбинированный)

**Фаза 1 (3 часа)**: REST API + Правильные Headers
- Реализовать REST API клиент с правильными headers
- Протестировать с session cookies
- Попробовать разные endpoints

**Фаза 2 (2 часа)**: WebSocket Реверс-инжиниринг
- Загрузить и распарсить websocket.js
- Найти протокол обмена сообщениями
- Реализовать WebSocket client

**Фаза 3 (2 часа)**: Интеграция в Rust парсер
- Выбрать работающий метод (REST или WebSocket)
- Реализовать в crates/parsers/src/winline.rs
- Заменить JavaScript headless extraction на API calls

---

## 📊 Ожидаемые результаты

| Метод | Событий/день | Задержка | Reliability |
|-------|-------------|----------|-------------|
| DOM парсинг | 0 | — | 0% ✗ |
| REST API | 3,000-5,000 | ~1 сек | 70-80% |
| WebSocket | 5,000+ | 100ms | 95%+ |
| Proxy + REST | 4,000-6,000 | ~2 сек | 90%+ |

---

## 🎯 Немедленные действия

```bash
# 1. Попробовать REST API с правильными headers
curl -H "User-Agent: Mozilla/5.0..." \
     -H "Referer: https://winline.ru/stavki/sport/futbol" \
     -H "Accept: application/json" \
     "https://winline.ru/api/cls/menu/sport/205/country-xy/8-22" \
     -b "cookies.txt" -c "cookies.txt"

# 2. Загрузить и распарсить websocket.js
curl "https://winline.ru/api/v2/websocket.js" > winline_websocket.js
# Анализировать: как инициализировать, какие сообщения отправлять

# 3. Реализовать в Rust
# Создать HTTP client с правильными headers
# ИЛИ WebSocket client для real-time events
```

---

## 📈 После взлома Winline

- **Получим**: +3,000-5,000 события/день
- **Время на взлом**: 5-8 часов
- **ROI**: ОЧЕНЬ ВЫСОКИЙ (Winline - один из самых популярных БК)

---

## ⚠️ Важно

- **НЕ использовать обычный Playwright** - легко детектируется
- **Использовать реальный Chrome** с профилем и cookies
- **Добавить задержки** между запросами (1-2 сек)
- **Ротировать User-Agents** и Referers
- **Мониторить HTTP коды** (429 = rate limit, 403 = block)
- **Использовать прокси** если сайт забл

окирует IP

---

**Статус**: Анализ завершен, стратегия определена, готов к реализации.

**Следующий шаг**: Выбрать между REST API + Headers ИЛИ WebSocket и начать реализацию.

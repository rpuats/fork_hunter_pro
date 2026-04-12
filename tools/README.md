# 🕵️ BK API Discovery Tool

Инструмент для обнаружения актуальных API эндпоинтов букмекерских сайтов.

## Установка

```bash
pip install mitmproxy requests
```

## Использование

### 1. Автоматический поиск API URL
```bash
python tools/bk_api_discovery.py --list                    # Список поддерживаемых БК
python tools/bk_api_discovery.py --bk fonbet --url https://fonbet.win  # Запуск перехвата
```

### 2. Как это работает

1. Скрипт запускает локальный прокси (порт 8080)
2. Открываете браузер с прокси
3. Заходите на сайт букмекера
4. Скрипт перехватывает все API запросы
5. Сохраняет JSON ответы в `discovery_output/<bk>/`
6. Генерирует шаблон парсера

### 3. Результат

После перехвата вы получите:
- `discovery_output/<bk>/api_endpoints_*.json` — список найденных эндпоинтов
- `discovery_output/<bk>/json_examples/` — примеры JSON ответов
- `discovery_output/<bk>/parser_template_<bk>.py` — готовый шаблон парсера

## Известные API паттерны

### Pari / Fonbet / Bettery / Marathon (shared platform)
Эти БК используют общую платформу с динамическими доменами:
```
https://lineXX-XXXXXXX.XXXXX-resources.com/events/list?lang=ru&scopeMarket=XXXX
https://lineXX-XXXXXXX.XXXXX-resources.com/events/listBase?lang=ru&scopeMarket=XXXX
```

Scope Market IDs:
- Pari: 2300
- Bettery: 501
- Marathon: 3000
- Fonbet: 1 (предположительно)

### Winline
Использует Playwright для рендеринга SPA.

### Liga Stavok
Защищён QRATOR и использует POST API на `lds-api-sites.ligastavok.ru`.

Известные рабочие точки discovery:
```
https://lds-api-sites.ligastavok.ru/rest/events/v8/eventsList
https://lds-api-sites.ligastavok.ru/rest/events/v8/actionLines
https://lds-api-sites.ligastavok.ru/rest/events/v8/tournamentTree
https://lds-api-sites.ligastavok.ru/rest/events/v2/filter
```

Специальный discovery helper:
```bash
python tools/ligastavok_discovery.py
python tools/ligastavok_discovery.py --headless
```

Полезные артефакты:
- `tools/ligastavok_discovery_config.json` — известные endpoint/payload hints
- `ligastavok_cookies.json` — можно подать в helper для повторного запуска
- `network_capture/ligastavok_network.json` — эталонный захват с живыми ответами

### Zenit / Betcity / Baltbet
Требуют обнаружения актуальных API URL.

## Поиск новых API URL

### Метод 1: Перехват трафика
1. Откройте DevTools в браузере (F12)
2. Перейдите на вкладку Network
3. Отфильтруйте по XHR/Fetch
4. Загрузите линию букмекера
5. Найдите запросы к API (обычно содержат /api/, /events/, /line/)

### Метод 2: Анализ JavaScript
1. Откройте исходный код страницы
2. Найдите JS файлы
3. Ищите URL API в коде (поиск по "api", "endpoint", "fetch")

### Метод 3: DNS запросы
1. Откройте DevTools → Network
2. Отфильтруйте по domain
3. Найдите все домены, к которым обращается сайт

## Добавление нового парсера

1. Найдите API URL букмекера
2. Скопируйте шаблон из `tools/discovery_output/<bk>/parser_template_<bk>.py`
3. Заполните методы `parse_events` и `parse_odds` на основе JSON примеров
4. Добавьте парсер в `crates/parsers/src/`
5. Зарегистрируйте в `parser_factory.rs`

## Troubleshooting

### API возвращает 403 Forbidden
- Нужны cookies или авторизация
- Попробуйте скопировать заголовки из браузера

### API возвращает HTML вместо JSON
- Возможно, нужен другой User-Agent
- Или сайт использует GraphQL (ищите /graphql запросы)

### Динамические домены
- Некоторые БК меняют домены API
- Нужно реализовать discovery механизм (как у Pari/Marathon)

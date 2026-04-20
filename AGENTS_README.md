# 🚀 Fork-OS Parallel Agents - Запуск и управление

## Быстрый старт

```bash
# Запустить все агенты (автоматический build + start)
.\fork_os.ps1 start

# Проверить статус
.\fork_os.ps1 status

# Остановить все агенты
.\fork_os.ps1 stop
```

## Что запускается

### 🕷️ Парсеры (7 агентов)
Каждый парсер работает независимо, параллельно собирая события:

- **Pari Agent** — ~6600 событий
- **Fonbet Agent** — ~6800 событий
- **Bettery Agent** — ~6800 событий
- **Marathon Agent** — ~6500 событий
- **24bet Agent** — ~6500 событий
- **Leon Agent** — ~3600 событий
- **Sportbet Agent** — ~250 событий

**Итого:** ~40k+ событий в цикле

### 🧮 Движок (2 агента)
- **Surebet Calculator** — поиск вилок в 8 рынках
- **Event Normalizer** — нормализация имен команд/лиг

### 🔀 Анализ & Оптимизация (3 агента)
- **Cross-BK Matcher** — матчинг событий между БК (97.5% accuracy)
- **Problem BK Debugger** — диагностика Olimp, Zenit, Betcity
- **Parser Optimizer** — профилирование и оптимизация парсеров

## Архитектура

```
┌─────────────────────────────────────────────────────────┐
│           🚀 FORK-OS ORCHESTRATOR (Main)               │
└─────────────────────────────────────────────────────────┘
                            │
        ┌───────────────────┼───────────────────┐
        │                   │                   │
   ┌────▼──────┐      ┌─────▼──────┐      ┌────▼──────┐
   │ 7 PARSERS │      │ 2 ENGINES  │      │ 3 HELPERS │
   │ (parallel)│      │ (pipeline) │      │(monitoring)
   └────┬──────┘      └─────┬──────┘      └────┬──────┘
        │                   │                   │
    [Data Flow]         [Events]            [Metrics]
        │                   │                   │
        └───────────────────┼───────────────────┘
                            │
                    ┌───────▼────────┐
                    │  API (Axum)    │
                    │ localhost:8080 │
                    └────────────────┘
```

## API Endpoints

Все агенты записывают результаты в API:

```bash
# Проверка здоровья
curl http://localhost:8080/api/v1/health

# Метрики сканирования
curl http://localhost:8080/api/v1/metrics

# Найденные вилки
curl http://localhost:8080/api/v1/surebets

# Здоровье парсеров
curl http://localhost:8080/api/v1/parsers/health

# Покрытие парсеров
curl http://localhost:8080/api/v1/parsers/coverage

# WebSocket real-time
wscat -c ws://localhost:8080/ws
```

## Файлы результатов

Агенты сохраняют результаты в файлы:

```
agent_results/
├── pari_parser_result.json          # Результат Pari парсера
├── fonbet_parser_result.json        # Результат Fonbet парсера
├── ...
├── calculator_result.json           # Результат калькулятора
└── normalizer_result.json           # Результат нормализатора

generated_ideas.jsonl               # Идеи по улучшению (1 JSON/строка)
parser_performance.json             # Метрики производительности
debug_results.log                   # Результаты диагностики проблемных БК
```

## Мониторинг в реальном времени

### Terminal 1: Основные агенты
```bash
.\fork_os.ps1 start
```

### Terminal 2: Смотреть метрики
```bash
while ($true) {
    cls
    $metrics = Invoke-RestMethod "http://localhost:8080/api/v1/metrics"
    $metrics | ConvertTo-Json
    Start-Sleep -Seconds 5
}
```

### Terminal 3: WebSocket stream
```bash
npm install -g wscat
wscat -c ws://localhost:8080/ws
```

### Terminal 4: Смотреть логи агентов
```bash
Get-Content agent_results/*.json -Tail 10 -Wait
```

## Конфигурация

### Timeout настройки (run_all_agents.py)

```python
AGENTS = {
    "pari_parser": { "timeout": 300 },  # 5 мин
    "calculator": { "timeout": 300 },   # 5 мин
    # ... и т.д.
}
```

### Цикл сканирования

По умолчанию:
- **Парсеры:** каждый работает независимо
- **Калькулятор:** непрерывно обрабатывает события
- **Дебаггер:** проверяет проблемные БК каждые 60 сек
- **Оптимизатор:** профилирует производительность каждые 120 сек
- **Генератор идей:** анализирует состояние каждые 180 сек

## Troubleshooting

### Агенты не запускаются
```bash
# Проверить Rust установку
rustc --version

# Проверить Python
python --version

# Проверить что проект компилируется
cargo build --release
```

### API не отвечает
```bash
# Проверить что основной сканер запущен
Get-Process | Where-Object { $_.ProcessName -like "*fork*" }

# Проверить порт 8080
netstat -ano | findstr :8080

# Запустить вручную
cargo run --release --bin fork-hunter-bin
```

### Парсеры не собирают события
```bash
# Проверить здоровье парсеров
curl http://localhost:8080/api/v1/parsers/health

# Запустить диагностику вручную
python debug_problem_bks.py
```

### Проверить результаты агентов
```bash
# Посмотреть результаты
Get-Content agent_results/*.json | head -20

# Посмотреть идеи
Get-Content generated_ideas.jsonl | ConvertFrom-Json | Select title, priority

# Посмотреть метрики производительности
Get-Content parser_performance.json | Tail -5 | ConvertFrom-Json
```

## Останов системы

```bash
# Graceful shutdown
.\fork_os.ps1 stop

# Или Ctrl+C в главном окне (orchestrator)
```

## Метрики успеха

Система работает правильно когда:

✅ **Парсеры:** собирают 30k+ событий в цикл  
✅ **Калькулятор:** находит вилки (или 0 если рынок эффективен)  
✅ **Матчинг:** 97%+ событий матчится между БК  
✅ **Цикл:** ~30 секунд на полное сканирование  
✅ **API:** отвечает за <100ms  
✅ **Ошибки:** <1% error rate в парсерах  

## Дополнительные команды

### Rebuild проекта
```bash
cargo clean
cargo build --release
```

### Запустить только тесты
```bash
cargo test --release
```

### Запустить один парсер вручную
```bash
cargo run --release --bin fork-hunter-bin -- --parser pari
```

### Смотреть live логи
```bash
cargo run --release --bin fork-hunter-bin -- --log-level debug
```

---

## Статус

- **Создано:** April 18, 2026
- **Агентов:** 12 (7 parsers + 2 engines + 3 helpers)
- **Тестов:** 91 (100% pass)
- **API endpoints:** 40+
- **Status:** BETA (feature-complete)

**Next:** Разблокировать Olimp + улучшить Zenit/Betcity

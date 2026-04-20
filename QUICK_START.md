# 🚀 Quick Start - Fork-OS Parallel Agents System

> Сканер вилок для РФ БК с 12+ агентами, работающими параллельно

## ⚡ Запуск в 1 команду

### Windows (PowerShell)
```powershell
.\fork_os.ps1 start
```

### Linux/Mac (Bash)
```bash
chmod +x launch_all_agents.sh stop_agents.sh
./launch_all_agents.sh
```

## 🎯 Что произойдет

Запустятся **12 агентов одновременно**:

```
🕷️  7x Парсеры БК          → собирают события
🧮 2x Движок              → вычисляют вилки
🔀 3x Анализаторы         → оптимизация + диагностика
────────────────────────────────────────────
✅ 40k+ событий в цикл
✅ 97.5% cross-BK matching
✅ Real-time API (40+ endpoints)
✅ WebSocket live updates
```

## 📊 Доступные API

После запуска все тесты будут доступны:

```bash
# Здоровье системы
curl http://localhost:8080/api/v1/health

# Метрики сканирования
curl http://localhost:8080/api/v1/metrics

# Найденные вилки (arbitrage opportunities)
curl http://localhost:8080/api/v1/surebets

# Здоровье парсеров
curl http://localhost:8080/api/v1/parsers/health

# Информация о покрытии БК
curl http://localhost:8080/api/v1/parsers/coverage

# Индекс щедрости БК
curl http://localhost:8080/api/v1/analytics/generosity

# Real-time WebSocket
wscat -c ws://localhost:8080/ws
```

## 📁 Файлы результатов

Агенты сохраняют результаты в реальном времени:

```
agent_results/
├── pari_parser_result.json
├── fonbet_parser_result.json
├── bettery_parser_result.json
├── marathon_parser_result.json
├── 24bet_parser_result.json
├── leon_parser_result.json
├── sportbet_parser_result.json
├── calculator_result.json
├── normalizer_result.json
├── cross_bk_matcher_result.json
├── problem_debugger_result.json
└── parser_optimizer_result.json

generated_ideas.jsonl          # Идеи по оптимизации
parser_performance.json        # Метрики производительности
debug_results.log             # Диагностика проблемных БК
```

## 📈 Мониторинг в реальном времени

Откройте несколько терминалов:

### Terminal 1: Основная система
```powershell
.\fork_os.ps1 start
```

### Terminal 2: Watch метрики
```powershell
# PowerShell
while ($true) { 
    Clear-Host
    Invoke-RestMethod "http://localhost:8080/api/v1/metrics" | ConvertTo-Json
    Start-Sleep 5
}
```

```bash
# Bash
while true; do
    clear
    curl http://localhost:8080/api/v1/metrics 2>/dev/null | jq .
    sleep 5
done
```

### Terminal 3: Watch здоровье парсеров
```bash
# Bash (лучше)
watch -n5 'curl http://localhost:8080/api/v1/parsers/health 2>/dev/null | jq .'

# PowerShell
while ($true) {
    Clear-Host
    Invoke-RestMethod "http://localhost:8080/api/v1/parsers/health" | ConvertTo-Json
    Start-Sleep 5
}
```

### Terminal 4: Watch логи агентов
```bash
tail -f agent_results/*.log

# или смотреть по одному
tail -f agent_results/pari_parser.log
tail -f agent_results/calculator.log
tail -f agent_results/problem_debugger.log
```

### Terminal 5: WebSocket stream
```bash
npm install -g wscat
wscat -c ws://localhost:8080/ws
```

## 🛑 Остановка системы

### Windows (PowerShell)
```powershell
.\fork_os.ps1 stop
```

### Linux/Mac (Bash)
```bash
./stop_agents.sh

# Или вручную:
for pid in agent_results/*.pid; do kill $(cat $pid) 2>/dev/null; done
```

## 📊 Проверка статуса

### Windows (PowerShell)
```powershell
.\fork_os.ps1 status
```

### Linux/Mac (Bash)
```bash
ps aux | grep -E "(python|cargo)" | grep -E "(fork|pari|fonbet|bettery|marathon)"
```

## 🔧 Troubleshooting

### Агенты не запускаются
```bash
# Проверить что проект компилируется
cargo build --release

# Запустить тесты
cargo test --release
```

### API не отвечает
```bash
# Проверить что порт свободен
lsof -i :8080  # Linux/Mac
netstat -ano | findstr :8080  # Windows

# Запустить сканер вручную
cargo run --release --bin fork-hunter-bin
```

### Парсеры не собирают события
```bash
# Проверить здоровье
curl http://localhost:8080/api/v1/parsers/health | jq

# Запустить диагностику
python3 debug_problem_bks.py
```

## 📚 Документация

- **PROJECT_GUIDE.md** — Полный гайд проекта
- **AGENTS_README.md** — Детали каждого агента
- **AGENTS.md** — История статуса всех 63 агентов (Fork-OS)
- **DIAGNOSTIC_REPORT.md** — Диагностика от 04-10

## 🎯 Метрики успеха

Система работает правильно когда:

✅ Все 12 агентов запустились (смотреть в логах)  
✅ API отвечает на http://localhost:8080/api/v1/health  
✅ Парсеры собирают события (~40k в цикл)  
✅ Калькулятор находит вилки (или 0 если рынок эффективен)  
✅ Матчинг показывает 97%+ событий  
✅ Error rate <1% в парсерах  

## 💡 Советы

1. **Первый запуск:** может занять 5-10 мин на build
2. **Мониторинг:** используйте несколько терминалов для наблюдения
3. **Логи:** все логи в `agent_results/*.log`, проверяйте при проблемах
4. **API:** используйте `jq` для красивого JSON вывода
5. **WebSocket:** `wscat` удобнее всего для просмотра real-time events

## 🔄 Typical Workflow

```
1. Запустить систему       → .\fork_os.ps1 start
2. Открыть 5 терминалов    → monitoring на каждый компонент
3. Смотреть метрики        → curl http://localhost:8080/api/v1/metrics
4. Проверить вилки         → curl http://localhost:8080/api/v1/surebets
5. Анализировать идеи      → jq . < generated_ideas.jsonl
6. Остановить              → .\fork_os.ps1 stop
```

## 🚀 Что дальше

После успешного запуска:

1. **Разблокировать Olimp** — использовать прокси (есть в config)
2. **Улучшить Zenit/Betcity** — диагностировать падения
3. **Добавить новые рынки** — Correct Score, Asian Handicap
4. **Запустить UI** — React dashboard с WebSocket
5. **Включить автоставки** — Kelly-based bet placement

---

**Version:** 1.0  
**Status:** BETA (feature-complete)  
**Last Updated:** April 18, 2026

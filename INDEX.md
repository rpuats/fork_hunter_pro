# 📑 Fork-OS Documentation Index

## 🚀 Getting Started

### Quick Start (⭐ начните отсюда!)
- **[QUICK_START.md](QUICK_START.md)** — 1-команда запуск всей системы
  - Windows: `.\fork_os.ps1 start`
  - Linux: `./launch_all_agents.sh`

### Health Check (перед запуском)
```bash
./health_check.sh
```

## 📘 Documentation

### Project Overview
- **[PROJECT_GUIDE.md](PROJECT_GUIDE.md)** — Полный гайд по проекту
  - Структура 13 крейтов
  - 17 парсеров БК (7 рабочих)
  - 8 модулей движка
  - Roadmap и текущие проблемы

- **[AGENTS.md](AGENTS.md)** — История всех 63 агентов Fork-OS
  - Event Bus архитектура
  - 61 агент по ролям (backend, parsers, frontend, devops, qa, etc)
  - Pipeline: Scraper → Normalizer → Calculator → Notifications

### Agents System
- **[AGENTS_README.md](AGENTS_README.md)** — Детали 12 параллельных агентов
  - Архитектура orchestrator
  - Каждый агент (parsers, engines, helpers)
  - Мониторинг и результаты

## 🎛️ Control Scripts

### Main Control (Start/Stop/Status)

#### Windows (PowerShell)
```powershell
.\fork_os.ps1 start    # Запустить все агенты
.\fork_os.ps1 status   # Показать статус
.\fork_os.ps1 stop     # Остановить все
.\fork_os.ps1 help     # Справка
```

#### Linux/Mac (Bash)
```bash
./health_check.sh      # Проверка перед запуском
./launch_all_agents.sh # Запустить все агенты
./stop_agents.sh       # Остановить все
```

#### Direct (Python)
```bash
python3 run_all_agents.py
```

## 🔧 Agent Scripts

| Script | Purpose | Type | Output |
|--------|---------|------|--------|
| **run_all_agents.py** | Главный orchestrator (ThreadPoolExecutor) | Python | agent_results/*.json |
| **debug_problem_bks.py** | Диагностика Olimp, Zenit, Betcity | Python | debug_results.log |
| **optimize_parsers.py** | Профилирование парсеров | Python | parser_performance.json |
| **generate_ideas.py** | Генератор идей по улучшению | Python | generated_ideas.jsonl |

## 📊 API Documentation

### Health & Status
```bash
curl http://localhost:8080/api/v1/health
curl http://localhost:8080/api/v1/metrics
curl http://localhost:8080/api/v1/scanner/status
```

### Surebets & Arbitrage
```bash
curl http://localhost:8080/api/v1/surebets
curl http://localhost:8080/api/v1/express-forks
curl http://localhost:8080/api/v1/corridors
curl http://localhost:8080/api/v1/odds-errors
```

### Bookmakers & Parsers
```bash
curl http://localhost:8080/api/v1/bookmakers
curl http://localhost:8080/api/v1/parsers/health
curl http://localhost:8080/api/v1/parsers/coverage
curl http://localhost:8080/api/v1/parsers/promotion-kpi
```

### Value & Analytics
```bash
curl http://localhost:8080/api/v1/value-bets
curl http://localhost:8080/api/v1/analytics/generosity
curl http://localhost:8080/api/v1/freebets
curl http://localhost:8080/api/v1/freebets/summary
```

### Real-time
```bash
# WebSocket
wscat -c ws://localhost:8080/ws

# Or curl with watch
watch -n1 'curl http://localhost:8080/api/v1/metrics 2>/dev/null | jq .'
```

## 🕷️ Parsers (БК)

### Active (7)
| БК | Events | Status | Parser |
|----|--------|--------|--------|
| Pari | 6600 | ✅ | pari.rs |
| Fonbet | 6800 | ✅ | fonbet.rs |
| Bettery | 6800 | ✅ | bettery.rs |
| Marathon | 6500 | ✅ | marathon.rs |
| 24bet | 6500 | ✅ FIXED | bet24.rs |
| Leon | 3600 | ✅ | leon.rs |
| Sportbet | 250 | ✅ | sportbet.rs |

### Experimental (7)
| БК | Events | Status | Parser |
|----|--------|--------|--------|
| Winline | 5000 | ⚠️ | winline.rs |
| Zenit | 4000 | ⚠️ | zenit.rs |
| Betcity | 5000 | ⚠️ | betcity.rs |
| Baltbet | 5000 | ⚠️ | baltbet.rs |
| Liga Stavok | ? | ⚠️ | liga_stavok.rs |
| Betboom | 0 | 📊 | betboom.rs |
| Melbet | 0 | 📊 | melbet.rs |

### Blocked (3)
| БК | Events | Issue | Parser |
|----|--------|-------|--------|
| Olimp | 0 | HTTP 403 | olimp.rs |
| Tennisi | 0 | Disabled | tennisi.rs |
| Olimpbet | 0 | Disabled | olimpbet.rs |

## 🧮 Engine Modules

| Module | Purpose | Lines | Tests |
|--------|---------|-------|-------|
| **calculator.rs** | Find surebets (8 markets) | ~300 | 8 |
| **normalizer.rs** | Team/league normalization | ~400 | 6 |
| **event_pool.rs** | Event caching + dedup | ~150 | 2 |
| **freebet.rs** | Freebet hunting | ~300 | 3 |
| **generosity.rs** | Bookmaker generosity index | ~250 | 2 |
| **mirror.rs** | Mirror line detection | ~100 | 2 |
| **momentum.rs** | Live arbitrage detection | ~150 | 2 |
| **verifier.rs** | Surebet verification | ~150 | 2 |
| **corridor.rs** | Line corridor detection | ~100 | 2 |
| **odds_errors.rs** | Anomaly detection | ~250 | 2 |
| **value.rs** | Value betting | ~300 | 2 |

**Total:** ~35k LOC, 91 tests (100% pass)

## 📈 Monitoring

### Live Metrics (Real-time)
```bash
# Watch cycle time
watch -n1 'curl http://localhost:8080/api/v1/metrics 2>/dev/null | jq .cycle_time_ms'

# Watch event count
watch -n5 'curl http://localhost:8080/api/v1/metrics 2>/dev/null | jq .total_events'

# Watch parser health
watch -n5 'curl http://localhost:8080/api/v1/parsers/health 2>/dev/null | jq keys'
```

### Agent Logs
```bash
# All logs
tail -f agent_results/*.log

# Specific agent
tail -f agent_results/pari_parser.log
tail -f agent_results/calculator.log
tail -f agent_results/problem_debugger.log

# Follow newly created logs
tail -F agent_results/*.log
```

### Results & Ideas
```bash
# Ideas
jq '.' generated_ideas.jsonl | head -20

# Parser metrics
jq '.' parser_performance.json | tail -5

# Debug results
cat debug_results.log | tail -20
```

## 🔄 Typical Workflow

```
1. cargo build --release              # Build everything
2. ./health_check.sh                  # Verify system ready
3. .\fork_os.ps1 start                # Launch all 12 agents
4. curl http://localhost:8080/...     # Query APIs
5. tail -f agent_results/*.log        # Monitor progress
6. .\fork_os.ps1 status               # Check status
7. .\fork_os.ps1 stop                 # Shutdown gracefully
```

## 📊 Project Stats

| Metric | Value |
|--------|-------|
| **Language** | Rust |
| **Crates** | 13 |
| **Parsers** | 17 (7 active) |
| **Tests** | 91 (100% pass) |
| **API Endpoints** | 40+ |
| **Agents** | 12 parallel |
| **Cross-BK Accuracy** | 97.5% |
| **Cycle Time** | ~30 sec |
| **Code Size** | ~35k LOC |

## ⏱️ Performance

- **Scan cycle:** ~30 seconds
- **Events collected:** ~40k per cycle
- **Parser timeout:** 5 sec each
- **Total events:** 3,800-4,000 cross-BK matched
- **API response:** <100ms
- **Error rate:** <1%

## 🎯 Success Metrics

✅ All 12 agents running  
✅ API responding (health check pass)  
✅ 40k+ events per cycle  
✅ 97% cross-BK matching  
✅ <1% error rate  
✅ ~30 sec cycle time  

## 🚀 Next Steps

1. **Разблокировать Olimp** — Use proxy rotation
2. **Починить Zenit/Betcity** — Diagnose transient failures
3. **Добавить новые рынки** — Correct Score, Asian Handicap
4. **Автоставки** — Kelly-based bet placement
5. **UI Dashboard** — React + WebSocket real-time
6. **ML/Analytics** — Fuzzy matching, trend prediction

## 📚 Related Files

- **Cargo.toml** — Workspace definition
- **crates/** — Source code
- **tests/** — Unit & integration tests
- **DIAGNOSTIC_REPORT.md** — Latest diagnostics
- **FIXES_2026_04_10.md** — Recent fixes

## 🆘 Help

For issues:
1. Check **QUICK_START.md** — troubleshooting section
2. Run `./health_check.sh` — verify system
3. Check `agent_results/*.log` — agent logs
4. Check API `/api/v1/parsers/health` — parser status
5. Run `python3 debug_problem_bks.py` — debug script

---

**Version:** 1.0  
**Status:** BETA (feature-complete)  
**Last Updated:** April 18, 2026  
**Agents:** 12/12 operational  
**Tests:** 91/91 passing ✅

# 👻 Sprint 1 Vision — Ghost Imperium

**Author:** VISIONARY (Product Owner & Creative Director)
**Date:** 2026-03-31
**Status:** Draft — Ready for Architect Review

---

## 1. Feature Ideas — 10 Innovative Features

### 1.1 🎯 Smart Freebet Optimizer

**What:** Автоматический детектор и оптимизатор фрибетов. Сканирует бонусные предложения всех 12 БК, рассчитывает оптимальную стратегию отыгрыша через арбитраж (matched betting).

**How it works:**
- Мониторинг бонусных программ (приветственные, reload, cashback)
- Расчёт EV+ стратегий отыгрыша: фрибет на один исход + хеджирование на другой БК
- Симуляция ROI до/после отыгрыша с учётом условий (вэйджер, мин. кэф, сроки)
- Автоматический подбор пары БК для хеджирования фрибета

**Why competitors lack this:** BetBurger и BreakingBet показывают бонусы как статичную информацию. Ни один не рассчитывает оптимальную стратегию отыгрыша через арбитраж.

---

### 1.2 📐 Corridor Scanner (Сканер коридоров)

**What:** Поиск коридорных ситуаций — когда две ставки на разные тоталы/форы создают «окно» выигрыша обеих или минимального проигрыша одной.

**How it works:**
- Обнаружение пар: ТБ 2.5 (кэф 1.85) + ТМ 3.5 (кэф 1.90) → коридор на 3 гола
- Расчёт трёх сценариев: обе выигрывают / одна выигрывает / одна проигрывает
- Оценка EV коридора с учётом вероятности попадания в «окно»
- Приоритезация по соотношению риск/прибыль

**Formula:**
```
Коридор = (ТБ X) + (ТМ Y) где X < Y
Сценарий A: обе выигрывают (попадание в коридор)
Сценарий B: одна выигрывает, другая проигрывает
Сценарий C: обе проигрывают (невозможно при правильном коридоре)
EV = P(A) × (Profit_A) + P(B) × (Profit_B)
```

**Why it matters:** OddStorm и ArbMate уже поддерживают коридоры. BreakingBet — middles. Это must-have для профессионалов.

---

### 1.3 💎 Value Bet Detector

**What:** Выявление переоценённых коэффициентов (value bets) через сравнение кэфов всех 12 БК и расчёт «справедливого» кэфа.

**How it works:**
- Для каждого события вычисляется средний кэф по всем БК (без маржи)
- БК с кэфом выше среднего на X% = value bet
- Расчёт implied probability vs fair probability
- Фильтрация по минимальному edge (2%, 3%, 5%)
- Отслеживание «щедрых» БК — какие чаще дают value

**Formula:**
```
Fair Odds = 1 / (Σ(1/Ki) / N)  — средний без маржи
Value = (K_bookmaker / K_fair) - 1
Value > 0.05 (5% edge) → сигнал
```

**Why competitors have it:** BetBurger, BreakingBet, OddsJam, RebelBetting — все имеют value bet модуль. Мы обязаны.

---

### 1.4 🛡️ Anti-Fraud Evasion System (Ghost Mode)

**What:** Система защиты от обнаружения и блокировки аккаунтов букмекерами.

**Components:**
- **Smart Stake Rounding:** Автоматическое округление ставок до «естественных» сумм (не 1347₽, а 1350₽ или 1400₽)
- **Bet Timing Randomization:** Случайная задержка между ставками (3-45 сек), имитация человеческого поведения
- **Account Heat Tracker:** Отслеживание «подозрительности» каждого аккаунта — частые выигрыши, резкие лимиты
- **Pattern Obfuscation:** Ротация рынков (не всегда 1X2, иногда тоталы, форы, статистика)
- **Loss Simulation:** Периодические «ошибочные» ставки на малые суммы для маскировки
- **Browser Fingerprint Rotation:** При использовании Playwright — ротация User-Agent, screen resolution, timezone

**Why it matters:** Главная проблема вилочников — порезка лимитов. Ни один конкурент не предлагает встроенную защиту на уровне клиента.

---

### 1.5 🏦 Smart Bankroll Manager

**What:** Интеллектуальное управление банкроллом с оптимизацией размера ставок по каждому БК.

**How it works:**
- Отслеживание баланса по каждой БК в реальном времени
- Автоматический расчёт оптимальной ставки с учётом:
  - Текущего баланса на каждой БК
  - Лимитов БК (макс. ставка)
  - «Температуры» аккаунта (risk of cut)
  - Целевого ROI
- Kelly Criterion для value bets
- Автоматический ребаланс: когда баланс на одной БК заканчивается — предлагает вывод/пополнение
- Прогноз «времени жизни» аккаунта до порезки

**Formula (Kelly for value bets):**
```
f* = (bp - q) / b
где b = кэф - 1, p = fair probability, q = 1 - p
```

---

### 1.6 ⚡ Live Scanner Engine

**What:** Сканер лайв-вилок с WebSocket-подпиской на изменения кэфов в реальном времени.

**How it works:**
- WebSocket подключение к БК, которые поддерживают (Winline, BetBoom)
- Polling с адаптивным интервалом для остальных (1-3 сек для лайва)
- Приоритизация «горячих» событий — где кэфы меняются чаще
- Pre-calculation: предварительный расчёт потенциальных вилок до их появления
- Flash alerts: мгновенное уведомление при появлении лайв-вилки > 3%

**Why competitors have it:** BetBurger — лидер в лайве (3 сек задержка). OddStorm — 1-3 сек. BreakingBet — лайв в beta с 25 сек задержкой. Наша цель: < 5 сек.

---

### 1.7 📊 Bookmaker Reliability Score

**What:** Система оценки надёжности каждой БК на основе исторических данных.

**Metrics tracked:**
- **Odds Accuracy:** Как часто кэфы меняются после показа (false positive rate)
- **Bet Acceptance Rate:** Какой % ставок принимается без ошибок
- **Payout Speed:** Время вывода средств (ручной ввод пользователя)
- **Cut Risk Index:** Вероятность порезки лимитов на основе паттернов
- **Market Depth:** Количество рынков и событий
- **Uptime:** Доступность API/сайта

**Output:** Рейтинг БК от 1 до 100, который влияет на приоритизацию вилок.

---

### 1.8 🔄 Cross-Market Arbitrage

**What:** Поиск вилок между разными рынками одного события (не только 1X2 vs 1X2).

**Examples:**
- П1 (1X2) vs Фора2(+1.5) — кросс-рынок
- ТБ 2.5 vs ТМ 2.5 — стандартный
- Индивидуальный тотал команды vs общий тотал
- Комбинированные рынки (1X + ТБ)

**How it works:**
- Расширенный маппинг рынков: определение эквивалентных исходов на разных рынках
- Логика пересечения: если исход A на рынке X покрывает исход B на рынке Y
- Проверка непротиворечивости: исключение ситуаций, где оба исхода могут проиграть

**Why it matters:** ArbMate поддерживает cross-market. Это расширяет пул вилок на 30-50%.

---

### 1.9 🤖 Auto-Bet Executor (Phase 2)

**What:** Полуавтоматическое размещение ставок через API/браузер с подтверждением пользователя.

**How it works:**
- Browser automation (Playwright) для БК без API
- One-click bet placement: клик по вилке → авто-заполнение купона → подтверждение
- Sequential betting: сначала ставка на БК с быстро меняющимся кэфом, потом на вторую
- Fallback: если первая ставка не прошла — отмена/уведомление
- Bet slip validation: проверка кэфа перед подтверждением (если изменился > 2% — предупреждение)

**Safety:** Всегда требует подтверждения пользователя. Полностью автоматический режим — опционально и с предупреждением.

---

### 1.10 📈 Predictive Odds Movement

**What:** Предсказание движения коэффициентов на основе исторических паттернов.

**How it works:**
- Сбор истории изменений кэфов по каждому событию
- ML-модель (простая линейная регрессия → со временем LSTM) для предсказания направления
- Сигналы: «кэф на П1 растёт» → лучше ставить сейчас на П2
- Trend alerts: уведомления о трендовых движениях
- Dropping odds: детектор резкого падения кэфов (инсайд, составы, травмы)

**Why it matters:** BreakingBet имеет «dropping odds». Мы идём дальше — предсказание, а не только констатация.

---

## 2. Competitor Analysis

### ODDSCORP
| Что у них | Что у нас | Gap |
|-----------|-----------|-----|
| Sports Events API (B2B) | Свой сканер | Мы — готовое решение, они — API провайдер |
| 280+ БК | 12 российских БК | ⚠️ Узкая география (но глубокая) |
| Real-time odds feed | Mock mode | 🔴 Критический gap |
| B2B фокус | B2C фокус | ✅ Наша ниша |

**Вывод:** ODDSCORP — это API-провайдер, не прямой конкурент. Но их данные могут быть полезны как fallback.

### BetBurger (Лидер рынка)
| Что у них | Что у нас | Gap |
|-----------|-----------|-----|
| 280+ БК, 40+ видов спорта | 12 БК, фокус на РФ | 🔴 Масштаб |
| Prematch + Live | Только prematch | 🔴 Live режим |
| Value Bets | ❌ | 🔴 Нет |
| Middles | ❌ | 🔴 Нет |
| Коридоры | ❌ | 🔴 Нет |
| 60 сек задержка на free | Без задержек | ✅ Наше преимущество |
| €320/мес (full) | Бесплатно (open-source) | ✅ Огромное преимущество |
| Telegram bot | ✅ Есть | ✅ Parity |

### BreakingBet
| Что у них | Что у нас | Gap |
|-----------|-----------|-----|
| 200+ БК | 12 БК | 🔴 |
| Prematch + Live | Prematch | 🔴 |
| Value Bets | ❌ | 🔴 |
| Middles | ❌ | 🔴 |
| CSV экспорт | ✅ Есть | ✅ |
| Заморозка подписки | N/A (free) | ✅ |
| €25-40/мес | Бесплатно | ✅ |
| Browser extension (OddsClicker) | ❌ | 🔴 |
| Betting history | ❌ | 🔴 |

### Surebet.com
| Что у них | Что у нас | Gap |
|-----------|-----------|-----|
| 400+ БК | 12 БК | 🔴 |
| Middles | ❌ | 🔴 |
| Value Bets | ❌ | 🔴 |
| Browser extension | ❌ | 🔴 |
| Free tier (до 1%) | Без ограничений | ✅ |
| €26/мес | Бесплатно | ✅ |

### OddStorm
| Что у них | Что у нас | Gap |
|-----------|-----------|-----|
| 1-3 сек в Live | ~720ms цикл (mock) | ⚠️ Реальные данные |
| Коридоры, middles | ❌ | 🔴 |
| Football only | Мульти-спорт | ✅ |
| €155-390/мес | Бесплатно | ✅ |

### What ALL competitors have that we DON'T:
1. 🔴 **Live scanning** — все топ-сканеры имеют лайв режим
2. 🔴 **Value bets** — стандарт для 2026
3. 🔴 **Middles/Corridors** — OddStorm, ArbMate, BreakingBet
4. 🔴 **Real bookmaker data** — у нас mock mode
5. 🔴 **Browser extension** — для быстрого размещения ставок
6. 🔴 **Betting history & analytics** — учёт ставок, ROI tracking
7. 🔴 **Anti-fraud protection** — ни у кого нет встроенной

### What WE have that competitors DON'T:
1. ✅ **100% free & open-source** — ни у кого
2. ✅ **12 российских БК** — глубокая экспертиза в РФ рынке
3. ✅ **No delays** — BetBurger задерживает free на 60 сек
4. ✅ **Self-hosted** — приватность, никаких подписок
5. ✅ **Telegram bot built-in** — у многих нет
6. ✅ **Cyberpunk Web UI** — лучший UX в классе
7. ✅ **API-first** — BreakingBet только недавно запустил API

---

## 3. Sprint 2 Roadmap

### Priority Matrix

| # | Feature | Priority | Complexity | Dependencies | Sprint |
|---|---------|----------|------------|--------------|--------|
| 1 | **Value Bet Detector** | P0 | M | Calculator, Aggregator | Sprint 2 |
| 2 | **Live Scanner Engine** | P0 | L | WebSocket parsers, Real APIs | Sprint 2-3 |
| 3 | **Corridor Scanner** | P1 | M | Enhanced market mapping | Sprint 2 |
| 4 | **Smart Bankroll Manager** | P1 | M | Database, Analytics | Sprint 2 |
| 5 | **Anti-Fraud Ghost Mode** | P1 | L | Auto-bet, Browser automation | Sprint 3 |
| 6 | **Bookmaker Reliability Score** | P2 | S | Analytics, History DB | Sprint 2 |
| 7 | **Cross-Market Arbitrage** | P1 | L | Market mapping, Normalizer | Sprint 3 |
| 8 | **Smart Freebet Optimizer** | P2 | M | Bonus system, Calculator | Sprint 3 |
| 9 | **Predictive Odds Movement** | P3 | L | History DB, ML pipeline | Sprint 4 |
| 10 | **Auto-Bet Executor** | P2 | L | Playwright, Anti-fraud | Sprint 3-4 |

### Detailed Sprint 2 Plan

---

#### Feature 1: Value Bet Detector
**Priority:** P0 (Critical)
**Complexity:** M (2-3 дня)
**Dependencies:** SurebetCalculator, EventAggregator

**Spec:**
- Новый модуль `core/value_detector.py`
- Алгоритм: расчёт fair odds из кэфов всех БК, сравнение с каждым
- API endpoint: `GET /api/v1/valuebets` с фильтрами (min_edge, sport, bookmaker)
- Telegram уведомления для value > 5%
- Метрика: edge % (насколько кэф выше fair)
- UI: отдельная вкладка в dashboard

**Acceptance Criteria:**
- [ ] Обнаружение value bets с edge > 2%
- [ ] API endpoint возвращает корректные данные
- [ ] Telegram уведомления работают
- [ ] Unit тесты: > 90% coverage

---

#### Feature 2: Corridor Scanner
**Priority:** P1 (High)
**Complexity:** M (2-3 дня)
**Dependencies:** Enhanced market mapping, Normalizer

**Spec:**
- Модуль `core/corridor_finder.py`
- Типы коридоров:
  - Тоталы: ТБ X + ТМ Y (X < Y)
  - Форы: Ф1(-X) + Ф2(+Y) где -X < +Y
  - Индивидуальные тоталы
- Расчёт 3 сценариев: win-both, win-one, lose-one
- API endpoint: `GET /api/v1/corridors`
- Метрика: EV коридора, вероятность попадания в окно

**Acceptance Criteria:**
- [ ] Обнаружение коридоров с положительным EV
- [ ] Расчёт всех 3 сценариев
- [ ] API endpoint с фильтрами
- [ ] Unit тесты

---

#### Feature 3: Smart Bankroll Manager
**Priority:** P1 (High)
**Complexity:** M (2-3 дня)
**Dependencies:** Database, Analytics Engine

**Spec:**
- Модуль `services/bankroll.py`
- Таблица БД: `bankroll_accounts` (bookmaker, balance, currency, status, heat_level)
- Расчёт оптимального размера ставки
- Kelly Criterion для value bets
- Dashboard виджет: балансы по БК
- Telegram команда: `/bankroll`

**Acceptance Criteria:**
- [ ] CRUD для аккаунтов БК
- [ ] Расчёт оптимальной ставки
- [ ] Отображение в UI
- [ ] Telegram команда

---

#### Feature 4: Bookmaker Reliability Score
**Priority:** P2 (Medium)
**Complexity:** S (1-2 дня)
**Dependencies:** Analytics Engine, History DB

**Spec:**
- Модуль `services/reliability.py`
- Метрики: odds_accuracy, bet_acceptance, uptime, cut_risk
- Автоматический расчёт на основе истории
- API: `GET /api/v1/bookmakers/{slug}/reliability`
- Влияние на ранжирование вилок

**Acceptance Criteria:**
- [ ] Расчёт reliability score для каждой БК
- [ ] API endpoint
- [ ] Интеграция с ранжированием вилок

---

#### Feature 5: Live Scanner Engine
**Priority:** P0 (Critical)
**Complexity:** L (5-7 дней)
**Dependencies:** Real API parsers, WebSocket support

**Spec:**
- Модуль `scanner/live_engine.py`
- Адаптивный polling: 1-3 сек для лайва
- WebSocket подписка для Winline, BetBoom
- Приоритизация «горячих» событий
- Flash alerts для лайв-вилок > 3%
- Отдельный конфиг: `LiveScannerConfig`

**Acceptance Criteria:**
- [ ] Live scanning с задержкой < 5 сек
- [ ] WebSocket поддержка для 2+ БК
- [ ] Flash alerts
- [ ] Отдельный режим: prematch / live / both

---

## 4. Unique Selling Points (USP)

### 🏆 Ghost Imperium vs The World

| USP | Описание | Почему это важно |
|-----|----------|-----------------|
| **1. 100% Free & Open-Source** | Никаких подписок, никаких задержек. Полный код доступен. | Конкуренты берут €25-320/мес. Free-версии задерживают данные на 60 сек. |
| **2. Russian Market Deep Dive** | 12 российских БК с глубоким парсингом, включая Playwright для защищённых. | Ни один международный сканер не имеет такой глубины по РФ рынку. |
| **3. Self-Hosted Privacy** | Данные не покидают ваш сервер. Никакого трекинга, никаких утечек. | Профессионалы ценят приватность. Подписочные сервисы видят ваши ставки. |
| **4. Anti-Fraud Built-In** | Ghost Mode: защита от порезки лимитов на уровне клиента. | Ни один конкурент не предлагает это. Это game-changer. |
| **5. API-First Architecture** | REST + WebSocket API для интеграции с любыми системами. | BreakingBet запустил API только в 2026. Мы — API-first с дня 1. |
| **6. Multi-Interface** | Web Dashboard + Telegram Bot + REST API + WebSocket. | Максимальная гибкость: десктоп, мобильный, автоматизация. |
| **7. No Artificial Delays** | Все вилки — мгновенно, без задержек. | BetBurger задерживает free на 60 сек. BreakingBet — 25 сек в лайве. |
| **8. Extensible Plugin System** | Архитектура для добавления новых БК, стратегий, стратегий. | Конкуренты — закрытые системы. Мы — платформа. |

### 🎯 Positioning Statement

> **Ghost Imperium** — единственный бесплатный, self-hosted сканер арбитражных ситуаций с глубокой экспертизой российского рынка и встроенной защитой от обнаружения букмекерами.

### 📊 Competitive Moat

```
                    │  Глубина РФ рынка
                    │
         Ghost      │     BetBurger
        Imperium    │    (широкий, не глубокий)
                    │
    ────────────────┼──────────────────
                    │
     BreakingBet    │    Surebet.com
     (budget)       │    (budget)
                    │
                    │
    ────────────────┼──────────────────
                    │
       Цена:        │
    Free ←──────────→ €320/мес
```

**Наша позиция:** Верхний левый квадрант — максимальная глубина по РФ + бесплатно. Никто не занимает эту нишу.

---

## 5. Strategic Recommendations

### Short-term (Sprint 2-3)
1. **Приоритет #1:** Реальные API парсеры — без них все фичи на mock данных
2. **Приоритет #2:** Value Bet Detector — самый востребованный feature после вилок
3. **Приоритет #3:** Corridor Scanner — дифференциатор от бюджетных конкурентов

### Mid-term (Sprint 4-6)
1. Live Scanner Engine — критично для конкуренции с BetBurger
2. Anti-Fraud Ghost Mode — уникальное преимущество, которого нет ни у кого
3. Auto-Bet Executor — переход от сканера к полноценной системе

### Long-term (Sprint 7+)
1. ML-based Odds Prediction — технологическое превосходство
2. Mobile App (React Native) — расширение аудитории
3. Community/Marketplace — обмен стратегиями, плагинами

### Key Metrics to Track
| Метрика | Цель | Текущее |
|---------|------|---------|
| Скорость сканирования | < 500ms | 720ms |
| Реальные БК подключены | 12/12 | 0/12 |
| Value bets detection | ✅ | ❌ |
| Corridor detection | ✅ | ❌ |
| Live scanning | ✅ | ❌ |
| Anti-fraud features | ✅ | ❌ |

---

*End of Sprint 1 Vision Document*

**Next Steps:**
1. ARCHITECT review → технический дизайн для P0 фич
2. MANAGER → раздача задач DEV SWARM
3. QA → подготовка тест-планов для новых модулей

# FORKING CATCHUP PLAN - Догоняем профессиональный сканер

## 🎯 Цель
Превратить fork_hunter_pro в полноценный профессиональный сканер вилок уровня forking.bet

## 📊 Чек-лист отличий Forking (приоритеты)

### 🔴 P0 - Критично (делаем первым)
| Функция | Описание | Статус |
|---------|----------|--------|
| Scanner Core 300-500ms | WebSocket + инкрементальные обновления | ❌ |
| ForkFinder Engine | 1X2, тоталы, форы, БТТС, коридоры | ❌ |
| Filter System | Top/Extended/Other лиги, мультифильтры | ❌ |
| ForkCard UI | Карточки вилок с цветовой индикацией | ❌ |
| StakingCalculator | Равная прибыль, Келли, фиксированный % | ❌ |
| ExecutionPanel | Авто/Полуавто/Ручной режимы | ❌ |
| WebSocket Real-Time | 30+ типов событий, heartbeat | ❌ |
| Dark Theme | Как у Forking - фиолетово-синяя тема | ❌ |

### 🟡 P1 - Важно (делаем вторым)
| Функция | Описание | Статус |
|---------|----------|--------|
| Sound Alerts | Звуки на новые вилки, высокую прибыль | ❌ |
| Push Notifications | Desktop уведомления Windows/macOS/Linux | ❌ |
| Profiles System | Профили дропов с переключением | ❌ |
| Fingerprint | Уникальный отпечаток на аккаунт | ❌ |
| Corridors Module | Отдельная вкладка коридоров | ❌ |
| Covers (Hedge) | Поиск перекрытий после первого плеча | ❌ |
| FreebetCalculator | Калькулятор отыгрыша бонусов | ❌ |
| Hotkeys | Ctrl+Enter, Escape, Space | ❌ |
| Anti-Detect | Маскировка automation от БК | ❌ |
| BetConfirmationPanel | Панель подтверждения ставок | ❌ |
| Captcha/2FA Modals | Модалки для ручного ввода | ❌ |
| HistoryPage | История и статистика ставок | ❌ |
| SettingsPage | Полная страница настроек | ❌ |

### 🟢 P2 - Желательно (делаем потом)
| Функция | Описание | Статус |
|---------|----------|--------|
| Auto-Updater | Автообновление приложения | ❌ |
| i18n RU/EN | Мультиязычность | ❌ |
| Prometheus Metrics | Метрики для Grafana | ❌ |
| Backup/Restore | Резервное копирование | ❌ |
| Help Panel | Встроенная документация | ❌ |
| Export/Import | CSV, JSON профилей | ❌ |
| Top3 Widgets | Плавающие окна топ вилок | ❌ |

## 🗓️ План реализации (по неделям)

### Неделя 1: Scanner Core
- [ ] Рефакторинг ScannerEngine под 300-500ms refresh
- [ ] WebSocket интеграция
- [ ] ForkFinder: 1X2, тоталы, форы
- [ ] FilterSystem базовый

### Неделя 2: UI Core
- [ ] Dark theme CSS
- [ ] ForkCard компонент
- [ ] ScannerPage layout
- [ ] FilterBar
- [ ] ScannerStats

### Неделя 3: Betting Core
- [ ] StakingCalculator (все стратегии)
- [ ] ExecutionPanel
- [ ] BetConfirmationPanel
- [ ] WebSocket events для ставок

### Неделя 4: Auth & Execution
- [ ] Потоковая авторизация через браузер
- [ ] CaptchaModal
- [ ] TwoFAModal
- [ ] Автозаполнение купона
- [ ] Полуавтоставка

### Неделя 5: Profiles & Filters
- [ ] ProfileManager
- [ ] Мультифильтры
- [ ] Filter presets (Top/Extended/Other)
- [ ] Fingerprint генератор

### Неделя 6: Advanced Features
- [ ] Corridors module
- [ ] Covers (hedge search)
- [ ] FreebetCalculator
- [ ] Hotkeys

### Неделя 7: Polish
- [ ] Sound alerts
- [ ] Push notifications
- [ ] History page
- [ ] Settings page

### Неделя 8+: Optional
- [ ] Auto-updater
- [ ] i18n
- [ ] Metrics
- [ ] Backup

## 📁 Структура файлов (новые)

```
crates/
├── engine/src/
│   ├── fork_finder.rs          # Поиск вилок
│   ├── filters.rs                # Фильтры
│   └── corridors/
│       ├── mod.rs
│       └── finder.rs
│
├── auto_betting/src/
│   ├── auth/
│   │   ├── browser_auth.rs
│   │   ├── session_storage.rs
│   │   └── display_config.rs
│   ├── execution/
│   │   ├── auto_bet.rs
│   │   ├── semi_auto_bet.rs
│   │   └── executor.rs
│   ├── fingerprint/
│   │   └── generator.rs
│   └── profiles/
│       └── models.rs
│
desktop-ui/src/
├── pages/
│   ├── ScannerPage.tsx
│   ├── AccountsPage.tsx
│   ├── BettingPage.tsx
│   ├── CorridorsPage.tsx
│   ├── CoversPage.tsx
│   ├── HistoryPage.tsx
│   ├── ProfilesPage.tsx
│   └── SettingsPage.tsx
│
├── components/
│   ├── ForkCard.tsx
│   ├── StakingCalculator.tsx
│   ├── ExecutionPanel.tsx
│   ├── BetConfirmationPanel.tsx
│   ├── CaptchaModal.tsx
│   ├── TwoFAModal.tsx
│   ├── ProfileManager.tsx
│   ├── FilterEditor.tsx
│   ├── CorridorCard.tsx
│   ├── FreebetCalculator.tsx
│   └── BrowserPreview.tsx
│
├── hooks/
│   ├── useAuth.ts
│   ├── useBetting.ts
│   ├── useProfiles.ts
│   ├── useWebSocket.ts
│   └── useOperatorQueue.ts
│
└── styles/
    └── themes/
        └── dark.css
```

## 🔧 Технические решения

### Scanner Speed (300-500ms)
```rust
// WebSocket где возможно (Fonbet, Marathon)
// HTTP polling с параллелизмом
pub const LIVE_REFRESH_MS: u64 = 400;
pub const PARSER_CONCURRENCY: usize = 15;
```

### Fork Detection Algorithm
```rust
// Сначала нормализация событий (fingerprint)
// Затем поиск по каждому виду рынка
// O(n²) для каждой пары БК
```

### Filter Priority
1. Profit range (min/max %)
2. Odds range
3. Sports filter
4. Leagues filter (Top/Extended/Other)
5. Bookmaker filter
6. Exclusions (women, youth, friendly)

### UI Colors (Forking-style)
```css
--profit-super: #00ff88;    /* > 2% */
--profit-high: #22c55e;      /* 1-2% */
--profit-normal: #84cc16;    /* 0.5-1% */
--profit-low: #eab308;       /* < 0.5% */
--profit-negative: #ef4444;  /* отрицательные */
```

### Staking Strategies
1. **EqualProfit**: Равная прибыль на оба плеча
2. **Proportional**: Пропорционально коэффициентам
3. **FixedAmount**: Фиксированная сумма
4. **KellyCriterion**: Критерий Келли
5. **FlatPercent**: Фиксированный % от банка

### WebSocket Events (30+ типов)
```rust
// Auth events
AuthStarted, AuthProgress, AuthSuccess, AuthFailed
CaptchaRequired, TwoFARequired

// Bet events
BetStarted, BetPreparing, BetAwaitingConfirmation
BetPlaced, BetFailed

// Fork events
ForkDetected, ForkExpired, ForkExecuting, ForkCompleted

// System events
ParserHealthUpdated, BalanceUpdated, QueueUpdated
```

## 🚀 Быстрый старт

Для начала работы:
1. Реализовать `ForkFinder` (crates/engine/src/fork_finder.rs)
2. Создать `ForkCard` UI (desktop-ui/src/components/ForkCard.tsx)
3. Добавить `StakingCalculator` (desktop-ui/src/components/StakingCalculator.tsx)
4. Подключить WebSocket для real-time

## 📊 Метрики успеха

- [ ] Скорость сканирования: 300-500ms
- [ ] Время от вилки до отображения: < 1s
- [ ] Время автоставки: < 5s
- [ ] Время полуавтоставки: < 10s
- [ ] FPS UI: 60

## 📝 Примечания

- Не пытаться сделать всё сразу
- Сначала core функционал, затем polish
- Каждый модуль должен иметь тесты
- UI должен работать без backend (mock data)

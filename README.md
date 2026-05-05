# 👻 Ghost Imperium Pro

**Профессиональная платформа для арбитражного беттинга** — сканер вилок, коридоров и экспресс-ставок с системой управления аккаунтами как у Forking.

![Version](https://img.shields.io/badge/version-2.0.0-purple)
![Rust](https://img.shields.io/badge/Rust-1.75+-orange)
![React](https://img.shields.io/badge/React-18-blue)
![License](https://img.shields.io/badge/license-Proprietary-red)

---

## 🚀 Быстрый старт

```bash
# Клонировать репозиторий
git clone https://github.com/rpuats/fork_hunter_pro.git
cd fork_hunter_pro

# Запуск Desktop UI (development)
cd desktop-ui
npm install
npm run dev

# Сборка production build
npm run build

# Tauri desktop app
cd src-tauri
cargo tauri dev
```

**Dev server:** http://localhost:1420

---

## ✨ Возможности

### 📊 Сканер вилок
- **7 рабочих парсеров** (Pari, Fonbet, Bettery, Marathon, 24bet, Leon, Sportbet)
- **Cross-BK matching** с точностью 97.5%
- **Цикл сканирования** ~30 секунд
- **Real-time обновления** через WebSocket

### 🎛️ Desktop UI
- **8 страниц** с единым дизайном
- **Система профилей** как у Forking (drops, fingerprints, прокси)
- **3 режима авто-ставок** (ручной, полуавто, полный авто)
- **Горячие клавиши** (Ctrl+1..8 для навигации)
- **Анимации** Framer Motion

### 🏦 Управление аккаунтами
- **Несколько профилей** на одну БК
- **Уникальные fingerprints** (screen, timezone, CPU, RAM)
- **Прокси на профиль** (HTTP/SOCKS5)
- **Cookie-based авторизация**
- **Смена дропа в 1 клик**

### 📈 Виды ставок
- **Вилки** (surebets) — классический арбитраж
- **Коридоры** (corridors) — перекрытие тоталов/фор
- **Экспрессы** — конструктор с калькулятором
- **Value ставки** — positive EV

---

## 🏗️ Архитектура

```
┌─────────────────────────────────────────────────────────────┐
│                    Ghost Imperium Pro                       │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐        │
│  │   Desktop    │  │     API      │  │   Tauri      │        │
│  │    UI        │  │   Server     │  │   Bridge     │        │
│  │  (React)     │  │   (Axum)     │  │  (Rust)      │        │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘        │
│         │                 │                 │                │
│         └─────────────────┼─────────────────┘                │
│                           │                                  │
│  ┌────────────────────────┴────────────────────────┐          │
│  │              Rust Core Engine                 │          │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐          │          │
│  │  │Scanner  │ │Calculator│ │Verifier │          │          │
│  │  │Parsers  │ │Normalizer│ │Oddds Err│          │          │
│  │  └─────────┘ └─────────┘ └─────────┘          │          │
│  └────────────────────────────────────────────────┘          │
└─────────────────────────────────────────────────────────────┘
```

---

## 📁 Структура проекта

```
fork_hunter_pro/
├── desktop-ui/              # React + TypeScript + Tailwind
│   ├── src/
│   │   ├── pages/          # 8 страниц приложения
│   │   │   ├── Dashboard.tsx
│   │   │   ├── SurebetsPage.tsx
│   │   │   ├── CorridorsPage.tsx
│   │   │   ├── ExpressPage.tsx
│   │   │   ├── OperatorPage.tsx
│   │   │   ├── AccountsPage.tsx      # ← Профили как у Forking
│   │   │   ├── HistoryPage.tsx
│   │   │   └── SettingsPage.tsx
│   │   ├── components/     # UI компоненты
│   │   │   ├── StatCard.tsx
│   │   │   ├── EmptyState.tsx
│   │   │   ├── Skeleton.tsx
│   │   │   └── Sidebar.tsx
│   │   ├── lib/            # Утилиты и данные
│   │   │   ├── demoData.ts
│   │   │   └── profiles.ts # ← Система профилей
│   │   └── App.tsx
│   ├── src-tauri/          # Desktop shell (Tauri)
│   └── package.json
│
├── crates/                 # Rust core
│   ├── api/               # Axum REST API
│   ├── engine/            # Calculator, Normalizer, Verifier
│   ├── scanner/           # Parsers & web scraping
│   └── shared/            # Models & events
│
├── AGENTS.md              # Документация по агентам
└── ARCHITECTURE.md        # Техническая архитектура
```

---

## 🛠️ Технологии

**Frontend:**
- React 18 + TypeScript
- Vite (build tool)
- Tailwind CSS
- Framer Motion (animations)
- Lucide React (icons)
- Recharts (charts)

**Desktop:**
- Tauri (Rust-based Electron alternative)
- WebSocket для real-time

**Backend:**
- Rust + Axum
- Tokio (async runtime)
- SQLite (local data)

---

## ⌨️ Горячие клавиши

| Клавиша | Действие |
|---------|----------|
| `Ctrl+1` | Обзор (Dashboard) |
| `Ctrl+2` | Вилки (Surebets) |
| `Ctrl+3` | Коридоры (Corridors) |
| `Ctrl+4` | Экспрессы (Express) |
| `Ctrl+5` | Авто-ставки (Operator) |
| `Ctrl+6` | Аккаунты (Accounts) |
| `Ctrl+7` | История (History) |
| `Ctrl+8` | Настройки (Settings) |
| `Ctrl+B` | Свернуть/развернуть sidebar |
| `Ctrl+R` | Обновить данные |
| `Esc` | Закрыть модалку |

---

## 📊 Метрики

| Показатель | Значение |
|------------|----------|
| **Рабочих БК (Rust)** | 7/7 |
| **Cross-BK Match Rate** | 97.5% (3832/3928) |
| **Цикл сканирования** | ~30 сек |
| **Вилок найдено** | 0 (рынок эффективен) |
| **Тесты Rust** | 91 passed |

---

## 📝 API Endpoints

```
GET  /api/v1/health              # Проверка здоровья
GET  /api/v1/metrics             # Метрики сканнера
GET  /api/v1/surebets            # Вилки
GET  /api/v1/corridors           # Коридоры
GET  /api/v1/freebets            # Фрибеты
GET  /api/v1/bookmakers          # Список БК
GET  /api/v1/analytics/generosity # Индекс щедрости
WS   /ws                         # WebSocket real-time
```

---

## 🎨 Цветовая система

```css
/* Основные цвета */
--bg-primary: #0B0F19;      /* Фон */
--accent: #7C3AED;          /* Фиолетовый акцент */
--success: #10B981;         /* Зелёный */
--warning: #F59E0B;         /* Жёлтый */
--error: #EF4444;           /* Красный */
--text-primary: #F1F5F9;   /* Текст */
--text-secondary: #94A3B8; /* Вторичный текст */
```

---

## 🔒 Безопасность

- **Шифрование сессий** — cookies хранятся в зашифрованном виде
- **Master password** — защита данных авторизации
- **2FA** — для операций (опционально)
- **Автоблокировка** — через 5 мин бездействия
- **Изолированные профили** — каждый дроп = отдельный fingerprint

---

## 📱 Скриншоты

> *[Вставить скриншоты UI]*

---

## 🤝 Поддержка

По вопросам и предложениям: [создать issue](https://github.com/rpuats/fork_hunter_pro/issues)

---

## 📜 Лицензия

**Proprietary** — все права защищены. 

Copyright © 2026 Ghost Imperium

---

<p align="center">
  <strong>Made with 🦀 Rust + ⚛️ React</strong>
</p>

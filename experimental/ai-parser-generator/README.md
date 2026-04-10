# 🤖 AI Parser Generator

Генератор парсеров для букмекерских контор на основе AI.

## Что это

Python-скрипт который:
1. Загружает страницу БК через Playwright
2. Отправляет HTML в LLM (OpenAI GPT-4, Claude, или локальную модель)
3. Получает CSS-селекторы для команд и коэффициентов
4. Генерирует готовый Rust-код парсера

## Как использовать

```bash
# 1. Установи зависимости
pip install playwright openai
playwright install chromium

# 2. Запусти генератор
python generate_parser.py \
  --url "https://example-bk.ru/football" \
  --name "NewBookmaker" \
  --api-key "your-openai-key" \
  --output "../crates/parsers/src/new_bk.rs"

# 3. Добавь в ParserFactory
```

## Что генерируется

```rust
// Автоматически сгенерировано AI Parser Generator
// БК: NewBookmaker
// URL: https://example-bk.ru/football
// Дата: 2026-04-09

use crate::base::{BookmakerParser, ParserResult};
// ... полный Rust парсер с headless_chrome ...
```

## Поддерживаемые модели

| Модель | Скорость | Точность | Цена |
|--------|----------|----------|------|
| GPT-4o | Быстро | 95% | $0.005/запрос |
| Claude 3.5 Sonnet | Средне | 97% | $0.003/запрос |
| Ollama (local) | Медленно | 80% | Бесплатно |
| Gemini 2.0 Flash | Очень быстро | 90% | Бесплатно (лимит) |

## Архитектура

```
URL → Playwright → HTML → LLM → CSS Selectors → Rust Code → Parser
```

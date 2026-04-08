# Ghost Imperium - Fork Scanner Skill

## Описание
Специализированный скилл для разработки системы арбитражного беттинга (Ghost Imperium).

## Контекст проекта
- **Букмекеры**: 12 российских БК (Winline, Olimp, Pari, Marathon, etc.)
- **Технологии**: Python 3.11+, FastAPI, asyncio, Playwright, Aiogram
- **Цель**: Поиск вилок в реальном времени, авто-ставки, Telegram уведомления

## Структура кода

### Парсеры букмекеров
Каждый парсер должен наследоваться от `BaseParser`:

```python
# scanner/parsers/{bookmaker}.py
class BookmakerParser(BaseParser):
    name = "bookmaker_slug"
    slug = "bookmaker_slug"
    
    async def get_events(self) -> List[Dict]:
        # Парсинг событий
        pass
```

### API Routes
```python
# api/routes/{module}.py
@router.get("/api/v1/{resource}")
async def get_resource():
    pass
```

### Модели данных
```python
# models/{model}.py
class ModelName(BaseModel):
    field: Type
```

## Конвенции

### Python
- Типизация: `from typing import *`, `from pydantic import BaseModel`
- Async: `async def`, `await asyncio.gather()`
- Логирование: `import structlog`
- Исключения: кастомные классы в `core/exceptions.py`

### Наименования
- Файлы: `snake_case.py`
- Классы: `PascalCase`
- Функции: `snake_case`
- Константы: `UPPER_SNAKE_CASE`

### API Response
```python
{
    "success": bool,
    "data": Any | None,
    "error": str | None
}
```

## Формулы

### Вилка (2-way)
```
S = 1/K1 + 1/K2
Если S < 1 → ВИЛКА
Прибыль = (1/S - 1) × 100%
```

### Ставки
```
Ставка1 = M / K1 / (1/K1 + 1/K2)
Ставка2 = M / K2 / (1/K1 + 1/K2)
```

## Команды

```bash
# Запуск
python main.py

# API
uvicorn api.main:app --reload

# Тесты
pytest tests/ -v
```

## Ресурсы
- AGENTS.md — основная документация
- ARCHITECTURE.md — архитектура системы

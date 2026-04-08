# Ghost Imperium - Agent Rules

## Общие правила
- Все парсеры наследуются от `BaseParser`
- Возвращают `List[Dict]` в стандартизированном формате
- Асинхронность везде где возможно
- Типизация обязательна (pydantic, typing)
- PEP 8, snake_case для функций/переменных

## Структура парсера
```python
class SomeParser(BaseParser):
    name = "BookmakerName"
    slug = "bookmaker_slug"
    base_url = "https://example.com"
    
    async def get_events(self) -> List[Dict]:
        # Реализация парсинга
        pass
```

## Формат события
```python
{
    'id': 'unique_id',
    'bookmaker': 'slug',
    'sport': 'football',
    'home_team': 'Team A',
    'away_team': 'Team B',
    'league': 'League Name',
    'home_odds': 2.10,
    'draw_odds': 3.50,  # или None
    'away_odds': 2.20,
    'is_live': True,
    'market': '1x2',
    'source_url': 'https://...',
    'scraped_at': 1234567890.0
}
```

## Запреты
- ❌ Не использовать `requests` (только `aiohttp`)
- ❌ Не блокировать event loop
- ❌ Не хардкодить URL без fallback
- ❌ Не логировать чувствительные данные

## Обязательности
- ✅ Rate limiting для каждого БК
- ✅ Retry с exponential backoff
- ✅ User-Agent rotation
- ✅ Graceful error handling
- ✅ Кэширование результатов

## Тестирование
- Каждый парсер должен проходить тест с моковыми данными
- Минимум 2 тестовых сценария (успех, ошибка)
- Проверка формата возвращаемых данных

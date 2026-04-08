# core/event_normalizer.py - балансированная нормализация для вилок
import re
from difflib import SequenceMatcher

def normalize_event_name(name: str) -> str:
    """УМНАЯ нормализация - оставляем команды, удаляем мусор"""
    if not name:
        return ""

    text = name.lower().strip()
    text = re.sub(r'\s+', ' ', text)

    # Удаляем только числа и символы, но оставляем слова
    patterns_to_remove = [
        # Числа и символы
        r'\d+',  # все числа
        r'[|/•.,;()\'"]+',  # все символы
        
        # Служебные слова
        r'корзина|история|бонусный\s+клуб|кешбэк',
        r'генератор\s+экспресса|размер\s+коэффициента',
        r'сумма\s+возм\.выигрыша|только\s+топ-события',
        r'добавить\s+исход|обновить\s+список',
        r'популярные\s+события|бонус|акция',
        r'добавить\s+в\s+корзину|подробнее',
        r'ежемесячно|деньгами|избранное',
        r'ближайшие|трансляции|видео',
        r'стрим|прямой\s+эфир|live|лайв',
        r'24/7|secret|медиа|приложения|результаты|статистика',
    ]
    
    for pattern in patterns_to_remove:
        text = re.sub(pattern, ' ', text, flags=re.IGNORECASE)
    
    # Очищаем множественные пробелы
    text = re.sub(r'\s+', ' ', text)
    text = text.strip()
    
    return text

def extract_team_names(name: str) -> tuple:
    """Балансированное извлечение команд"""
    normalized = normalize_event_name(name)
    
    # Ищем разделители команд
    separators = [' - ', ' — ', ' vs ', ' : ', '-', '—', ':', 'vs']
    
    for sep in separators:
        if sep in normalized:
            parts = normalized.split(sep)
            if len(parts) >= 2:
                team1 = parts[0].strip()
                team2 = parts[1].strip()
                
                # Дополнительная очистка
                team1 = re.sub(r'\s+', ' ', team1)
                team2 = re.sub(r'\s+', ' ', team2)
                
                # Удаляем пустые и слишком короткие
                if len(team1) >= 3 and len(team2) >= 3:
                    return team1, team2
    
    return None, None

def are_same_event(name1: str, name2: str, threshold: float = 0.5) -> bool:
    """Балансированная проверка схожести событий"""
    if not name1 or not name2:
        return False
    
    n1 = normalize_event_name(name1)
    n2 = normalize_event_name(name2)
    
    if not n1 or not n2:
        return False
    
    # Точное совпадение
    if n1 == n2:
        return True
    
    # Извлекаем команды
    team1_a, team2_a = extract_team_names(name1)
    team1_b, team2_b = extract_team_names(name2)
    
    # Сравниваем команды
    if team1_a and team1_b and team2_a and team2_b:
        # Проверяем совпадение команд в любом порядке
        match1 = (team1_a == team1_b and team2_a == team2_b)
        match2 = (team1_a == team2_b and team2_a == team1_b)
        
        if match1 or match2:
            return True
        
        # Проверяем частичное совпадение команд
        similarity1 = SequenceMatcher(None, team1_a, team1_b).ratio()
        similarity2 = SequenceMatcher(None, team2_a, team2_b).ratio()
        
        if similarity1 >= 0.85 and similarity2 >= 0.85:
            return True
    
    # Общая схожесть текста
    similarity = SequenceMatcher(None, n1, n2).ratio()
    if similarity >= threshold:
        return True
    
    return False

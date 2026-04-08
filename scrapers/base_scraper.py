# scrapers/base_scraper.py
from abc import ABC, abstractmethod
from typing import List, Dict
from core.event_normalizer import normalize_event_name


class BaseScraper(ABC):
    def __init__(self):
        self.name = "Base"
    
    @abstractmethod
    async def get_events(self) -> List[Dict]:
        pass
    
    def normalize_event(self, event: Dict) -> Dict:
        if 'name' in event:
            event['normalized_name'] = normalize_event_name(event['name'])
        return event

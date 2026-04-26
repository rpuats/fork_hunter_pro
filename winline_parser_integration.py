#!/usr/bin/env python3
"""
WINLINE PARSER - COMPLETE INTEGRATION
Полный парсер для интеграции в систему
- Загружает события из JSON
- Парсит структуру events
- Возвращает 10+ live + 3000 prematch
"""

import json
import logging
import sys
from pathlib import Path
from typing import List, Dict, Optional
from datetime import datetime

if sys.platform == 'win32':
    sys.stdout.reconfigure(encoding='utf-8')

logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s [%(levelname)s] %(message)s'
)
logger = logging.getLogger(__name__)


class WinlineParser:
    """Интегрированный Winline парсер"""
    
    def __init__(self, data_file: str = "winline_events_final.json"):
        self.data_file = Path(data_file)
        self.events = []
    
    def load_from_json(self) -> bool:
        """Загружает события из JSON файла"""
        
        if not self.data_file.exists():
            logger.warning(f"Data file not found: {self.data_file}")
            return False
        
        try:
            with open(self.data_file, 'r', encoding='utf-8') as f:
                data = json.load(f)
            
            if 'events' in data:
                self.events = data['events']
                logger.info(f"✓ Loaded {len(self.events)} events from {self.data_file}")
                return True
            else:
                logger.error("No 'events' key in JSON")
                return False
        
        except Exception as e:
            logger.error(f"Failed to load JSON: {e}")
            return False
    
    def get_live_events(self) -> List[Dict]:
        """Возвращает live события"""
        return [e for e in self.events if e.get('is_live', False)]
    
    def get_prematch_events(self) -> List[Dict]:
        """Возвращает prematch события"""
        return [e for e in self.events if not e.get('is_live', False)]
    
    def validate(self) -> bool:
        """Проверяет что парсер соответствует требованиям"""
        
        live = len(self.get_live_events())
        prematch = len(self.get_prematch_events())
        
        logger.info(f"Live events: {live}")
        logger.info(f"Prematch events: {prematch}")
        
        success = live >= 10 and prematch >= 3000
        
        if success:
            logger.info(f"✓ Validation PASSED!")
        else:
            logger.warning(f"✗ Validation FAILED!")
            if live < 10:
                logger.warning(f"  - Live events: {live} < 10")
            if prematch < 3000:
                logger.warning(f"  - Prematch events: {prematch} < 3000")
        
        return success
    
    def export_for_integration(self, output_file: str = "winline_export.json") -> bool:
        """Экспортирует события для интеграции в систему"""
        
        export_data = {
            "bookmaker": "winline",
            "timestamp": datetime.now().isoformat(),
            "stats": {
                "total": len(self.events),
                "live": len(self.get_live_events()),
                "prematch": len(self.get_prematch_events()),
            },
            "events": self.events
        }
        
        try:
            with open(output_file, 'w', encoding='utf-8') as f:
                json.dump(export_data, f, ensure_ascii=False, indent=2)
            
            logger.info(f"✓ Exported to {output_file}")
            return True
        except Exception as e:
            logger.error(f"Export failed: {e}")
            return False
    
    def print_summary(self):
        """Выводит итоги"""
        
        live = self.get_live_events()
        prematch = self.get_prematch_events()
        
        print("\n" + "=" * 60)
        print("WINLINE PARSER - INTEGRATION STATUS")
        print("=" * 60)
        print(f"Total events: {len(self.events)}")
        print(f"Live events: {len(live)}")
        print(f"Prematch events: {len(prematch)}")
        
        if live:
            print(f"\nLive matches ({len(live)}):")
            for event in live[:5]:
                print(f"  • {event['home_team']} vs {event['away_team']} ({event['league']})")
        
        if prematch:
            print(f"\nPrematch matches ({len(prematch)} total):")
            for event in prematch[:5]:
                start = event['start_time'][:10] if isinstance(event['start_time'], str) else 'N/A'
                print(f"  • {event['home_team']} vs {event['away_team']} ({event['league']}) - {start}")
        
        print("=" * 60)


def main():
    parser = WinlineParser()
    
    # Load data
    if not parser.load_from_json():
        logger.error("Failed to load data")
        return 1
    
    # Validate
    if not parser.validate():
        logger.error("Validation failed")
        return 1
    
    # Export
    parser.export_for_integration()
    
    # Show summary
    parser.print_summary()
    
    return 0


if __name__ == "__main__":
    sys.exit(main())

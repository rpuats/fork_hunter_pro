"""
Winline WebSocket Parser - подключается к wss://wss.winline.ru и получает события
"""
import asyncio
import json
import websockets
import sys

sys.stdout.reconfigure(encoding='utf-8')

async def connect_to_winline_ws():
    """Подключается к WebSocket Winline и собирает события"""
    
    ws_url = "wss://wss.winline.ru/data_ng?client=newsite&nb=true"
    
    print(f"Connecting to {ws_url}...")
    
    events_count = 0
    live_count = 0
    prematch_count = 0
    
    try:
        async with websockets.connect(ws_url, origin="https://winline.ru") as ws:
            print("✓ Connected to WebSocket!")
            
            # WebSocket может требовать инициализации
            # Отправляем команды чтобы запросить события
            
            init_commands = [
                {"action": "get_sports"},
                {"action": "get_events"},
                {"sport": 5},  # Football
            ]
            
            for cmd in init_commands:
                try:
                    await ws.send(json.dumps(cmd))
                    print(f"Sent: {cmd}")
                except Exception as e:
                    print(f"Error sending: {e}")
            
            # Ловим сообщения в течение 30 секунд
            print("\nWaiting for messages (30 seconds)...")
            
            start_time = asyncio.get_event_loop().time()
            timeout = 30
            
            while asyncio.get_event_loop().time() - start_time < timeout:
                try:
                    # Получаем сообщение с таймаутом 5 секунд
                    message = await asyncio.wait_for(ws.recv(), timeout=5)
                    
                    # Пытаемся спарсить как JSON
                    try:
                        data = json.loads(message)
                        
                        # Проверяем на события
                        if isinstance(data, dict):
                            if 'events' in data:
                                events = data['events']
                                if isinstance(events, list):
                                    events_count += len(events)
                                    live_count += sum(1 for e in events if e.get('is_live'))
                                    prematch_count += sum(1 for e in events if not e.get('is_live'))
                                    print(f"  Received {len(events)} events (Live: {sum(1 for e in events if e.get('is_live'))}, Prematch: {sum(1 for e in events if not e.get('is_live'))})")
                            
                            elif any(k in data for k in ['event', 'sport', 'markets', 'odds']):
                                print(f"  Data keys: {list(data.keys())[:10]}")
                            
                            else:
                                # Show only first message structure
                                if events_count == 0:
                                    print(f"  Message structure: {list(data.keys())[:20]}")
                    
                    except json.JSONDecodeError:
                        # Not JSON, might be binary or text
                        if len(message) > 100:
                            print(f"  Non-JSON message: {len(message)} bytes")
                        else:
                            print(f"  Message: {message}")
                
                except asyncio.TimeoutError:
                    # 5-second timeout between messages - this is normal
                    print(".", end="", flush=True)
                    continue
                except Exception as e:
                    print(f"Error: {e}")
                    break
            
            print(f"\n\n=== RESULTS ===")
            print(f"Total events received: {events_count}")
            print(f"Live events: {live_count}")
            print(f"Prematch events: {prematch_count}")
            
            if events_count > 0:
                print(f"\n✓ SUCCESS! WebSocket returned {events_count} events!")
                return True
    
    except websockets.exceptions.WebSocketException as e:
        print(f"WebSocket error: {e}")
    except Exception as e:
        print(f"Error: {e}")
        import traceback
        traceback.print_exc()
    
    return False

if __name__ == '__main__':
    try:
        result = asyncio.run(connect_to_winline_ws())
        if result:
            print("\n SUCCESS - WebSocket works!")
        else:
            print("\nWebSocket connection failed or no events received")
    except KeyboardInterrupt:
        print("\nInterrupted by user")

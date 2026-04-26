import asyncio, websockets, sys

URL = 'wss://ru-ws.sporthub.bet:444/api/tree_ws/v1'

async def main():
    try:
        async with websockets.connect(URL, subprotocols=['protobuf'], origin='https://betboom.ru', additional_headers={'User-Agent': 'Mozilla/5.0'}) as ws:
            for i in range(3):
                try:
                    msg = await asyncio.wait_for(ws.recv(), timeout=5)
                    if isinstance(msg, bytes):
                        print(f'BYTES len={len(msg)} hex={msg[:80].hex()}')
                    else:
                        print(f'TEXT {msg[:200]}')
                except asyncio.TimeoutError:
                    print('TIMEOUT')
                    break
    except Exception as e:
        print('ERR', repr(e))

asyncio.run(main())

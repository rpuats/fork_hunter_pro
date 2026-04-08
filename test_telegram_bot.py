import sys
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from bot import handlers as bot_handlers

print('TELEGRAM BOT CHECK:')
print(f'  Bot handlers module: {"OK" if bot_handlers else "FAIL"}')
print(f'  register_handlers: {"OK" if hasattr(bot_handlers, "register_handlers") else "FAIL"}')
print(f'  dp_instance: {"OK" if hasattr(bot_handlers, "dp_instance") else "FAIL"}')
print(f'  cmd_start: {"OK" if hasattr(bot_handlers, "cmd_start") else "FAIL"}')
print(f'  cmd_surebets: {"OK" if hasattr(bot_handlers, "cmd_surebets") else "FAIL"}')
print(f'  inline_search: {"OK" if hasattr(bot_handlers, "inline_search") else "FAIL"}')
print(f'  callback_calculate: {"OK" if hasattr(bot_handlers, "callback_calculate") else "FAIL"}')
print(f'  send_notification: {"OK" if hasattr(bot_handlers, "send_notification") else "FAIL"}')

# Count handlers
handler_funcs = [attr for attr in dir(bot_handlers) if attr.startswith(('cmd_', 'callback_', 'handle_', 'inline_', 'send_'))]
print(f'  Total handler functions: {len(handler_funcs)}')

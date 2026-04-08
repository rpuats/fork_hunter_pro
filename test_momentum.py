import sys
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from core.momentum_scanner import MomentumScanner, MomentumTrigger, MomentumTriggerType

print('MOMENTUM SCANNER CHECK:')
print(f'  MomentumScanner class: {"OK" if MomentumScanner else "FAIL"}')
print(f'  MomentumTrigger class: {"OK" if MomentumTrigger else "FAIL"}')
print(f'  MomentumTriggerType enum: {"OK" if MomentumTriggerType else "FAIL"}')

triggers = ['GOAL', 'RED_CARD', 'PENALTY', 'ODDS_SPIKE']
for t in triggers:
    status = "DEFINED" if t in dir(MomentumTriggerType) else "MISSING"
    print(f'  Trigger {t}: {status}')

print()
print('WINDOW DURATIONS:')
for t in MomentumTriggerType:
    temp = MomentumTrigger(trigger_type=t, match_key="test|test")
    print(f'  {t.value}: {temp.window_duration}s (priority: {temp.priority})')

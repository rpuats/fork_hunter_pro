// Demo data for Ghost Imperium Pro UI
// Used when real data is not available

export const demoAccounts = [
  {
    id: '1',
    name: 'Pari',
    logo: 'https://via.placeholder.com/40/4F46E5/FFFFFF?text=P',
    balance: 45000,
    currency: '₽',
    locked: 5000,
    available: 40000,
    connectionStatus: 'connected' as const,
    lastSync: new Date().toISOString(),
    riskLevel: 'low' as const,
    riskFactors: [],
    maxBet: 25000,
    minBet: 100,
    dailyLimit: 50000,
    usedToday: 15000,
    totalBets: 124,
    wonBets: 89,
    lostBets: 35,
    totalProfit: 12750,
    lastBet: new Date(Date.now() - 3600000).toISOString(),
    bonuses: [
      { id: 'b1', type: 'freebet' as const, name: 'Фрибет 500₽', amount: 500, wagering: 1, expiresAt: new Date(Date.now() + 86400000).toISOString(), isActive: true }
    ],
    autoBetEnabled: true,
    maxBetAmount: 10000,
    notifications: true,
    twoFactorEnabled: false,
    ipWhitelist: [],
    betsPerHourLimit: 10,
    pauseAfterLoss: 5
  },
  {
    id: '2',
    name: 'Fonbet',
    logo: 'https://via.placeholder.com/40/10B981/FFFFFF?text=F',
    balance: 32000,
    currency: '₽',
    locked: 8000,
    available: 24000,
    connectionStatus: 'connected' as const,
    lastSync: new Date().toISOString(),
    riskLevel: 'medium' as const,
    riskFactors: [
      { type: 'frequent_wins' as const, severity: 'low' as const, description: 'Частые выигрыши', recommendation: 'Снизить размер ставок', detectedAt: new Date(Date.now() - 86400000).toISOString() }
    ],
    maxBet: 30000,
    minBet: 200,
    dailyLimit: 60000,
    usedToday: 22000,
    totalBets: 98,
    wonBets: 62,
    lostBets: 36,
    totalProfit: 8450,
    lastBet: new Date(Date.now() - 7200000).toISOString(),
    bonuses: [],
    autoBetEnabled: false,
    maxBetAmount: 5000,
    notifications: true,
    twoFactorEnabled: true,
    ipWhitelist: ['192.168.1.1'],
    betsPerHourLimit: 8,
    pauseAfterLoss: 10
  },
  {
    id: '3',
    name: 'Leon',
    logo: 'https://via.placeholder.com/40/F59E0B/FFFFFF?text=L',
    balance: 18000,
    currency: '₽',
    locked: 2000,
    available: 16000,
    connectionStatus: 'error' as const,
    lastSync: new Date(Date.now() - 3600000).toISOString(),
    riskLevel: 'high' as const,
    riskFactors: [
      { type: 'rapid_bets' as const, severity: 'high' as const, description: 'Слишком быстрые ставки', recommendation: 'Увеличить паузу между ставками', detectedAt: new Date(Date.now() - 172800000).toISOString() }
    ],
    maxBet: 15000,
    minBet: 500,
    dailyLimit: 40000,
    usedToday: 35000,
    totalBets: 76,
    wonBets: 45,
    lostBets: 31,
    totalProfit: -2300,
    lastBet: new Date(Date.now() - 18000000).toISOString(),
    bonuses: [
      { id: 'b2', type: 'cashback' as const, name: 'Кэшбэк 5%', amount: 1200, wagering: 3, expiresAt: new Date(Date.now() + 172800000).toISOString(), isActive: true }
    ],
    autoBetEnabled: false,
    maxBetAmount: 2000,
    notifications: false,
    twoFactorEnabled: false,
    ipWhitelist: [],
    betsPerHourLimit: 5,
    pauseAfterLoss: 15
  }
];

export const demoForks = [
  {
    id: 'f1',
    match: 'ЦСКА — Спартак',
    league: 'РПЛ',
    sport: 'Футбол',
    market: 'Победитель',
    profit: 5.2,
    bonus: false,
    isHot: true,
    timeLeft: 1800,
    startTime: new Date(Date.now() + 7200000).toISOString(),
    totalStake: 10000,
    bookmakers: [
      { name: 'Pari', outcome: 'П1', odds: 2.45, stake: 5102 },
      { name: 'Fonbet', outcome: 'П2', odds: 2.38, stake: 4898 }
    ]
  },
  {
    id: 'f2',
    match: 'Локомотив — Зенит',
    league: 'РПЛ',
    sport: 'Футбол',
    market: 'Тотал больше 2.5',
    profit: 3.8,
    bonus: true,
    isHot: false,
    timeLeft: 3600,
    startTime: new Date(Date.now() + 10800000).toISOString(),
    totalStake: 8000,
    bookmakers: [
      { name: 'Pari', outcome: 'ТБ 2.5', odds: 1.85, stake: 4189 },
      { name: 'Leon', outcome: 'ТМ 2.5', odds: 1.95, stake: 3811 }
    ]
  },
  {
    id: 'f3',
    match: 'Реал Мадрид — Барселона',
    league: 'Ла Лига',
    sport: 'Футбол',
    market: 'Фора +1.5',
    profit: 7.1,
    bonus: false,
    isHot: true,
    timeLeft: 600,
    startTime: new Date(Date.now() + 1800000).toISOString(),
    totalStake: 15000,
    bookmakers: [
      { name: 'Fonbet', outcome: 'Ф1 +1.5', odds: 1.72, stake: 8920 },
      { name: 'Leon', outcome: 'Ф2 -1.5', odds: 2.10, stake: 6080 }
    ]
  },
  {
    id: 'f4',
    match: 'Манчестер Сити — Ливерпуль',
    league: 'АПЛ',
    sport: 'Футбол',
    market: 'Обе забьют',
    profit: 2.4,
    bonus: false,
    isHot: false,
    timeLeft: 5400,
    startTime: new Date(Date.now() + 18000000).toISOString(),
    totalStake: 5000,
    bookmakers: [
      { name: 'Pari', outcome: 'Да', odds: 1.65, stake: 3125 },
      { name: 'Leon', outcome: 'Нет', odds: 2.40, stake: 1875 }
    ]
  }
];

export const demoNotifications = [
  { id: 'n1', type: 'success' as const, message: 'Вилка найдена: ЦСКА — Спартак 5.2%', time: new Date(Date.now() - 300000).toISOString(), read: false },
  { id: 'n2', type: 'warning' as const, message: 'Сессия Leon истекает через 1 час', time: new Date(Date.now() - 1800000).toISOString(), read: false },
  { id: 'n3', type: 'info' as const, message: 'Баланс Fonbet обновлен: +2,450 ₽', time: new Date(Date.now() - 3600000).toISOString(), read: true },
  { id: 'n4', type: 'error' as const, message: 'Ошибка подключения к Бетсити', time: new Date(Date.now() - 7200000).toISOString(), read: true }
];

export const demoProfitChart = {
  labels: ['Пн', 'Вт', 'Ср', 'Чт', 'Пт', 'Сб', 'Вс'],
  data: [1200, 1800, 950, 2100, 1500, 3200, 2450]
};

export const demoActivityLog = [
  { id: 'a1', action: 'Ставка размещена', detail: 'ЦСКА — Спартак @ Pari', time: new Date(Date.now() - 300000).toISOString(), type: 'bet' as const },
  { id: 'a2', action: 'Вилка найдена', detail: 'Реал Мадрид — Барселона 7.1%', time: new Date(Date.now() - 600000).toISOString(), type: 'fork' as const },
  { id: 'a3', action: 'Баланс обновлен', detail: 'Fonbet: 32,450 ₽', time: new Date(Date.now() - 900000).toISOString(), type: 'balance' as const },
  { id: 'a4', action: 'Ставка выиграна', detail: 'Зенит — Локомотив +1,200 ₽', time: new Date(Date.now() - 1800000).toISOString(), type: 'win' as const },
  { id: 'a5', action: 'Ошибка', detail: 'Таймаут Leon API', time: new Date(Date.now() - 2700000).toISOString(), type: 'error' as const }
];

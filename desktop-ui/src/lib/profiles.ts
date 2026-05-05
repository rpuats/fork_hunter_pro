// Profile system for Ghost Imperium Pro
// Each bookmaker can have multiple profiles (accounts/drops)

export interface BrowserFingerprint {
  userAgent: string
  screenResolution: string
  colorDepth: number
  timezone: string
  language: string
  platform: string
  cpuCores: number
  memory: number  // GB
  touchSupport: boolean
}

export interface AccountProfile {
  id: string
  name: string  // e.g. "Drop #1", "Main", "Backup"
  bookmaker: string  // Pari, Fonbet, etc.
  
  // Auth data (encrypted in real app)
  cookies?: string
  token?: string
  sessionData?: Record<string, any>
  
  // Browser fingerprint for this profile
  fingerprint: BrowserFingerprint
  
  // Proxy settings
  proxy?: {
    host: string
    port: number
    username?: string
    password?: string
    type: 'http' | 'socks5'
  }
  
  // Status
  isActive: boolean
  lastUsed: string | null
  createdAt: string
  
  // Profile color/icon for quick identification
  color: string
  icon?: string
}

export interface BookmakerProfileGroup {
  bookmaker: string
  logo?: string
  profiles: AccountProfile[]
  activeProfileId: string | null
}

// Generate unique fingerprint for each profile
export function generateFingerprint(): BrowserFingerprint {
  const resolutions = ['1920x1080', '2560x1440', '1366x768', '1440x900', '1536x864']
  const timezones = ['Europe/Moscow', 'Europe/Kiev', 'Europe/Minsk', 'Asia/Almaty', 'Europe/Warsaw']
  const languages = ['ru-RU', 'ru,en;q=0.9', 'en-US', 'uk-UA']
  const platforms = ['Win32', 'MacIntel', 'Linux x86_64']
  
  return {
    userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
    screenResolution: resolutions[Math.floor(Math.random() * resolutions.length)],
    colorDepth: [24, 32][Math.floor(Math.random() * 2)],
    timezone: timezones[Math.floor(Math.random() * timezones.length)],
    language: languages[Math.floor(Math.random() * languages.length)],
    platform: platforms[Math.floor(Math.random() * platforms.length)],
    cpuCores: [4, 6, 8, 12, 16][Math.floor(Math.random() * 5)],
    memory: [4, 8, 16, 32][Math.floor(Math.random() * 4)],
    touchSupport: false
  }
}

// Demo profiles data
export const demoProfiles: BookmakerProfileGroup[] = [
  {
    bookmaker: 'Pari',
    logo: 'P',
    activeProfileId: 'pari-1',
    profiles: [
      {
        id: 'pari-1',
        name: 'Основной',
        bookmaker: 'Pari',
        fingerprint: generateFingerprint(),
        isActive: true,
        lastUsed: new Date().toISOString(),
        createdAt: new Date(Date.now() - 86400000 * 30).toISOString(),
        color: '#4F46E5',
        proxy: {
          host: '185.123.45.67',
          port: 8080,
          type: 'http'
        }
      },
      {
        id: 'pari-2',
        name: 'Дроп #2',
        bookmaker: 'Pari',
        fingerprint: generateFingerprint(),
        isActive: false,
        lastUsed: new Date(Date.now() - 86400000 * 2).toISOString(),
        createdAt: new Date(Date.now() - 86400000 * 15).toISOString(),
        color: '#10B981',
        proxy: {
          host: '185.123.45.68',
          port: 8080,
          type: 'http'
        }
      },
      {
        id: 'pari-3',
        name: 'Резерв',
        bookmaker: 'Pari',
        fingerprint: generateFingerprint(),
        isActive: false,
        lastUsed: null,
        createdAt: new Date(Date.now() - 86400000 * 5).toISOString(),
        color: '#F59E0B'
      }
    ]
  },
  {
    bookmaker: 'Fonbet',
    logo: 'F',
    activeProfileId: 'fonbet-1',
    profiles: [
      {
        id: 'fonbet-1',
        name: 'Основной',
        bookmaker: 'Fonbet',
        fingerprint: generateFingerprint(),
        isActive: true,
        lastUsed: new Date().toISOString(),
        createdAt: new Date(Date.now() - 86400000 * 45).toISOString(),
        color: '#10B981',
        proxy: {
          host: '46.23.12.89',
          port: 3128,
          type: 'socks5'
        }
      },
      {
        id: 'fonbet-2',
        name: 'Дроп #2',
        bookmaker: 'Fonbet',
        fingerprint: generateFingerprint(),
        isActive: false,
        lastUsed: new Date(Date.now() - 86400000).toISOString(),
        createdAt: new Date(Date.now() - 86400000 * 20).toISOString(),
        color: '#EF4444'
      }
    ]
  },
  {
    bookmaker: 'Leon',
    logo: 'L',
    activeProfileId: 'leon-1',
    profiles: [
      {
        id: 'leon-1',
        name: 'Основной',
        bookmaker: 'Leon',
        fingerprint: generateFingerprint(),
        isActive: true,
        lastUsed: new Date(Date.now() - 3600000).toISOString(),
        createdAt: new Date(Date.now() - 86400000 * 60).toISOString(),
        color: '#F59E0B'
      }
    ]
  },
  {
    bookmaker: 'Winline',
    logo: 'W',
    activeProfileId: null,
    profiles: []
  },
  {
    bookmaker: 'Olimp',
    logo: 'O',
    activeProfileId: null,
    profiles: []
  }
]

// Profile actions
export function switchProfile(group: BookmakerProfileGroup, profileId: string): BookmakerProfileGroup {
  return {
    ...group,
    activeProfileId: profileId,
    profiles: group.profiles.map(p => ({
      ...p,
      isActive: p.id === profileId,
      lastUsed: p.id === profileId ? new Date().toISOString() : p.lastUsed
    }))
  }
}

export function createProfile(group: BookmakerProfileGroup, name: string): BookmakerProfileGroup {
  const newProfile: AccountProfile = {
    id: `${group.bookmaker.toLowerCase()}-${Date.now()}`,
    name,
    bookmaker: group.bookmaker,
    fingerprint: generateFingerprint(),
    isActive: false,
    lastUsed: null,
    createdAt: new Date().toISOString(),
    color: ['#4F46E5', '#10B981', '#F59E0B', '#EF4444', '#8B5CF6'][Math.floor(Math.random() * 5)]
  }
  
  return {
    ...group,
    profiles: [...group.profiles, newProfile]
  }
}

export function deleteProfile(group: BookmakerProfileGroup, profileId: string): BookmakerProfileGroup {
  const filtered = group.profiles.filter(p => p.id !== profileId)
  return {
    ...group,
    activeProfileId: group.activeProfileId === profileId 
      ? (filtered[0]?.id || null) 
      : group.activeProfileId,
    profiles: filtered
  }
}

export function getActiveProfile(group: BookmakerProfileGroup): AccountProfile | null {
  return group.profiles.find(p => p.id === group.activeProfileId) || null
}

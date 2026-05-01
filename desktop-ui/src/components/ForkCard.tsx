import { useState } from 'react';
import { motion } from 'framer-motion';
import { 
  TrendingUp, 
  Clock, 
  Target, 
  Shield, 
  AlertCircle,
  Star,
  EyeOff,
  Play
} from 'lucide-react';

// Types matching the Rust backend
export interface ForkLeg {
  bookmaker_slug: string;
  market: string;
  selection: string;
  odds: number;
  event_id: string;
  original_event_id: string;
}

export type ForkType = 
  | 'match_winner_1x2' 
  | 'match_winner_12' 
  | 'total_over_under' 
  | 'handicap' 
  | 'btts' 
  | 'corridor';

export interface Fork {
  id: string;
  event_id: string;
  home_team: string;
  away_team: string;
  league: string;
  sport: string;
  is_live: boolean;
  start_time?: string;
  profit_percent: number;
  legs: ForkLeg[];
  fork_type: ForkType;
  created_at: string;
  age_ms: number;
}

interface ForkCardProps {
  fork: Fork;
  isSelected: boolean;
  isNew: boolean;
  onClick: () => void;
  onExecute: () => void;
  onAddToFavorites: () => void;
  onHide: () => void;
}

const BOOKMAKER_NAMES: Record<string, string> = {
  pari: 'Пари',
  fonbet: 'Фонбет',
  marathon: 'Марафон',
  betcity: 'Бетсити',
  zenit: 'Зенит',
  baltbet: 'Балтбет',
  bettery: 'Беттери',
  leon: 'Леон',
  sportbet: 'Спортбет',
  bet24: '24bet',
  olimp: 'Олимп',
  winline: 'Винлайн',
};

const SPORT_ICONS: Record<string, string> = {
  football: '⚽',
  tennis: '🎾',
  basketball: '🏀',
  hockey: '🏒',
  volleyball: '🏐',
  esports: '🎮',
};

function getSportIcon(sport: string): string {
  return SPORT_ICONS[sport.toLowerCase()] || '🏆';
}

function getBookmakerName(slug: string): string {
  return BOOKMAKER_NAMES[slug.toLowerCase()] || slug;
}

function formatAge(ageMs: number): string {
  if (ageMs < 1000) return 'now';
  if (ageMs < 60000) return `${Math.floor(ageMs / 1000)}s`;
  return `${Math.floor(ageMs / 60000)}m`;
}

function formatProfit(profit: number): string {
  return `${profit > 0 ? '+' : ''}${profit.toFixed(2)}%`;
}

function getProfitColor(profit: number): string {
  if (profit > 2.0) return 'super-profit';
  if (profit > 1.0) return 'high-profit';
  if (profit > 0.5) return 'normal-profit';
  if (profit > 0) return 'low-profit';
  return 'negative-profit';
}

function getForkTypeName(type: ForkType): string {
  const names: Record<ForkType, string> = {
    match_winner_1x2: '1X2',
    match_winner_12: '12',
    total_over_under: 'Тотал',
    handicap: 'Фора',
    btts: 'Обе забьют',
    corridor: 'Коридор',
  };
  return names[type] || type;
}

export function ForkCard({
  fork,
  isSelected,
  isNew,
  onClick,
  onExecute,
  onAddToFavorites,
  onHide,
}: ForkCardProps) {
  const [isHovered, setIsHovered] = useState(false);

  const profitClass = getProfitColor(fork.profit_percent);

  return (
    <motion.div
      layout
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, scale: 0.95 }}
      className={`fork-card ${isSelected ? 'selected' : ''} ${isNew ? 'new' : ''} ${profitClass}`}
      onClick={onClick}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
    >
      {/* New badge */}
      {isNew && (
        <div className="new-badge">
          <span className="badge-text">NEW</span>
        </div>
      )}

      {/* Live indicator */}
      {fork.is_live && (
        <div className="live-indicator">
          <span className="live-dot" />
          <span className="live-text">LIVE</span>
        </div>
      )}

      {/* Header */}
      <div className="fork-header">
        <div className="sport-info">
          <span className="sport-icon">{getSportIcon(fork.sport)}</span>
          <span className="league">{fork.league}</span>
        </div>
        <div className="fork-meta">
          <span className="fork-type">{getForkTypeName(fork.fork_type)}</span>
          <span className="age">
            <Clock size={12} />
            {formatAge(fork.age_ms)}
          </span>
        </div>
      </div>

      {/* Teams */}
      <div className="fork-teams">
        <div className="team home">
          <span className="team-name">{fork.home_team}</span>
        </div>
        <span className="vs">vs</span>
        <div className="team away">
          <span className="team-name">{fork.away_team}</span>
        </div>
      </div>

      {/* Legs */}
      <div className="fork-legs">
        {fork.legs.map((leg, idx) => (
          <div key={idx} className="fork-leg">
            <div className="leg-bookmaker">
              <img 
                src={`/icons/bk/${leg.bookmaker_slug}.png`} 
                alt={getBookmakerName(leg.bookmaker_slug)}
                className="bk-logo"
                onError={(e) => {
                  (e.target as HTMLImageElement).style.display = 'none';
                }}
              />
              <span className="bk-name">{getBookmakerName(leg.bookmaker_slug)}</span>
            </div>
            <div className="leg-selection">
              <span className="market">{leg.market}</span>
              <span className="selection">{leg.selection}</span>
            </div>
            <div className="leg-odds">
              <span className="odds-value">{leg.odds.toFixed(2)}</span>
            </div>
          </div>
        ))}
      </div>

      {/* Profit section */}
      <div className="fork-profit">
        <div className="profit-label">
          <TrendingUp size={14} />
          <span>Прибыль:</span>
        </div>
        <div className={`profit-value ${profitClass}`}>
          {formatProfit(fork.profit_percent)}
        </div>
        <div className="profit-estimate">
          ~{Math.round(fork.profit_percent * 100).toLocaleString()} ₽ на 10к
        </div>
      </div>

      {/* Actions */}
      <div className={`fork-actions ${isHovered ? 'visible' : ''}`}>
        <button
          className="btn-execute"
          onClick={(e) => {
            e.stopPropagation();
            onExecute();
          }}
        >
          <Play size={14} />
          Ставить
        </button>
        <button
          className="btn-fav"
          onClick={(e) => {
            e.stopPropagation();
            onAddToFavorites();
          }}
        >
          <Star size={14} />
        </button>
        <button
          className="btn-hide"
          onClick={(e) => {
            e.stopPropagation();
            onHide();
          }}
        >
          <EyeOff size={14} />
        </button>
      </div>

      {/* Verification badge */}
      {fork.profit_percent > 1.0 && (
        <div className="verification-badge">
          <Shield size={12} />
          <span>Верифицировано</span>
        </div>
      )}

      {/* Warnings */}
      {fork.profit_percent > 5.0 && (
        <div className="warning-banner">
          <AlertCircle size={12} />
          <span>Высокая прибыль - проверьте лимиты БК</span>
        </div>
      )}
    </motion.div>
  );
}

export default ForkCard;

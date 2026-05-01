import { useState, useMemo } from 'react';
import { Calculator, TrendingUp, AlertCircle, Wallet } from 'lucide-react';
import type { Fork } from './ForkCard';

export type StakingStrategy = 
  | 'equal_profit' 
  | 'proportional' 
  | 'fixed_amount' 
  | 'kelly' 
  | 'flat_percent';

interface Account {
  id: string;
  bookmaker_slug: string;
  login: string;
  balance: number;
  status: 'Authenticated' | 'Pending' | 'Error';
}

interface StakingCalculatorProps {
  fork: Fork;
  accounts: Account[];
  strategy: StakingStrategy;
  onStrategyChange: (strategy: StakingStrategy) => void;
  onExecute: (stakes: StakePlan) => void;
}

export interface LegStake {
  leg_index: number;
  bookmaker_slug: string;
  stake: number;
  profit_if_wins: number;
  roi_percent: number;
}

export interface StakePlan {
  total_stake: number;
  stakes: LegStake[];
  guaranteed_profit: number;
  roi_percent: number;
}

const STRATEGY_NAMES: Record<StakingStrategy, string> = {
  equal_profit: 'Равная прибыль',
  proportional: 'Пропорционально',
  fixed_amount: 'Фиксированная сумма',
  kelly: 'Критерий Келли',
  flat_percent: 'Фиксированный % банка',
};

function calculateEqualProfitStakes(fork: Fork, totalStake: number): StakePlan {
  const odds = fork.legs.map(l => l.odds);
  const sumInverses = odds.reduce((sum, o) => sum + 1 / o, 0);
  
  const stakes = odds.map((o, i) => {
    const stake = totalStake * (1 / o) / sumInverses;
    const profitIfWins = stake * o - totalStake;
    return {
      leg_index: i,
      bookmaker_slug: fork.legs[i].bookmaker_slug,
      stake,
      profit_if_wins: profitIfWins,
      roi_percent: (profitIfWins / stake) * 100,
    };
  });
  
  const guaranteedProfit = stakes[0].profit_if_wins;
  
  return {
    total_stake: totalStake,
    stakes,
    guaranteed_profit: guaranteedProfit,
    roi_percent: (guaranteedProfit / totalStake) * 100,
  };
}

function calculateProportionalStakes(fork: Fork, totalStake: number): StakePlan {
  const odds = fork.legs.map(l => l.odds);
  const sumOdds = odds.reduce((sum, o) => sum + o, 0);
  
  const stakes = odds.map((o, i) => {
    const stake = totalStake * o / sumOdds;
    const profitIfWins = stake * o - totalStake;
    return {
      leg_index: i,
      bookmaker_slug: fork.legs[i].bookmaker_slug,
      stake,
      profit_if_wins: profitIfWins,
      roi_percent: (profitIfWins / stake) * 100,
    };
  });
  
  // For proportional, profits are different, take average
  const avgProfit = stakes.reduce((sum, s) => sum + s.profit_if_wins, 0) / stakes.length;
  
  return {
    total_stake: totalStake,
    stakes,
    guaranteed_profit: avgProfit,
    roi_percent: (avgProfit / totalStake) * 100,
  };
}

function calculateFixedStakes(fork: Fork, fixedAmount: number): StakePlan {
  const stakes = fork.legs.map((leg, i) => {
    const profitIfWins = fixedAmount * leg.odds - fixedAmount * fork.legs.length;
    return {
      leg_index: i,
      bookmaker_slug: leg.bookmaker_slug,
      stake: fixedAmount,
      profit_if_wins: profitIfWins,
      roi_percent: (profitIfWins / fixedAmount) * 100,
    };
  });
  
  const totalStake = fixedAmount * fork.legs.length;
  const avgProfit = stakes.reduce((sum, s) => sum + s.profit_if_wins, 0) / stakes.length;
  
  return {
    total_stake: totalStake,
    stakes,
    guaranteed_profit: avgProfit,
    roi_percent: (avgProfit / totalStake) * 100,
  };
}

function calculateKellyStakes(
  fork: Fork, 
  bankroll: number, 
  probabilities: number[],
  fraction: number = 0.25
): StakePlan {
  const stakes = fork.legs.map((leg, i) => {
    const b = leg.odds - 1;
    const p = probabilities[i] || 0.5;
    const q = 1 - p;
    const kelly = (b * p - q) / b;
    const kellyCapped = Math.max(0, Math.min(kelly, 0.25));
    const stake = bankroll * kellyCapped * fraction;
    const profitIfWins = stake * leg.odds - bankroll;
    
    return {
      leg_index: i,
      bookmaker_slug: leg.bookmaker_slug,
      stake,
      profit_if_wins: profitIfWins,
      roi_percent: (profitIfWins / stake) * 100,
    };
  });
  
  const totalStake = stakes.reduce((sum, s) => sum + s.stake, 0);
  const avgProfit = stakes.reduce((sum, s) => sum + s.profit_if_wins, 0) / stakes.length;
  
  return {
    total_stake: totalStake,
    stakes,
    guaranteed_profit: avgProfit,
    roi_percent: (avgProfit / totalStake) * 100,
  };
}

function calculateFlatPercent(fork: Fork, bankroll: number, percent: number): StakePlan {
  const stakePerLeg = bankroll * percent;
  return calculateFixedStakes(fork, stakePerLeg);
}

export function StakingCalculator({
  fork,
  accounts,
  strategy,
  onStrategyChange,
  onExecute,
}: StakingCalculatorProps) {
  const [totalBankroll, setTotalBankroll] = useState(100000);
  const [fixedAmount, setFixedAmount] = useState(5000);
  const [kellyFraction, setKellyFraction] = useState(0.25);
  const [flatPercent, setFlatPercent] = useState(0.01);
  const [probabilities, setProbabilities] = useState<number[]>(
    fork.legs.map(() => 0.5)
  );

  const stakePlan = useMemo<StakePlan>(() => {
    switch (strategy) {
      case 'equal_profit':
        return calculateEqualProfitStakes(fork, totalBankroll);
      case 'proportional':
        return calculateProportionalStakes(fork, totalBankroll);
      case 'fixed_amount':
        return calculateFixedStakes(fork, fixedAmount);
      case 'kelly':
        return calculateKellyStakes(fork, totalBankroll, probabilities, kellyFraction);
      case 'flat_percent':
        return calculateFlatPercent(fork, totalBankroll, flatPercent);
      default:
        return calculateEqualProfitStakes(fork, totalBankroll);
    }
  }, [fork, strategy, totalBankroll, fixedAmount, probabilities, kellyFraction, flatPercent]);

  const hasInsufficientFunds = stakePlan.stakes.some(stake => {
    const account = accounts.find(a => 
      a.bookmaker_slug === stake.bookmaker_slug && 
      a.status === 'Authenticated'
    );
    return !account || account.balance < stake.stake;
  });

  const formatMoney = (amount: number) => {
    return `${Math.round(amount).toLocaleString('ru-RU')} ₽`;
  };

  return (
    <div className="staking-calculator">
      <div className="calc-header">
        <Calculator size={18} />
        <h4>Калькулятор ставок</h4>
        <select 
          value={strategy} 
          onChange={(e) => onStrategyChange(e.target.value as StakingStrategy)}
          className="strategy-select"
        >
          {Object.entries(STRATEGY_NAMES).map(([key, name]) => (
            <option key={key} value={key}>{name}</option>
          ))}
        </select>
      </div>

      {/* Strategy-specific inputs */}
      <div className="strategy-inputs">
        {(strategy === 'equal_profit' || strategy === 'proportional' || 
          strategy === 'kelly' || strategy === 'flat_percent') && (
          <div className="input-group">
            <label>
              <Wallet size={14} />
              Банкролл:
            </label>
            <input
              type="number"
              value={totalBankroll}
              onChange={(e) => setTotalBankroll(Number(e.target.value))}
              min={1000}
              step={1000}
            />
            <span>₽</span>
          </div>
        )}

        {strategy === 'fixed_amount' && (
          <div className="input-group">
            <label>Фикс. сумма:</label>
            <input
              type="number"
              value={fixedAmount}
              onChange={(e) => setFixedAmount(Number(e.target.value))}
              min={100}
              step={100}
            />
            <span>₽</span>
          </div>
        )}

        {strategy === 'kelly' && (
          <>
            <div className="input-group">
              <label>Фракция Келли:</label>
              <input
                type="number"
                value={kellyFraction}
                onChange={(e) => setKellyFraction(Number(e.target.value))}
                min={0.05}
                max={1}
                step={0.05}
              />
            </div>
            {fork.legs.map((leg, i) => (
              <div key={i} className="input-group probability">
                <label>Вероятность {getBookmakerName(leg.bookmaker_slug)}:</label>
                <input
                  type="number"
                  value={probabilities[i]}
                  onChange={(e) => {
                    const newProbs = [...probabilities];
                    newProbs[i] = Number(e.target.value);
                    setProbabilities(newProbs);
                  }}
                  min={0.1}
                  max={0.9}
                  step={0.05}
                />
              </div>
            ))}
          </>
        )}

        {strategy === 'flat_percent' && (
          <div className="input-group">
            <label>% от банка:</label>
            <input
              type="number"
              value={flatPercent * 100}
              onChange={(e) => setFlatPercent(Number(e.target.value) / 100)}
              min={0.5}
              max={10}
              step={0.5}
            />
            <span>%</span>
          </div>
        )}
      </div>

      {/* Stakes table */}
      <div className="stakes-table">
        <table>
          <thead>
            <tr>
              <th>БК</th>
              <th>Счёт</th>
              <th>Кэф</th>
              <th>Сумма</th>
              <th>Прибыль</th>
              <th>ROI</th>
            </tr>
          </thead>
          <tbody>
            {stakePlan.stakes.map((stake) => {
              const account = accounts.find(a => 
                a.bookmaker_slug === stake.bookmaker_slug && 
                a.status === 'Authenticated'
              );
              const insufficient = account && account.balance < stake.stake;
              
              return (
                <tr key={stake.leg_index} className={insufficient ? 'insufficient' : ''}>
                  <td>
                    <img 
                      src={`/icons/bk/${stake.bookmaker_slug}.png`}
                      alt={getBookmakerName(stake.bookmaker_slug)}
                      className="bk-logo-small"
                    />
                    {getBookmakerName(stake.bookmaker_slug)}
                  </td>
                  <td>
                    {account ? (
                      <select>
                        <option value={account.id}>
                          {maskLogin(account.login)} ({formatMoney(account.balance)})
                        </option>
                      </select>
                    ) : (
                      <span className="no-account">Нет счёта</span>
                    )}
                  </td>
                  <td>{fork.legs[stake.leg_index].odds.toFixed(2)}</td>
                  <td className={insufficient ? 'warning' : ''}>
                    {formatMoney(stake.stake)}
                  </td>
                  <td className="profit">+{formatMoney(stake.profit_if_wins)}</td>
                  <td className="roi">{stake.roi_percent.toFixed(2)}%</td>
                </tr>
              );
            })}
          </tbody>
          <tfoot>
            <tr>
              <td colSpan={3}>ИТОГО:</td>
              <td className="total-stake">{formatMoney(stakePlan.total_stake)}</td>
              <td className="total-profit">+{formatMoney(stakePlan.guaranteed_profit)}</td>
              <td className="total-roi">{stakePlan.roi_percent.toFixed(2)}%</td>
            </tr>
          </tfoot>
        </table>
      </div>

      {/* Warning */}
      {hasInsufficientFunds && (
        <div className="warning-banner">
          <AlertCircle size={14} />
          <span>⚠️ Недостаточно средств на одном из счетов!</span>
        </div>
      )}

      {/* Actions */}
      <div className="calc-actions">
        <button
          className="btn-primary btn-large"
          onClick={() => onExecute(stakePlan)}
          disabled={hasInsufficientFunds}
        >
          <TrendingUp size={16} />
          СТАВИТЬ В ОБЕ БК
        </button>
        <button
          className="btn-secondary"
          onClick={() => onExecute({...stakePlan, stakes: stakePlan.stakes.slice(0, 1)})}
          disabled={hasInsufficientFunds}
        >
          Только первое плечо
        </button>
      </div>
    </div>
  );
}

function getBookmakerName(slug: string): string {
  const names: Record<string, string> = {
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
  };
  return names[slug.toLowerCase()] || slug;
}

function maskLogin(login: string): string {
  if (login.length <= 4) return login;
  return login.slice(0, 2) + '***' + login.slice(-2);
}

export default StakingCalculator;

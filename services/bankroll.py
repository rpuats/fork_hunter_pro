# services/bankroll.py
"""
Smart Bankroll Manager — tracks balances per bookmaker, calculates optimal
stake sizes using Kelly Criterion, and manages bankroll distribution.
"""
from typing import List, Dict, Optional, Tuple
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
import logging
import aiosqlite

logger = logging.getLogger(__name__)


class RiskLevel(str, Enum):
    CONSERVATIVE = "conservative"
    MEDIUM = "medium"
    AGGRESSIVE = "aggressive"


@dataclass
class BankrollAccount:
    """Represents a bookmaker account balance."""
    id: int
    bookmaker: str
    balance: float
    currency: str = "RUB"
    initial_balance: float = 0.0
    status: str = "active"
    heat_level: int = 0
    updated_at: str = field(default_factory=lambda: datetime.utcnow().isoformat())

    def to_dict(self) -> Dict:
        return {
            'id': self.id,
            'bookmaker': self.bookmaker,
            'balance': round(self.balance, 2),
            'currency': self.currency,
            'initial_balance': round(self.initial_balance, 2),
            'status': self.status,
            'heat_level': self.heat_level,
            'updated_at': self.updated_at,
            'profit': round(self.balance - self.initial_balance, 2),
            'profit_percent': round(
                (self.balance - self.initial_balance) / self.initial_balance * 100
                if self.initial_balance > 0 else 0, 2
            ),
        }


@dataclass
class OptimalStake:
    """Represents an optimally calculated stake for a bookmaker."""
    bookmaker: str
    stake: float
    percent_of_bankroll: float
    kelly_fraction: float
    risk_adjusted_stake: float

    def to_dict(self) -> Dict:
        return {
            'bookmaker': self.bookmaker,
            'stake': round(self.stake, 2),
            'percent_of_bankroll': round(self.percent_of_bankroll, 2),
            'kelly_fraction': round(self.kelly_fraction, 4),
            'risk_adjusted_stake': round(self.risk_adjusted_stake, 2),
        }


class BankrollManager:
    """
    Manages bankroll across multiple bookmaker accounts.

    Features:
    - Track balances per bookmaker (SQLite)
    - Kelly Criterion for value bets
    - Optimal stake distribution
    - Risk level management
    """

    RISK_MULTIPLIERS = {
        RiskLevel.CONSERVATIVE: 0.25,
        RiskLevel.MEDIUM: 0.5,
        RiskLevel.AGGRESSIVE: 1.0,
    }

    def __init__(self, db_path: str = "ghost_imperium.db"):
        self.db_path = db_path
        self.db: Optional[aiosqlite.Connection] = None
        self.risk_level = RiskLevel.MEDIUM
        self._accounts: Dict[str, BankrollAccount] = {}

    async def init(self):
        """Initialize database connection and create tables."""
        self.db = await aiosqlite.connect(self.db_path)
        assert self.db is not None
        await self.db.execute("""
            CREATE TABLE IF NOT EXISTS bankroll_accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                bookmaker TEXT UNIQUE NOT NULL,
                balance REAL NOT NULL DEFAULT 0.0,
                currency TEXT NOT NULL DEFAULT 'RUB',
                initial_balance REAL NOT NULL DEFAULT 0.0,
                status TEXT NOT NULL DEFAULT 'active',
                heat_level INTEGER NOT NULL DEFAULT 0,
                updated_at TEXT NOT NULL
            )
        """)
        await self.db.commit()
        await self._load_accounts()

    async def _load_accounts(self):
        """Load all accounts from database into memory."""
        assert self.db is not None
        cursor = await self.db.execute("SELECT * FROM bankroll_accounts")
        rows = await cursor.fetchall()
        for row in rows:
            account = BankrollAccount(
                id=row[0],
                bookmaker=row[1],
                balance=row[2],
                currency=row[3],
                initial_balance=row[4],
                status=row[5],
                heat_level=row[6],
                updated_at=row[7],
            )
            self._accounts[account.bookmaker] = account

    async def update_balance(
        self,
        bookmaker: str,
        balance: float,
        currency: str = "RUB",
        initial_balance: Optional[float] = None,
    ) -> BankrollAccount:
        """
        Update or create a bookmaker account balance.

        Args:
            bookmaker: Bookmaker slug.
            balance: Current balance.
            currency: Currency code.
            initial_balance: Initial deposit (used for profit calculation).

        Returns:
            Updated BankrollAccount.
        """
        assert self.db is not None
        now = datetime.utcnow().isoformat()

        if bookmaker in self._accounts:
            existing = self._accounts[bookmaker]
            init_bal = initial_balance if initial_balance is not None else existing.initial_balance
            await self.db.execute("""
                UPDATE bankroll_accounts
                SET balance = ?, currency = ?, initial_balance = ?, updated_at = ?
                WHERE bookmaker = ?
            """, (balance, currency, init_bal, now, bookmaker))
            account = BankrollAccount(
                id=existing.id,
                bookmaker=bookmaker,
                balance=balance,
                currency=currency,
                initial_balance=init_bal,
                status=existing.status,
                heat_level=existing.heat_level,
                updated_at=now,
            )
        else:
            init_bal = initial_balance if initial_balance is not None else balance
            cursor = await self.db.execute("""
                INSERT INTO bankroll_accounts (bookmaker, balance, currency, initial_balance, updated_at)
                VALUES (?, ?, ?, ?, ?)
            """, (bookmaker, balance, currency, init_bal, now))
            await self.db.commit()
            account = BankrollAccount(
                id=cursor.lastrowid or 0,
                bookmaker=bookmaker,
                balance=balance,
                currency=currency,
                initial_balance=init_bal,
                updated_at=now,
            )

        await self.db.commit()
        self._accounts[bookmaker] = account
        return account

    async def set_heat_level(self, bookmaker: str, heat_level: int) -> None:
        """
        Set the heat level (risk of account restriction) for a bookmaker.

        Args:
            bookmaker: Bookmaker slug.
            heat_level: Heat level 0-100.
        """
        assert self.db is not None
        if bookmaker in self._accounts:
            account = self._accounts[bookmaker]
            account.heat_level = max(0, min(100, heat_level))
            await self.db.execute("""
                UPDATE bankroll_accounts SET heat_level = ?, updated_at = ?
                WHERE bookmaker = ?
            """, (account.heat_level, datetime.utcnow().isoformat(), bookmaker))
            await self.db.commit()

    async def set_status(self, bookmaker: str, status: str) -> None:
        """Set account status (active, suspended, restricted)."""
        assert self.db is not None
        if bookmaker in self._accounts:
            account = self._accounts[bookmaker]
            account.status = status
            await self.db.execute("""
                UPDATE bankroll_accounts SET status = ?, updated_at = ?
                WHERE bookmaker = ?
            """, (status, datetime.utcnow().isoformat(), bookmaker))
            await self.db.commit()

    def get_account(self, bookmaker: str) -> Optional[BankrollAccount]:
        """Get account by bookmaker slug."""
        return self._accounts.get(bookmaker)

    def get_all_accounts(self) -> List[BankrollAccount]:
        """Get all accounts."""
        return list(self._accounts.values())

    def get_total_balance(self) -> float:
        """Get total balance across all active accounts."""
        return sum(
            acc.balance for acc in self._accounts.values()
            if acc.status == "active"
        )

    def calculate_kelly_fraction(
        self,
        odds: float,
        fair_probability: float,
    ) -> float:
        """
        Calculate Kelly Criterion fraction.

        Formula: f* = (bp - q) / b
        where:
            b = odds - 1 (decimal odds minus 1)
            p = fair probability of winning
            q = 1 - p (probability of losing)

        Args:
            odds: Decimal odds offered by bookmaker.
            fair_probability: Calculated fair probability of the outcome.

        Returns:
            Kelly fraction (0 if no edge, negative if negative EV).
        """
        if odds <= 1.0 or fair_probability <= 0 or fair_probability >= 1:
            return 0.0

        b = odds - 1.0
        p = fair_probability
        q = 1.0 - p

        kelly = (b * p - q) / b
        return max(0.0, kelly)

    def calculate_optimal_stake(
        self,
        bookmaker: str,
        odds: float,
        fair_probability: float,
        total_stake: Optional[float] = None,
        risk_level: Optional[RiskLevel] = None,
    ) -> OptimalStake:
        """
        Calculate optimal stake for a bet using Kelly Criterion.

        Args:
            bookmaker: Target bookmaker slug.
            odds: Decimal odds.
            fair_probability: Fair probability of outcome.
            total_stake: Total amount to distribute (if None, uses account balance).
            risk_level: Override risk level.

        Returns:
            OptimalStake with calculated values.
        """
        account = self._accounts.get(bookmaker)
        if not account:
            return OptimalStake(
                bookmaker=bookmaker,
                stake=0.0,
                percent_of_bankroll=0.0,
                kelly_fraction=0.0,
                risk_adjusted_stake=0.0,
            )

        bankroll = account.balance
        if total_stake is not None:
            bankroll = total_stake

        kelly = self.calculate_kelly_fraction(odds, fair_probability)
        risk = risk_level or self.risk_level
        multiplier = self.RISK_MULTIPLIERS[risk]

        heat_penalty = 1.0 - (account.heat_level / 200.0)

        raw_stake = bankroll * kelly * multiplier * heat_penalty
        percent = (raw_stake / bankroll * 100) if bankroll > 0 else 0.0

        max_stake_pct = 0.05 if risk == RiskLevel.CONSERVATIVE else (
            0.10 if risk == RiskLevel.MEDIUM else 0.20
        )
        max_stake = bankroll * max_stake_pct
        final_stake = min(raw_stake, max_stake)

        return OptimalStake(
            bookmaker=bookmaker,
            stake=round(final_stake, 2),
            percent_of_bankroll=round(percent, 2),
            kelly_fraction=round(kelly, 4),
            risk_adjusted_stake=round(final_stake, 2),
        )

    def calculate_optimal_distribution(
        self,
        total_amount: float,
        risk_level: Optional[RiskLevel] = None,
    ) -> List[OptimalStake]:
        """
        Calculate optimal distribution of a total amount across all bookmakers.

        Distributes proportionally to available balance, adjusted for heat level.

        Args:
            total_amount: Total amount to distribute.
            risk_level: Override risk level.

        Returns:
            List of OptimalStake for each bookmaker.
        """
        risk = risk_level or self.risk_level
        active_accounts = [
            acc for acc in self._accounts.values()
            if acc.status == "active" and acc.balance > 0
        ]

        if not active_accounts:
            return []

        total_available = sum(acc.balance for acc in active_accounts)
        if total_available <= 0:
            return []

        distributions = []
        for acc in active_accounts:
            weight = acc.balance / total_available
            heat_penalty = 1.0 - (acc.heat_level / 200.0)
            adjusted_weight = weight * heat_penalty

            stake = total_amount * adjusted_weight
            percent = (stake / total_amount * 100) if total_amount > 0 else 0.0

            distributions.append(OptimalStake(
                bookmaker=acc.bookmaker,
                stake=round(stake, 2),
                percent_of_bankroll=round(percent, 2),
                kelly_fraction=0.0,
                risk_adjusted_stake=round(stake, 2),
            ))

        distributions.sort(key=lambda x: x.stake, reverse=True)
        return distributions

    def get_summary(self) -> Dict:
        """Get bankroll summary."""
        accounts = self.get_all_accounts()
        total = self.get_total_balance()
        total_initial = sum(acc.initial_balance for acc in accounts)
        total_profit = total - total_initial
        profit_pct = (total_profit / total_initial * 100) if total_initial > 0 else 0.0

        return {
            'total_balance': round(total, 2),
            'total_initial': round(total_initial, 2),
            'total_profit': round(total_profit, 2),
            'profit_percent': round(profit_pct, 2),
            'active_accounts': sum(1 for acc in accounts if acc.status == 'active'),
            'total_accounts': len(accounts),
            'risk_level': self.risk_level.value,
            'accounts': [acc.to_dict() for acc in accounts],
        }

    async def close(self):
        """Close database connection."""
        if self.db:
            await self.db.close()
            self.db = None

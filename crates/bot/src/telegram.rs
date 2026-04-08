use shared::Surebet;
use teloxide::prelude::*;
use teloxide::types::ChatId;
use tracing::{error, info};

pub struct TelegramBot {
    bot: Bot,
    admin_chats: Vec<i64>,
    min_profit: f64,
    silent: bool,
}

impl TelegramBot {
    pub fn new(token: &str, admin_chats: Vec<i64>, min_profit: f64, silent: bool) -> Self {
        Self {
            bot: Bot::new(token),
            admin_chats,
            min_profit,
            silent,
        }
    }

    pub async fn notify_surebet(&self, surebet: &Surebet) {
        if surebet.profit_percent < self.min_profit {
            return;
        }

        let message = self.format_surebet_message(surebet);

        for &chat_id in &self.admin_chats {
            if let Err(e) = self.bot.send_message(ChatId(chat_id), &message).await {
                error!(chat_id, error = e.to_string(), "Failed to send Telegram message");
            }
        }
    }

    pub async fn notify_system(&self, message: &str) {
        if self.silent {
            return;
        }

        for &chat_id in &self.admin_chats {
            let _ = self.bot.send_message(ChatId(chat_id), message).await;
        }
    }

    pub fn format_surebet_message(&self, surebet: &Surebet) -> String {
        let time_str = surebet.start_time.map(|t| t.format("%H:%M").to_string()).unwrap_or_else(|| "N/A".to_string());
        let mut msg = format!(
            "🔥 ВИЛКА {:.2}%\n\n📌 {{home}} vs {{away}}\n🏆 {{league}}\n⏰ {{time}}\n\n",
            surebet.profit_percent,
        );
        msg = msg.replace("{home}", &surebet.home_team).replace("{away}", &surebet.away_team).replace("{league}", &surebet.league).replace("{time}", &time_str);

        for (i, leg) in surebet.legs.iter().enumerate() {
            msg.push_str(&format!(
                "{}. {} → {} @ {:.2} (ставка: {:.0})\n",
                i + 1,
                leg.bookmaker,
                leg.selection,
                leg.odds,
                leg.stake
            ));
        }

        msg.push_str(&format!(
            "\n💰 Выигрыш: {:.0}₽\n💵 Общая ставка: {:.0}₽",
            surebet.legs.first().map(|l| l.payout).unwrap_or(0.0),
            surebet.total_stake
        ));

        msg
    }

    /// Запуск бота в отдельном tokio task — НЕ блокирует вызывающий код
    pub fn spawn(self: std::sync::Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let bot = self.bot.clone();

            let handler = |msg: Message, bot: Bot| async move {
                if let Some(text) = msg.text() {
                    match text {
                        "/start" => {
                            let _ = bot
                                .send_message(msg.chat.id, "👋 Ghost Imperium Bot\n\nКоманды:\n/status - статус сканера\n/help - помощь")
                                .await;
                        }
                        "/status" => {
                            let _ = bot
                                .send_message(msg.chat.id, "✅ Сканер работает")
                                .await;
                        }
                        "/help" => {
                            let _ = bot
                                .send_message(msg.chat.id, "📖 Ghost Imperium - сканер вилок\n\nДоступные команды:\n/start - начать\n/status - статус\n/help - помощь")
                                .await;
                        }
                        _ => {}
                    }
                }
                Ok(())
            };

            info!("Telegram bot starting (async spawn mode)...");
            teloxide::repl(bot, handler).await;
            info!("Telegram bot stopped");
        })
    }
}

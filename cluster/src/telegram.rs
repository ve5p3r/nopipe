pub async fn notify_ops(bot_token: &str, chat_id: i64, text: &str) {
    let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);
    let client = reqwest::Client::new();
    let _ = client
        .post(&url)
        .json(&serde_json::json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "HTML"
        }))
        .send()
        .await;
    // fire-and-forget, never block the response path on Telegram
}

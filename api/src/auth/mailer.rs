use std::sync::Mutex;

#[async_trait::async_trait]
pub trait Mailer: Send + Sync {
    async fn send_magic_link(&self, to: &str, link: &str) -> Result<(), String>;
}

pub struct ResendMailer {
    pub api_key: String,
    pub http: reqwest::Client,
}

#[async_trait::async_trait]
impl Mailer for ResendMailer {
    async fn send_magic_link(&self, to: &str, link: &str) -> Result<(), String> {
        let body = serde_json::json!({
            "from": "peetopee.cash <peetopee@bitwyre.com>",
            "to": [to],
            "subject": "Your peetopee.cash login link",
            "html": format!(
                "<p>Click to log in to peetopee.cash:</p><p><a href=\"{link}\">{link}</a></p><p>This link expires in 15 minutes.</p>"
            ),
        });
        let res = self.http
            .post("https://api.resend.com/emails")
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !res.status().is_success() {
            return Err(format!("resend returned {}", res.status()));
        }
        Ok(())
    }
}

/// Dev fallback when RESEND_API_KEY is unset: logs the link.
pub struct LogMailer;

#[async_trait::async_trait]
impl Mailer for LogMailer {
    async fn send_magic_link(&self, to: &str, link: &str) -> Result<(), String> {
        tracing::info!("magic link for {to}: {link}");
        Ok(())
    }
}

/// Test mailer capturing (to, link) pairs.
#[derive(Default)]
pub struct MemoryMailer {
    pub sent: Mutex<Vec<(String, String)>>,
}

#[async_trait::async_trait]
impl Mailer for MemoryMailer {
    async fn send_magic_link(&self, to: &str, link: &str) -> Result<(), String> {
        self.sent.lock().unwrap().push((to.to_string(), link.to_string()));
        Ok(())
    }
}

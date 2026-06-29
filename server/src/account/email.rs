//! SMTP 发信（找回密码 / 邮箱验证）。SMTP 未配置时调用方应跳过（返回"未配置"）。

use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::config::SmtpConfig;

/// 发送一封纯文本邮件。失败返回 Err（调用方best-effort，不阻塞主流程）。
pub async fn send(cfg: &SmtpConfig, to: &str, subject: &str, body: String) -> anyhow::Result<()> {
    let email = Message::builder()
        .from(cfg.from.parse().map_err(|e| anyhow::anyhow!("bad from address: {e}"))?)
        .to(to.parse().map_err(|e| anyhow::anyhow!("bad to address: {e}"))?)
        .subject(subject)
        .body(body)?;

    let builder = if cfg.starttls {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&cfg.host)?
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&cfg.host)?
    };
    let mut builder = builder.port(cfg.port);
    if !cfg.username.is_empty() {
        builder = builder.credentials(Credentials::new(cfg.username.clone(), cfg.password.clone()));
    }
    let mailer = builder.build();
    mailer.send(email).await?;
    Ok(())
}

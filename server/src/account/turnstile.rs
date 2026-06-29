//! B5：Cloudflare Turnstile 服务端校验。复用项目已有的 reqwest（json + rustls/ring，无需新依赖）。
//! 启用时网络失败 fail-closed（不让验证服务抖动变成开放注册口子）；未配 secret = 放行（gate 在调用方）。

/// 校验 Turnstile token。secret 为空 → 直接放行（未配置）；token 为空 → 拒。
pub async fn verify(secret: &str, token: &str, remote_ip: Option<&str>) -> bool {
    if secret.is_empty() {
        return true;
    }
    if token.is_empty() {
        return false;
    }
    let mut form = vec![("secret", secret.to_string()), ("response", token.to_string())];
    if let Some(ip) = remote_ip {
        form.push(("remoteip", ip.to_string()));
    }
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    match client
        .post("https://challenges.cloudflare.com/turnstile/v0/siteverify")
        .form(&form)
        .send()
        .await
    {
        Ok(r) => r
            .json::<serde_json::Value>()
            .await
            .ok()
            .and_then(|v| v["success"].as_bool())
            .unwrap_or(false),
        Err(e) => {
            tracing::warn!(category = "auth", "turnstile verify 失败：{e}");
            false // fail-closed
        }
    }
}

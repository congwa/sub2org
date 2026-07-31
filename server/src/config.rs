use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

/// 运行期配置。M1 从 config.toml 读取；后续 worker/apikey 迁到 Postgres。
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default = "default_bind")]
    pub bind: String,
    #[serde(default)]
    pub worker_tokens: Vec<String>,
    #[serde(default)]
    pub api_keys: Vec<String>,
    #[serde(default = "default_queue_wait")]
    pub queue_wait_secs: u64,
    #[serde(default = "default_generation")]
    pub generation_secs: u64,
    /// model 名前缀 → provider。最长前缀优先匹配。
    #[serde(default)]
    pub provider_map: HashMap<String, String>,
    /// Postgres 连接串。为空 → 用内存 store（dev）。
    #[serde(default)]
    pub database_url: Option<String>,
    /// 续接会话但其 worker 离线时：true=故障转移到其它 worker 开新窗，false=报错 409。
    #[serde(default)]
    pub failover_on_offline: bool,
    /// 心跳超过该秒数没来 → 判定 worker 过期、剔除（M3）。
    #[serde(default = "default_stale")]
    pub heartbeat_stale_secs: i64,
    /// 限流恢复时间解析不出时的兜底冷却秒数（M3）。
    #[serde(default = "default_cooldown")]
    pub default_cooldown_secs: i64,
    // ---- 日志系统 ----
    /// 是否写库（false=只控制台，仍保留内存环形供查询）。
    #[serde(default = "default_true_bool")]
    pub log_to_db: bool,
    /// 入库最低级别（trace/debug/info/warn/error），低于此只进控制台。
    #[serde(default = "default_log_level")]
    pub log_min_level: String,
    /// 日志保留天数，超期自动清理。
    #[serde(default = "default_retention_days")]
    pub log_retention_days: i64,
    /// 日志投递通道容量（满了丢弃并计数，绝不阻塞）。
    #[serde(default = "default_log_capacity")]
    pub log_channel_capacity: usize,
    // ---- 用户体系 / 鉴权 / 计费 ----
    /// admin 接口令牌（`X-Admin-Token`）；空=不走 token 校验（改走控制台 admin JWT 登录）。
    #[serde(default)]
    pub admin_token: String,
    /// P0#2：admin_token 为空时是否「放行全部 /admin/*」（仅供内网/dev）。默认 false=拒绝（deny-by-default）。
    /// 即便为 true，控制台的 admin 角色 JWT 登录始终可用；置 true 只是额外放开免鉴权直连。
    #[serde(default)]
    pub allow_open_admin: bool,
    /// Prometheus /metrics 抓取令牌（Bearer 或 ?token=）；空=开放（需用 nginx allowlist 限制）。
    #[serde(default)]
    pub metrics_token: String,
    /// 用户 dashboard JWT 签名密钥。
    #[serde(default = "default_jwt_secret")]
    pub jwt_secret: String,
    /// 是否允许自助注册。
    #[serde(default = "default_true_bool")]
    pub allow_registration: bool,
    /// 计费总开关（false=不计费，保持 M1–M3 行为）。
    #[serde(default)]
    pub billing_enabled: bool,
    /// 默认每次扣费（credits）。
    #[serde(default = "default_cost")]
    pub default_cost: i64,
    /// 新用户初始余额。
    #[serde(default)]
    pub default_initial_balance: i64,
    /// model 名前缀 → 每次扣费（credits），最长前缀匹配，fallback default_cost。
    #[serde(default)]
    pub model_cost: HashMap<String, i64>,
    /// B3：全局费率倍率（作用于所有扣费，默认 1.0）。
    #[serde(default = "default_multiplier")]
    pub cost_multiplier: f64,
    /// B3：按 model 前缀的费率倍率（最长前缀匹配，fallback 1.0）。最终扣费 = 基础价 × model倍率 × 全局倍率。
    #[serde(default)]
    pub model_multiplier: HashMap<String, f64>,
    /// 对外公布的 model id 列表（`GET /v1/models` 的来源）。空 = 回退默认 ["gpt-4o","claude"]。
    /// 非空时还会启用 model 白名单校验：不在表内且无 provider_map 前缀匹配的 model → 400。
    #[serde(default)]
    pub models: Vec<String>,
    /// 首启 bootstrap 的 admin 账号。
    #[serde(default = "default_admin_user")]
    pub admin_username: String,
    /// admin 密码；空 = 首启随机生成并打印一次。
    #[serde(default)]
    pub admin_password: String,
    // ---- 生产硬化 / 网络 ----
    /// 生产模式：开启后 `validate_for_prod` 会拒绝带 dev 默认值启动（也可用 env SUB2ORG_ENV=prod 打开）。
    #[serde(default)]
    pub production: bool,
    /// P1#5：是否信任反代写入的 X-Real-IP / X-Forwarded-For（限流/IP 过滤取客户端 IP 用）。
    /// 默认 true（生产应在 nginx 后）。直连暴露务必设 false，否则客户端可伪造头绕过限流/IP 规则。
    #[serde(default = "default_true_bool")]
    pub trust_proxy_headers: bool,
    /// CORS 允许的来源；非空 = 收紧到白名单，空 = dev 放行所有。
    #[serde(default)]
    pub allowed_origins: Vec<String>,
    /// 请求体上限（字节），防超大 prompt 打爆内存。
    #[serde(default = "default_max_body")]
    pub max_body_bytes: usize,
    /// 对外公网基址（拼支付回调 / 邮件链接），如 https://sub2org.example.com 。
    #[serde(default)]
    pub public_base_url: String,
    // ---- 账号恢复 / 邮箱 ----
    /// 开启后注册需邮箱验证才能消费（默认关；需配 [smtp]）。
    #[serde(default)]
    pub require_email_verify: bool,
    /// SMTP 配置；缺省 = 不发邮件（找回/验证端点返回"未配置"）。
    #[serde(default)]
    pub smtp: Option<SmtpConfig>,
    // ---- 在线支付（易支付/epay）----
    /// 支付配置；缺省 = 在线支付未启用（仍可用兑换码 / admin 充值）。
    #[serde(default)]
    pub payment: Option<PaymentConfig>,
    // ---- worker 规模化预配（扩展集中领取身份）----
    /// worker 预配（`POST /worker/claim`）；默认开启，用 [worker] 调。
    #[serde(default)]
    pub worker: WorkerProvisionConfig,
    // ---- 告警钩子 ----
    /// 告警配置；缺省 = 未启用（无 webhook / 邮件告警）。
    #[serde(default)]
    pub alert: Option<AlertConfig>,
    // ---- 网络层 IP 过滤 ----
    /// IP allow/deny（默认空 = 不限制）。
    #[serde(default)]
    pub security: SecurityConfig,
    // ---- 图片 / 多模态 ----
    /// 图片支持（vision）；默认开启，用 [image] 调。
    #[serde(default)]
    pub image: ImageConfig,
    // ---- 熔断（B4）----
    /// 坏 worker 熔断；默认关。窗内连续错误达阈值 → 自动停派发一段时间后自愈。
    #[serde(default)]
    pub breaker: BreakerConfig,
    // ---- 验证码（B5）----
    /// 注册/登录 Cloudflare Turnstile 验证码；缺省 = 未启用（流程零变化）。
    #[serde(default)]
    pub turnstile: Option<TurnstileConfig>,
}

/// B5 Cloudflare Turnstile 配置。secret_key 建议走 env SUB2ORG_TURNSTILE_SECRET。
#[derive(Debug, Clone, Deserialize)]
pub struct TurnstileConfig {
    /// 公开 site key（下发给控制台渲染 widget）。
    pub site_key: String,
    /// 服务端校验密钥。
    #[serde(default)]
    pub secret_key: String,
    /// 注册时校验（默认开）。
    #[serde(default = "default_true_bool")]
    pub on_register: bool,
    /// 登录时校验（默认关）。
    #[serde(default)]
    pub on_login: bool,
}

/// B4 熔断配置。仅对「worker 级故障码」计数（见 fault_codes），不计每请求内容错误。
#[derive(Debug, Clone, Deserialize)]
pub struct BreakerConfig {
    #[serde(default)]
    pub enabled: bool,
    /// 滑窗长度（秒）。
    #[serde(default = "default_breaker_window")]
    pub window_secs: i64,
    /// 窗内错误数达此阈值 → 熔断。
    #[serde(default = "default_breaker_threshold")]
    pub threshold: u32,
    /// 熔断打开时长（秒），到期自愈。
    #[serde(default = "default_breaker_open")]
    pub open_secs: i64,
    /// 计入熔断的 worker 级故障码（其余如内容错误不计）。
    #[serde(default = "default_fault_codes")]
    pub fault_codes: Vec<String>,
}

impl Default for BreakerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            window_secs: default_breaker_window(),
            threshold: default_breaker_threshold(),
            open_secs: default_breaker_open(),
            fault_codes: default_fault_codes(),
        }
    }
}

fn default_breaker_window() -> i64 {
    60
}
fn default_breaker_threshold() -> u32 {
    5
}
fn default_breaker_open() -> i64 {
    300
}
fn default_fault_codes() -> Vec<String> {
    ["rate_limited", "dom_error", "logged_out", "navigation_failed"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

/// 图片/多模态配置。base64 data URI 直接透传；https URL 由服务端 fetch 后归一为 data URI。
#[derive(Debug, Clone, Deserialize)]
pub struct ImageConfig {
    /// 总开关（关 → 图片仍被丢弃，行为同旧版）。
    #[serde(default = "default_true_bool")]
    pub enabled: bool,
    /// 单图解码后最大字节（base64 估算 / fetch content-length 与实际皆校验）。
    #[serde(default = "default_image_max_bytes")]
    pub max_bytes: usize,
    /// 单请求最多图片数。
    #[serde(default = "default_image_max_count")]
    pub max_count: usize,
    /// 是否允许服务端 fetch https URL 图片（关 → 只收 base64 data URI）。
    #[serde(default = "default_true_bool")]
    pub fetch_url: bool,
    /// URL fetch 超时秒。
    #[serde(default = "default_image_fetch_timeout")]
    pub fetch_timeout_secs: u64,
}

impl Default for ImageConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_bytes: default_image_max_bytes(),
            max_count: default_image_max_count(),
            fetch_url: true,
            fetch_timeout_secs: default_image_fetch_timeout(),
        }
    }
}

fn default_image_max_bytes() -> usize {
    5_242_880 // 5 MiB（解码后）
}
fn default_image_max_count() -> usize {
    4
}
fn default_image_fetch_timeout() -> u64 {
    10
}

/// 网络层 IP allow/deny。allow 非空 = 仅放行这些 CIDR（allowlist）；deny 始终优先拦截。
/// CIDR 启动时解析，非法值 fail-fast 拒启。XFF 信任假设：仅 nginx 反代后可信。
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SecurityConfig {
    /// 允许的 CIDR/IP 列表；非空 = allowlist 模式。空 = 不限制（仅 deny 生效）。
    #[serde(default)]
    pub ip_allow: Vec<String>,
    /// 拒绝的 CIDR/IP 列表；始终优先生效。
    #[serde(default)]
    pub ip_deny: Vec<String>,
}

/// 告警配置（webhook_secret 建议走 env SUB2ORG_ALERT_WEBHOOK_SECRET）。
#[derive(Debug, Clone, Deserialize)]
pub struct AlertConfig {
    /// 总开关。
    #[serde(default)]
    pub enabled: bool,
    /// 告警 webhook（HTTP POST JSON）；空 = 不发 webhook。
    #[serde(default)]
    pub webhook_url: Option<String>,
    /// webhook 鉴权头 X-Sub2org-Alert-Token（建议走 env）。
    #[serde(default)]
    pub webhook_secret: Option<String>,
    /// 告警收件邮箱（复用 [smtp]）；空 = 不发邮件。
    #[serde(default)]
    pub email_to: Option<String>,
    /// 余额低于该值（reserve 扣后）触发 balance_low；<=0 关闭。
    #[serde(default)]
    pub balance_low_threshold: i64,
    /// 错误率阈值 0.0..1.0（60s 窗、≥20 样本才评估）。
    #[serde(default = "default_error_rate")]
    pub error_rate_threshold: f64,
    /// 同一告警键最小间隔秒数（去重防刷）。
    #[serde(default = "default_alert_interval")]
    pub min_interval_secs: i64,
}

fn default_error_rate() -> f64 {
    0.5
}
fn default_alert_interval() -> i64 {
    300
}

/// worker 规模化预配配置。扩展用共享 worker_token 调 `/worker/claim` 领取唯一身份。
#[derive(Debug, Clone, Deserialize)]
pub struct WorkerProvisionConfig {
    /// `POST /worker/claim` 是否可用（关 → 403，仅能手填/socket 直连）。
    #[serde(default = "default_true_bool")]
    pub provision_enabled: bool,
    /// claim 未指定 provider 时的默认值。
    #[serde(default = "default_provider")]
    pub default_provider: String,
    /// 自动生成 account_label 的前缀（→ wk-001）。
    #[serde(default = "default_label_prefix")]
    pub label_prefix: String,
}

impl Default for WorkerProvisionConfig {
    fn default() -> Self {
        Self {
            provision_enabled: true,
            default_provider: default_provider(),
            label_prefix: default_label_prefix(),
        }
    }
}

fn default_provider() -> String {
    "chatgpt".to_string()
}
fn default_label_prefix() -> String {
    "wk".to_string()
}

/// SMTP 发信配置（password 建议走 env SUB2ORG_SMTP_PASSWORD）。
#[derive(Debug, Clone, Deserialize)]
pub struct SmtpConfig {
    pub host: String,
    #[serde(default = "default_smtp_port")]
    pub port: u16,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    /// 发件人地址，如 "sub2org <no-reply@example.com>"。
    pub from: String,
    /// 是否用 STARTTLS（587）；false=隐式 TLS（465）。
    #[serde(default = "default_true_bool")]
    pub starttls: bool,
}

/// 易支付（彩虹/epay 兼容）配置。epay_key 建议走 env SUB2ORG_EPAY_KEY。
#[derive(Debug, Clone, Deserialize)]
pub struct PaymentConfig {
    /// 网关基址（末尾带或不带 / 都可），如 https://pay.example.com 。
    pub epay_url: String,
    pub epay_pid: String,
    #[serde(default)]
    pub epay_key: String,
    /// 允许的支付渠道（epay `type`）：alipay / wxpay / qqpay 等。
    #[serde(default = "default_channels")]
    pub enabled_channels: Vec<String>,
    /// 单笔最小/最大下单积分。
    #[serde(default = "default_min_amount")]
    pub min_amount: i64,
    #[serde(default = "default_max_amount")]
    pub max_amount: i64,
    /// 多少积分 = 1 元人民币（下单时 money = amount / credits_per_cny）。
    #[serde(default = "default_credits_per_cny")]
    pub credits_per_cny: i64,
}

fn default_max_body() -> usize {
    33_554_432 // 32 MiB —— 需容纳带图请求（base64 data URI 体积膨胀；上限由 image.max_count×max_bytes 兜底）
}
fn default_smtp_port() -> u16 {
    587
}
fn default_channels() -> Vec<String> {
    vec!["alipay".to_string(), "wxpay".to_string()]
}
fn default_min_amount() -> i64 {
    1
}
fn default_max_amount() -> i64 {
    1_000_000
}
fn default_credits_per_cny() -> i64 {
    100
}

fn default_jwt_secret() -> String {
    "dev-secret-change-me".to_string()
}
fn default_multiplier() -> f64 {
    1.0
}
fn default_cost() -> i64 {
    1
}
fn default_admin_user() -> String {
    "admin".to_string()
}

fn default_stale() -> i64 {
    90
}
fn default_cooldown() -> i64 {
    3600
}
fn default_true_bool() -> bool {
    true
}
fn default_log_level() -> String {
    "info".to_string()
}
fn default_retention_days() -> i64 {
    14
}
fn default_log_capacity() -> usize {
    10000
}

fn default_bind() -> String {
    "0.0.0.0:3000".to_string()
}
fn default_queue_wait() -> u64 {
    120
}
fn default_generation() -> u64 {
    180
}

/// P1#4：常量时间字节比较（防 token 计时枚举）。长度不等直接 false（长度本身非敏感）。
pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let raw = std::fs::read_to_string(path)?;
        let cfg: Config = toml::from_str(&raw)?;
        Ok(cfg)
    }

    /// 用环境变量覆盖敏感项（非空才覆盖）。在 main 启动钩子内、validate 之前调用。
    pub fn apply_env_overrides(&mut self) {
        use std::env;
        let set = |dst: &mut String, key: &str| {
            if let Ok(v) = env::var(key) {
                if !v.is_empty() {
                    *dst = v;
                }
            }
        };
        set(&mut self.jwt_secret, "SUB2ORG_JWT_SECRET");
        set(&mut self.admin_token, "SUB2ORG_ADMIN_TOKEN");
        set(&mut self.metrics_token, "SUB2ORG_METRICS_TOKEN");
        set(&mut self.admin_password, "SUB2ORG_ADMIN_PASSWORD");
        set(&mut self.bind, "SUB2ORG_BIND");
        set(&mut self.public_base_url, "SUB2ORG_PUBLIC_BASE_URL");
        if let Ok(v) = env::var("SUB2ORG_DATABASE_URL") {
            if !v.is_empty() {
                self.database_url = Some(v);
            }
        }
        if let Ok(v) = env::var("SUB2ORG_ENV") {
            if v == "prod" || v == "production" {
                self.production = true;
            }
        }
        if let Some(p) = self.payment.as_mut() {
            set(&mut p.epay_pid, "SUB2ORG_EPAY_PID");
            set(&mut p.epay_key, "SUB2ORG_EPAY_KEY");
        }
        if let Some(s) = self.smtp.as_mut() {
            set(&mut s.password, "SUB2ORG_SMTP_PASSWORD");
        }
        if let Some(t) = self.turnstile.as_mut() {
            set(&mut t.secret_key, "SUB2ORG_TURNSTILE_SECRET");
        }
        if let Some(a) = self.alert.as_mut() {
            if let Ok(v) = env::var("SUB2ORG_ALERT_WEBHOOK_SECRET") {
                if !v.is_empty() {
                    a.webhook_secret = Some(v);
                }
            }
        }
    }

    /// 生产模式下拒绝带 dev 默认值启动（密钥/裸奔 admin/种子凭据/无 DB）。
    pub fn validate_for_prod(&self) -> anyhow::Result<()> {
        if !self.production {
            return Ok(());
        }
        // 软提醒（非拒启）：/metrics 开放仅靠 nginx allowlist 兜底，输出仅聚合指标无 PII。
        if self.metrics_token.is_empty() {
            eprintln!(
                "[config] 警告：metrics_token 为空 → /metrics 开放，请设 SUB2ORG_METRICS_TOKEN 或确认 nginx 的 location = /metrics allowlist 已生效"
            );
        }
        let mut errs: Vec<String> = Vec::new();
        if self.jwt_secret == default_jwt_secret() || self.jwt_secret.len() < 16 {
            errs.push("jwt_secret 仍是默认或过短（设 SUB2ORG_JWT_SECRET，≥16 位随机串）".into());
        }
        if self.admin_token.is_empty() {
            errs.push("admin_token 为空 → /admin/* 无鉴权（设 SUB2ORG_ADMIN_TOKEN 或 config.admin_token）".into());
        }
        if self.api_keys.iter().any(|k| k == "sk-sub2org-dev") {
            errs.push("api_keys 含 dev 种子 'sk-sub2org-dev'，请移除".into());
        }
        if self.worker_tokens.iter().any(|t| t == "dev-worker-token") {
            errs.push("worker_tokens 含 dev 种子 'dev-worker-token'，请改为强随机".into());
        }
        let memory_backend = self.database_url.as_deref().map(|u| u.is_empty()).unwrap_or(true);
        if memory_backend {
            errs.push("database_url 为空 → 用内存存储（重启丢全部用户/余额/会话），生产必须配 Postgres".into());
        }
        // 内存后端 + 空 admin_password = 每次重启随机改密（admin 永远登不进）。配 Postgres 后空密码可（首启生成一次并持久）。
        if memory_backend && self.admin_password.is_empty() {
            errs.push("admin_password 为空且用内存存储 → 每次重启随机改密（设 SUB2ORG_ADMIN_PASSWORD 或配 Postgres）".into());
        }
        if self.worker.provision_enabled && self.worker_tokens.is_empty() {
            errs.push("worker.provision_enabled=true 但 worker_tokens 为空 → /worker/claim 恒 401（配 worker_tokens 或关 provision_enabled）".into());
        }
        // #7：配了 per-model 差价（model_cost）却没设 models 白名单 → model 准入门是关的，
        // 客户端可传任意 model 串按 default_cost 计费、却被路由到贵模型的 worker（如 gpt-4 按 gpt-3.5 价买）。
        // 生产必须显式声明 models 白名单以启用 consumer 的准入门。
        if self.billing_enabled && !self.model_cost.is_empty() && self.models.is_empty() {
            errs.push("billing_enabled 且配了 model_cost 差价，但 models 白名单为空 → model 准入门关闭，客户端可传未知 model 按 default_cost 买到贵模型；生产必须设 models 白名单".into());
        }
        if !errs.is_empty() {
            anyhow::bail!(
                "生产模式(SUB2ORG_ENV=prod / production=true)配置校验失败：\n - {}",
                errs.join("\n - ")
            );
        }
        Ok(())
    }

    /// P0#2/P1#5：启动期暴露面告警（不阻断 dev；prod 由 validate_for_prod 硬拒）。
    /// bind 非回环（公网/全网卡）时，对仍是 dev 默认值/危险开关发醒目告警。
    pub fn warn_exposure(&self) {
        let host = self.bind.rsplit_once(':').map(|(h, _)| h).unwrap_or(self.bind.as_str());
        let public = !matches!(host, "" | "127.0.0.1" | "::1" | "localhost");
        if !public {
            return;
        }
        if self.jwt_secret == default_jwt_secret() {
            tracing::warn!(category = "system", "⚠️ bind 非回环且 jwt_secret 仍是默认值 → JWT 可被伪造；请设 SUB2ORG_JWT_SECRET");
        }
        if self.admin_token.is_empty() && self.allow_open_admin {
            tracing::warn!(category = "system", "⚠️ bind 非回环且 allow_open_admin=true → /admin/* 免鉴权直连可达；生产请关闭，改用控制台 admin 登录");
        }
        if self.trust_proxy_headers {
            tracing::warn!(category = "system", "bind 非回环且 trust_proxy_headers=true → 须在可信反代(nginx)后；否则 X-Forwarded-For 可伪造绕过限流/IP 规则，直连暴露请设 trust_proxy_headers=false");
        }
    }

    /// P2#11：跨配置一致性告警（不阻断；逻辑自相矛盾的配置常在运行期才暴露，启动期先吼一声）。
    pub fn warn_config_consistency(&self) {
        // 心跳剔除阈值须 > 传输层死连检测窗（ping_interval 20 + ping_timeout 60 = 80s），
        // 否则 health 抢在 on_disconnect 干净清理之前剔除 worker（详见 main.rs ping 配置）。
        const DEAD_CONN_SECS: i64 = 80;
        if self.heartbeat_stale_secs < DEAD_CONN_SECS + 10 {
            tracing::warn!(
                category = "system",
                "heartbeat_stale_secs={} 偏小（应 ≥ ping_interval+ping_timeout+裕量 ≈ 90s）→ 可能在传输层断连前误剔除 worker",
                self.heartbeat_stale_secs
            );
        }
        if let Some(a) = &self.alert {
            if a.enabled
                && a.webhook_url.as_deref().unwrap_or("").is_empty()
                && a.email_to.as_deref().unwrap_or("").is_empty()
            {
                tracing::warn!(category = "system", "alert.enabled=true 但 webhook_url 与 email_to 均空 → 告警无处可发");
            }
        }
        if self.billing_enabled && self.default_cost <= 0 && self.model_cost.is_empty() {
            tracing::warn!(category = "system", "billing_enabled=true 但 default_cost<=0 且 model_cost 为空 → 实际每次扣 0，等于未计费");
        }
    }

    /// 校验 worker 准入 token（P1#4：常量时间比较，防计时枚举；逐项累加不提前退出）。
    pub fn worker_token_ok(&self, token: &str) -> bool {
        let mut ok = false;
        for t in &self.worker_tokens {
            ok |= ct_eq(t.as_bytes(), token.as_bytes());
        }
        ok
    }

    /// 校验消费方 api key（P1#4：常量时间比较）。
    pub fn api_key_ok(&self, key: &str) -> bool {
        let mut ok = false;
        for k in &self.api_keys {
            ok |= ct_eq(k.as_bytes(), key.as_bytes());
        }
        ok
    }

    /// model 名 → provider（最长前缀匹配）；无任何前缀命中返回 None。
    pub fn provider_for_model_opt(&self, model: &str) -> Option<String> {
        let m = model.to_ascii_lowercase();
        let mut best: Option<(&String, &String)> = None;
        for (prefix, provider) in &self.provider_map {
            if m.starts_with(&prefix.to_ascii_lowercase()) {
                match best {
                    Some((bp, _)) if bp.len() >= prefix.len() => {}
                    _ => best = Some((prefix, provider)),
                }
            }
        }
        best.map(|(_, p)| p.clone())
    }

    /// model 名 → provider（最长前缀匹配，无命中默认 chatgpt）。
    pub fn provider_for_model(&self, model: &str) -> String {
        self.provider_for_model_opt(model)
            .unwrap_or_else(|| "chatgpt".to_string())
    }

    /// model 是否受支持（语义收敛）：有 provider_map 前缀命中，或在 `models` 白名单内（大小写不敏感）。
    /// 仅在 `models` 非空时作为准入校验依据（见 consumer）。
    pub fn model_supported(&self, model: &str) -> bool {
        if self.provider_for_model_opt(model).is_some() {
            return true;
        }
        let m = model.to_ascii_lowercase();
        self.models.iter().any(|x| x.to_ascii_lowercase() == m)
    }

    /// model 名 → 每次扣费 credits（最长前缀匹配，fallback default_cost）。
    pub fn cost_for_model(&self, model: &str) -> i64 {
        let m = model.to_ascii_lowercase();
        let mut best: Option<(&String, &i64)> = None;
        for (prefix, cost) in &self.model_cost {
            if m.starts_with(&prefix.to_ascii_lowercase()) {
                match best {
                    Some((bp, _)) if bp.len() >= prefix.len() => {}
                    _ => best = Some((prefix, cost)),
                }
            }
        }
        best.map(|(_, c)| *c).unwrap_or(self.default_cost)
    }

    /// B3：model 名 → 费率倍率（最长前缀匹配，fallback 1.0）。
    pub fn model_multiplier_for(&self, model: &str) -> f64 {
        let m = model.to_ascii_lowercase();
        let mut best: Option<(&String, &f64)> = None;
        for (prefix, mult) in &self.model_multiplier {
            if m.starts_with(&prefix.to_ascii_lowercase()) {
                match best {
                    Some((bp, _)) if bp.len() >= prefix.len() => {}
                    _ => best = Some((prefix, mult)),
                }
            }
        }
        best.map(|(_, c)| *c).unwrap_or(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Config {
        let mut provider_map = HashMap::new();
        provider_map.insert("gpt-".to_string(), "chatgpt".to_string());
        provider_map.insert("claude".to_string(), "claude".to_string());
        provider_map.insert("o3".to_string(), "chatgpt".to_string());
        Config {
            bind: default_bind(),
            worker_tokens: vec!["tok".into()],
            api_keys: vec!["sk".into()],
            queue_wait_secs: 120,
            generation_secs: 180,
            provider_map,
            database_url: None,
            failover_on_offline: false,
            heartbeat_stale_secs: 90,
            default_cooldown_secs: 3600,
            log_to_db: true,
            log_min_level: "info".to_string(),
            log_retention_days: 14,
            log_channel_capacity: 10000,
            admin_token: String::new(),
            allow_open_admin: false,
            metrics_token: String::new(),
            jwt_secret: "test".to_string(),
            allow_registration: true,
            billing_enabled: false,
            default_cost: 1,
            default_initial_balance: 0,
            model_cost: HashMap::new(),
            cost_multiplier: 1.0,
            model_multiplier: HashMap::new(),
            models: Vec::new(),
            admin_username: "admin".to_string(),
            admin_password: String::new(),
            production: false,
            trust_proxy_headers: true,
            allowed_origins: Vec::new(),
            max_body_bytes: default_max_body(),
            public_base_url: String::new(),
            require_email_verify: false,
            smtp: None,
            payment: None,
            worker: WorkerProvisionConfig::default(),
            alert: None,
            security: SecurityConfig::default(),
            image: ImageConfig::default(),
            breaker: BreakerConfig::default(),
            turnstile: None,
        }
    }

    #[test]
    fn maps_model_prefix_to_provider() {
        let c = cfg();
        assert_eq!(c.provider_for_model("gpt-4o"), "chatgpt");
        assert_eq!(c.provider_for_model("GPT-4O"), "chatgpt"); // 大小写不敏感
        assert_eq!(c.provider_for_model("claude-opus-4-8"), "claude");
        assert_eq!(c.provider_for_model("o3-mini"), "chatgpt");
        assert_eq!(c.provider_for_model("unknown-model"), "chatgpt"); // 默认
    }

    #[test]
    fn validates_tokens_and_keys() {
        let c = cfg();
        assert!(c.worker_token_ok("tok"));
        assert!(!c.worker_token_ok("nope"));
        assert!(c.api_key_ok("sk"));
        assert!(!c.api_key_ok("bad"));
        // 多 token 列表：任一命中即放行（常量时间逐项累加，不提前退出）。
        let mut c2 = cfg();
        c2.worker_tokens = vec!["a".into(), "second-token".into()];
        assert!(c2.worker_token_ok("second-token"));
        assert!(!c2.worker_token_ok("third"));
    }

    #[test]
    fn ct_eq_matches_and_rejects() {
        assert!(ct_eq(b"abc", b"abc"));
        assert!(!ct_eq(b"abc", b"abd"));
        assert!(!ct_eq(b"abc", b"abcd")); // 长度不等
        assert!(!ct_eq(b"abc", b""));
        assert!(ct_eq(b"", b""));
    }

    #[test]
    fn prod_validation_rejects_dev_defaults() {
        let mut c = cfg();
        c.production = true;
        c.jwt_secret = "dev-secret-change-me".into();
        c.admin_token = String::new();
        c.api_keys = vec!["sk-sub2org-dev".into()];
        c.worker_tokens = vec!["dev-worker-token".into()];
        c.database_url = None;
        assert!(c.validate_for_prod().is_err()); // 命中多项 → 拒启

        // 全部补成生产值后通过
        c.jwt_secret = "a-very-strong-secret-32bytes-long!!".into();
        c.admin_token = "strong-admin-token".into();
        c.api_keys = vec![];
        c.worker_tokens = vec!["strong-worker-token".into()];
        c.database_url = Some("postgres://x".into());
        assert!(c.validate_for_prod().is_ok());

        // 非生产模式永远放行
        c.production = false;
        c.jwt_secret = "dev-secret-change-me".into();
        assert!(c.validate_for_prod().is_ok());
    }

    #[test]
    fn worker_provision_defaults_and_prod_check() {
        let w = WorkerProvisionConfig::default();
        assert!(w.provision_enabled);
        assert_eq!(w.default_provider, "chatgpt");
        assert_eq!(w.label_prefix, "wk");

        // 生产 + 开启预配 + 无 worker_tokens → 拒启
        let mut c = cfg();
        c.production = true;
        c.jwt_secret = "a-very-strong-secret-32bytes-long!!".into();
        c.admin_token = "strong-admin-token".into();
        c.api_keys = vec![];
        c.database_url = Some("postgres://x".into());
        c.worker_tokens = vec![];
        c.worker = WorkerProvisionConfig::default();
        assert!(c.validate_for_prod().is_err());
        // 配上 worker_token 后通过
        c.worker_tokens = vec!["strong-worker-token".into()];
        assert!(c.validate_for_prod().is_ok());
    }

    #[test]
    fn model_support_and_provider_opt() {
        let mut c = cfg();
        // 无前缀命中 → opt 为 None，但 provider_for_model 仍默认 chatgpt。
        assert_eq!(c.provider_for_model_opt("mystery"), None);
        assert_eq!(c.provider_for_model("mystery"), "chatgpt");
        assert_eq!(c.provider_for_model_opt("gpt-4o").as_deref(), Some("chatgpt"));
        // models 白名单：前缀命中 或 在表内（大小写不敏感）即受支持。
        assert!(c.model_supported("gpt-4o")); // 前缀命中（gpt-）
        assert!(c.model_supported("gpt-5.5-pro")); // 同样前缀命中
        assert!(!c.model_supported("grok-2")); // 无前缀、不在白名单
        c.models = vec!["grok-2".into()];
        assert!(c.model_supported("GROK-2")); // 白名单（忽略大小写）
    }

    #[test]
    fn prod_rejects_empty_admin_password_on_memory() {
        let mut c = cfg();
        c.production = true;
        c.jwt_secret = "a-very-strong-secret-32bytes-long!!".into();
        c.admin_token = "strong-admin-token".into();
        c.api_keys = vec![];
        c.worker_tokens = vec!["strong-worker-token".into()];
        c.worker = WorkerProvisionConfig::default();
        c.database_url = Some("postgres://x".into());
        c.admin_password = String::new();
        assert!(c.validate_for_prod().is_ok()); // Postgres + 空密码：首启生成一次并持久，可
        // 切回内存后端 + 空密码 → 拒启（每次重启改密）。
        c.database_url = None;
        assert!(c.validate_for_prod().is_err());
        // 配上固定密码后（仍内存）→ admin 这条不再报，但 database_url 空仍会报（生产必须 Postgres）。
        c.admin_password = "fixed-admin-pw".into();
        assert!(c.validate_for_prod().is_err());
    }

    #[test]
    fn cost_for_model_prefix() {
        let mut c = cfg();
        c.default_cost = 1;
        c.model_cost.insert("gpt-4".into(), 5);
        c.model_cost.insert("gpt-4o".into(), 3);
        assert_eq!(c.cost_for_model("gpt-4o-mini"), 3); // 最长前缀
        assert_eq!(c.cost_for_model("gpt-4-turbo"), 5);
        assert_eq!(c.cost_for_model("claude-x"), 1); // 默认
    }

    #[test]
    fn model_multiplier_prefix_and_default() {
        let mut c = cfg();
        c.cost_multiplier = 2.0; // 全局 ×2
        c.model_multiplier.insert("gpt-4".into(), 1.5);
        c.model_multiplier.insert("gpt-4o".into(), 3.0);
        assert_eq!(c.model_multiplier_for("gpt-4o-mini"), 3.0); // 最长前缀
        assert_eq!(c.model_multiplier_for("gpt-4-turbo"), 1.5);
        assert_eq!(c.model_multiplier_for("claude-x"), 1.0); // 默认
    }
}

mod account;
mod alert;
mod billing;
mod config;
mod http;
mod logging;
mod metrics;
mod payment;
mod pool;
mod state;
mod store;
mod stream;
mod ws;

use std::sync::Arc;

use axum::http::{HeaderValue, Method};
use socketioxide::SocketIo;
use tower_http::cors::{Any, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

use crate::account::store::{AccountStore, InMemoryAccountStore, PgAccountStore};
use crate::account::AccountService;
use crate::logging::{level_rank, InMemoryLogStore, LogSink, LogStore, PgLogStore};
use crate::store::{InMemoryStore, PgStore, Store};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1) config（env 覆盖敏感项 → 生产模式拒绝 dev 默认值）
    let config_path = std::env::var("SUB2ORG_CONFIG").unwrap_or_else(|_| "config.toml".to_string());
    let mut config = config::Config::load(&config_path)?;
    config.apply_env_overrides();
    config.validate_for_prod()?;
    // IP allow/deny 预编译（非法 CIDR → 启动期 fail-fast）。
    let ip_rules = http::ipfilter::IpRuleSet::build(&config.security.ip_allow, &config.security.ip_deny)?;
    let config = Arc::new(config);
    let use_pg = config
        .database_url
        .as_deref()
        .map(|u| !u.is_empty())
        .unwrap_or(false);

    // 2) 中转 store（Postgres 在此跑全部 migrations，含 users/billing 的 0004）
    let store: Arc<dyn Store> = if use_pg {
        let url = config.database_url.clone().unwrap();
        Arc::new(PgStore::connect(&url, &config.worker_tokens).await?)
    } else {
        Arc::new(InMemoryStore::from_config(&config.worker_tokens))
    };

    // 3) 账户 store（表已由上一步迁移建好）
    let account_store: Arc<dyn AccountStore> = if use_pg {
        Arc::new(PgAccountStore::connect(config.database_url.as_deref().unwrap()).await?)
    } else {
        Arc::new(InMemoryAccountStore::new())
    };
    let account = AccountService::new(account_store, config.clone());

    // 4) 日志存储
    let log_store: Arc<dyn LogStore> = if use_pg && config.log_to_db {
        Arc::new(PgLogStore::connect(config.database_url.as_deref().unwrap()).await?)
    } else {
        Arc::new(InMemoryLogStore::new(5000))
    };
    let (log_sink, log_rx) =
        LogSink::new(config.log_channel_capacity, level_rank(&config.log_min_level));
    logging::spawn_flusher(log_rx, log_store.clone());
    logging::spawn_retention(log_store.clone(), config.log_retention_days);

    // 5) tracing：控制台 + 入库 layer
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,sub2org=debug")),
        )
        .with(tracing_subscriber::fmt::layer())
        .with(logging::layer::LogChannelLayer::new(log_sink.clone()))
        .init();

    // 6) bootstrap admin + 把 config 种子 api_keys 归属 admin（保证纯 api_key 调用不破）
    if let Some((u, pw)) = account.ensure_admin().await? {
        if pw.is_empty() {
            tracing::info!(category = "system", username = %u, "已创建 admin 用户（config 密码）");
        } else {
            tracing::warn!(category = "system", username = %u, password = %pw, "已创建 admin 用户（随机密码，仅此一次）");
            // 内存后端 + 空 admin_password：随机密码不持久，每次重启都会变 → 提示固定办法。
            if !use_pg {
                tracing::warn!(
                    category = "system",
                    "内存后端下随机 admin 密码不持久，每次重启都会变；请设 SUB2ORG_ADMIN_PASSWORD 或配 database_url(Postgres)"
                );
            }
        }
    }
    if let Some(admin) = account.store.get_user_by_username(&config.admin_username).await {
        for raw in &config.api_keys {
            let _ = account.store.seed_api_key(&admin.id, raw, "config").await;
        }
    }

    if use_pg {
        tracing::info!(category = "system", store = "postgres", "sub2org starting");
    } else {
        tracing::warn!(
            category = "system",
            store = "memory",
            "⚠️ 内存模式：重启将丢失全部用户/API key/会话/计费/审计。对外运营请配 database_url(Postgres)"
        );
    }

    let bind = config.bind.clone();

    // 7) 启动钩子内一次性构造共享状态
    // ping_interval 20s（< MV3 service worker 30s 空闲回收线，靠 WS 活动保活 SW）；
    // ping_timeout 60s（容忍被节流的后台 SW，减少误判超时断连）。死连检测 20+60=80s < heartbeat_stale 90s。
    let (layer, io) = SocketIo::builder()
        .ping_interval(std::time::Duration::from_secs(20))
        .ping_timeout(std::time::Duration::from_secs(60))
        .build_layer();
    let state = Arc::new(state::AppState::new(
        config, io.clone(), store, log_sink, log_store, account, ip_rules,
    ));
    ws::gateway::register(&io, state.clone());
    pool::health::spawn(state.clone());
    alert::spawn(state.clone());

    // CORS：配了 allowed_origins → 收紧白名单；否则放行所有（dev / 同域反代）。
    let cors = if state.config.allowed_origins.is_empty() {
        CorsLayer::permissive()
    } else {
        let origins: Vec<HeaderValue> = state
            .config
            .allowed_origins
            .iter()
            .filter_map(|o| o.parse::<HeaderValue>().ok())
            .collect();
        CorsLayer::new()
            .allow_origin(origins)
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::PATCH, Method::DELETE, Method::OPTIONS])
            .allow_headers(Any)
    };

    let app = http::router(state.clone())
        .layer(layer)
        .layer(cors)
        .layer(RequestBodyLimitLayer::new(state.config.max_body_bytes))
        // axum 的 Json/Bytes 提取器自带 DefaultBodyLimit(2MiB)，会先于 RequestBodyLimitLayer 拒绝带图大请求；
        // 抬到同一上限，让 max_body_bytes 成为唯一权威限制。
        .layer(axum::extract::DefaultBodyLimit::max(state.config.max_body_bytes))
        // IP allow/deny 置于最外层：先于 CORS/socket.io 执行，覆盖 /socket.io 与 OPTIONS 预检。
        .layer(axum::middleware::from_fn_with_state(state.clone(), http::ipfilter::ip_filter));

    state.config.warn_exposure(); // P0#2/P1#5：直连暴露 + dev 默认值/可伪造头的醒目告警
    state.config.warn_config_consistency(); // P2#11：跨配置一致性告警

    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!(category = "system", "sub2org listening on http://{bind}");
    // P1#5：into_make_service_with_connect_info 让中间件能取 socket peer IP（trust_proxy_headers=false 时用它）。
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    tracing::info!(category = "system", "sub2org shut down cleanly");
    Ok(())
}

/// 等待 SIGTERM / Ctrl-C：触发后停止收新连接，让在途请求自然结束。
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = term => {}
    }
    tracing::info!(category = "system", "shutdown signal received, draining in-flight requests…");
}

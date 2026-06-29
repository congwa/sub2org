//! IP allow/deny 过滤。启动时把 CIDR 预编译成 IpRule，运行期 O(rules) 位掩码比较。
//! XFF 信任假设：仅在 nginx 反代后才可信（client_ip 取 XFF 第一跳）。直连暴露时务必清空 ip_allow/ip_deny。

use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

use crate::http::ratelimit_mw::client_ip;
use crate::state::AppState;

/// 一条预编译 CIDR 规则。
#[derive(Debug, Clone)]
pub enum IpRule {
    V4 { net: u32, mask: u32 },
    V6 { net: u128, mask: u128 },
}

impl IpRule {
    /// 解析 "10.0.0.0/8" / "2001:db8::/32" / 裸 "1.2.3.4"(=/32) / "::1"(=/128)。
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        let s = s.trim();
        let (addr_part, prefix_part) = match s.split_once('/') {
            Some((a, p)) => (a, Some(p)),
            None => (s, None),
        };
        let addr = IpAddr::from_str(addr_part).map_err(|_| anyhow::anyhow!("非法 IP/CIDR: {s}"))?;
        match addr {
            IpAddr::V4(v4) => {
                let bits: u8 = match prefix_part {
                    Some(p) => p.parse().map_err(|_| anyhow::anyhow!("非法前缀: {s}"))?,
                    None => 32,
                };
                if bits > 32 {
                    anyhow::bail!("IPv4 前缀越界(>32): {s}");
                }
                let mask = if bits == 0 { 0u32 } else { u32::MAX << (32 - bits) };
                Ok(IpRule::V4 { net: u32::from(v4) & mask, mask })
            }
            IpAddr::V6(v6) => {
                let bits: u8 = match prefix_part {
                    Some(p) => p.parse().map_err(|_| anyhow::anyhow!("非法前缀: {s}"))?,
                    None => 128,
                };
                if bits > 128 {
                    anyhow::bail!("IPv6 前缀越界(>128): {s}");
                }
                let mask = if bits == 0 { 0u128 } else { u128::MAX << (128 - bits) };
                Ok(IpRule::V6 { net: u128::from(v6) & mask, mask })
            }
        }
    }

    pub fn contains(&self, ip: &IpAddr) -> bool {
        match (self, ip) {
            (IpRule::V4 { net, mask }, IpAddr::V4(a)) => (u32::from(*a) & mask) == *net,
            (IpRule::V6 { net, mask }, IpAddr::V6(a)) => (u128::from(*a) & mask) == *net,
            // IPv4-mapped IPv6 (::ffff:1.2.3.4) 归一化后再比 v4 规则。
            (IpRule::V4 { .. }, IpAddr::V6(a)) => {
                a.to_ipv4_mapped().is_some_and(|v4| self.contains(&IpAddr::V4(v4)))
            }
            (IpRule::V6 { .. }, IpAddr::V4(_)) => false,
        }
    }
}

/// 预编译规则集合，存入 AppState。allow 非空 = allowlist 模式。
#[derive(Debug, Clone, Default)]
pub struct IpRuleSet {
    pub allow: Vec<IpRule>,
    pub deny: Vec<IpRule>,
}

impl IpRuleSet {
    /// 从字符串 CIDR 列表构建；任一非法 → Err（启动期 fail-fast）。
    /// allow 非空时自动追加回环（127.0.0.1/32、::1/128），避免本机探针被锁。
    pub fn build(allow_raw: &[String], deny_raw: &[String]) -> anyhow::Result<Self> {
        let parse_all = |v: &[String]| -> anyhow::Result<Vec<IpRule>> {
            v.iter().map(|s| IpRule::parse(s)).collect()
        };
        let mut allow = parse_all(allow_raw)?;
        if !allow.is_empty() {
            allow.push(IpRule::parse("127.0.0.1/32").unwrap());
            allow.push(IpRule::parse("::1/128").unwrap());
        }
        Ok(Self { allow, deny: parse_all(deny_raw)? })
    }

    /// true = 放行。deny 优先拦截；allow 非空时仅命中 allow 放行。
    pub fn permits(&self, ip: &IpAddr) -> bool {
        if self.deny.iter().any(|r| r.contains(ip)) {
            return false;
        }
        if self.allow.is_empty() {
            return true;
        }
        self.allow.iter().any(|r| r.contains(ip))
    }

    pub fn is_empty(&self) -> bool {
        self.allow.is_empty() && self.deny.is_empty()
    }
}

fn deny_response() -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(json!({"error": {"message": "访问被拒绝（IP 不在允许范围内）", "type": "forbidden"}})),
    )
        .into_response()
}

/// 全局 IP 过滤中间件。空规则集 → 直接放行（零开销快路径）。
pub async fn ip_filter(State(state): State<Arc<AppState>>, req: Request, next: Next) -> Response {
    // 健康探针永不过滤：编排器/直连探针无 XFF（会被 allowlist 误拒），且不含敏感数据。
    if matches!(req.uri().path(), "/health" | "/healthz" | "/livez" | "/readyz") {
        return next.run(req).await;
    }
    let rules = &state.ip_rules;
    if rules.is_empty() {
        return next.run(req).await;
    }
    let ip_str = client_ip(&req, state.config.trust_proxy_headers);
    match ip_str.parse::<IpAddr>() {
        Ok(ip) if rules.permits(&ip) => next.run(req).await,
        Ok(_) => deny_response(),
        // "direct"(无 XFF/Real-IP) 或解析失败：allowlist 模式拒，否则放行。
        Err(_) => {
            if rules.allow.is_empty() {
                next.run(req).await
            } else {
                deny_response()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn parse_rejects_garbage() {
        assert!(IpRule::parse("10.0.0.0/33").is_err());
        assert!(IpRule::parse("999.1.1.1").is_err());
        assert!(IpRule::parse("notanip").is_err());
    }

    #[test]
    fn allow_deny_semantics() {
        let s = IpRuleSet::build(&["10.0.0.0/8".into()], &["10.1.2.0/24".into()]).unwrap();
        assert!(s.permits(&ip("10.5.5.5"))); // in allow
        assert!(!s.permits(&ip("10.1.2.7"))); // deny wins over allow
        assert!(!s.permits(&ip("8.8.8.8"))); // not in allow
        assert!(s.permits(&ip("127.0.0.1"))); // 回环自动放行
        let empty = IpRuleSet::build(&[], &[]).unwrap();
        assert!(empty.permits(&ip("8.8.8.8"))); // 空 = 全放行
    }

    #[test]
    fn v6_and_mapped() {
        let s = IpRuleSet::build(&["2001:db8::/32".into()], &[]).unwrap();
        assert!(s.permits(&ip("2001:db8::1")));
        assert!(!s.permits(&ip("2001:db9::1")));
        let d = IpRuleSet::build(&[], &["1.2.3.4/32".into()]).unwrap();
        assert!(!d.permits(&ip("::ffff:1.2.3.4"))); // IPv4-mapped 命中 v4 deny
    }
}

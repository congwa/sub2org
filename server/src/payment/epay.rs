//! 彩虹易支付 / epay 兼容协议：签名、验签、构造跳转支付 URL。
//!
//! 签名算法（业界 epay 约定）：参数（剔除 `sign`/`sign_type`/空值）按键名 ASCII 升序拼成
//! `a=1&b=2&…`（**值不 URL 编码**），末尾**直接**拼接商户密钥 KEY，再 MD5 取小写十六进制。
//! 注意：URL 里 query 的值需要百分号编码，但签名是对**原始值**计算的——两者分开处理。

use std::collections::BTreeMap;

use md5::{Digest, Md5};

/// 计算 epay 签名（小写 md5）。`params` 用 BTreeMap 天然按键名升序。
pub fn sign(params: &BTreeMap<String, String>, key: &str) -> String {
    let mut buf = String::new();
    for (k, v) in params {
        if k == "sign" || k == "sign_type" || v.is_empty() {
            continue;
        }
        if !buf.is_empty() {
            buf.push('&');
        }
        buf.push_str(k);
        buf.push('=');
        buf.push_str(v);
    }
    buf.push_str(key);
    let mut h = Md5::new();
    h.update(buf.as_bytes());
    hex::encode(h.finalize())
}

/// 校验回调签名：取 params 里的 `sign` 与重算结果比对（大小写不敏感）。
pub fn verify(params: &BTreeMap<String, String>, key: &str) -> bool {
    let provided = params.get("sign").cloned().unwrap_or_default();
    if provided.is_empty() {
        return false;
    }
    provided.eq_ignore_ascii_case(&sign(params, key))
}

/// 一次下单的入参（页面跳转 submit.php 方式）。
pub struct PayRequest<'a> {
    /// 网关基址，如 https://pay.example.com （末尾有无 / 都可）。
    pub gateway: &'a str,
    pub pid: &'a str,
    pub key: &'a str,
    pub out_trade_no: &'a str,
    /// epay `type`：alipay / wxpay / qqpay …
    pub channel: &'a str,
    pub name: &'a str,
    /// 金额（元），形如 "1.00"。
    pub money: &'a str,
    pub notify_url: &'a str,
    pub return_url: &'a str,
}

/// 构造跳转支付 URL（GET submit.php?…）。值做百分号编码，但签名按原始值计算。
pub fn build_submit_url(r: &PayRequest) -> String {
    let mut p: BTreeMap<String, String> = BTreeMap::new();
    p.insert("pid".into(), r.pid.to_string());
    p.insert("type".into(), r.channel.to_string());
    p.insert("out_trade_no".into(), r.out_trade_no.to_string());
    p.insert("notify_url".into(), r.notify_url.to_string());
    p.insert("return_url".into(), r.return_url.to_string());
    p.insert("name".into(), r.name.to_string());
    p.insert("money".into(), r.money.to_string());
    let signature = sign(&p, r.key);
    p.insert("sign".into(), signature);
    p.insert("sign_type".into(), "MD5".into());

    let base = r.gateway.trim_end_matches('/');
    let query: Vec<String> = p
        .iter()
        .map(|(k, v)| format!("{}={}", k, pct_encode(v)))
        .collect();
    format!("{base}/submit.php?{}", query.join("&"))
}

/// 最小百分号编码：保留 unreserved（A-Za-z0-9-_.~），其余按 UTF-8 字节编码。
pub fn pct_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_skips_sign_and_empty_then_appends_key() {
        let mut p = BTreeMap::new();
        p.insert("pid".to_string(), "1001".to_string());
        p.insert("money".to_string(), "1.00".to_string());
        p.insert("name".to_string(), "credits".to_string());
        p.insert("empty".to_string(), "".to_string()); // 空值剔除
        p.insert("sign_type".to_string(), "MD5".to_string()); // 剔除
        // 期望明文：money=1.00&name=credits&pid=1001 + KEY
        let expected_plain = "money=1.00&name=credits&pid=1001KEY";
        let mut h = Md5::new();
        h.update(expected_plain.as_bytes());
        let expected = hex::encode(h.finalize());
        assert_eq!(sign(&p, "KEY"), expected);
    }

    #[test]
    fn verify_roundtrip() {
        let mut p = BTreeMap::new();
        p.insert("pid".to_string(), "1001".to_string());
        p.insert("out_trade_no".to_string(), "s2o-abc".to_string());
        p.insert("trade_status".to_string(), "TRADE_SUCCESS".to_string());
        p.insert("money".to_string(), "2.50".to_string());
        let s = sign(&p, "secret-key");
        p.insert("sign".to_string(), s);
        p.insert("sign_type".to_string(), "MD5".to_string());
        assert!(verify(&p, "secret-key"));
        assert!(!verify(&p, "wrong-key"));
    }

    #[test]
    fn pct_encode_encodes_reserved() {
        assert_eq!(pct_encode("https://a.com/x?y=1"), "https%3A%2F%2Fa.com%2Fx%3Fy%3D1");
        assert_eq!(pct_encode("abc-_.~"), "abc-_.~");
    }
}

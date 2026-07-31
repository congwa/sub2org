//! 在线支付（易支付/epay）。下单链路 + 回调验签在 `http::payment`，纯协议在 `epay`。

pub mod epay;

/// 把积分金额换算成"元"字符串（如 amount=100, credits_per_cny=100 → "1.00"）。
pub fn money_str(amount_credits: i64, credits_per_cny: i64) -> String {
    let cpc = credits_per_cny.max(1);
    format!("{:.2}", amount_credits as f64 / cpc as f64)
}

/// 把"元"金额字符串解析成分（fen，整数）。容忍 "10" / "10.0" / "10.00" 等不同格式，
/// 负数/非法返回 None。#8：回调金额校验改用数值比较，避免网关把 "10.00" 规整成 "10.0"
/// 导致严格字符串比对失败 → 真金已付却永远入不了账（无需改配置即触发的丢钱路径）。
pub fn money_to_fen(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let (int_part, frac_part) = match s.split_once('.') {
        Some((a, b)) => (a, b),
        None => (s, ""),
    };
    if int_part.is_empty() && frac_part.is_empty() {
        return None;
    }
    if !int_part.chars().all(|c| c.is_ascii_digit()) || !frac_part.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let yuan: i64 = if int_part.is_empty() { 0 } else { int_part.parse().ok()? };
    // 取两位小数（fen）：不足补 0，多余截断（金额只到分）。
    let mut frac = frac_part.to_string();
    while frac.len() < 2 {
        frac.push('0');
    }
    let fen_frac: i64 = frac[..2].parse().ok()?;
    yuan.checked_mul(100)?.checked_add(fen_frac)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_formatting() {
        assert_eq!(money_str(100, 100), "1.00");
        assert_eq!(money_str(250, 100), "2.50");
        assert_eq!(money_str(1, 1), "1.00");
        assert_eq!(money_str(0, 100), "0.00");
    }

    #[test]
    fn money_to_fen_tolerates_formats() {
        // #8：同一金额的不同字符串表示都解析成相同 fen（网关规整不再导致误判）。
        assert_eq!(money_to_fen("10.00"), Some(1000));
        assert_eq!(money_to_fen("10.0"), Some(1000));
        assert_eq!(money_to_fen("10"), Some(1000));
        assert_eq!(money_to_fen("2.50"), Some(250));
        assert_eq!(money_to_fen("2.5"), Some(250));
        assert_eq!(money_to_fen("0.01"), Some(1));
        assert_eq!(money_to_fen(".5"), Some(50));
        // 非法/负数拒绝
        assert_eq!(money_to_fen(""), None);
        assert_eq!(money_to_fen("-1.00"), None);
        assert_eq!(money_to_fen("abc"), None);
        assert_eq!(money_to_fen("1.2.3"), None);
    }
}

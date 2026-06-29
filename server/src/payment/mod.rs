//! 在线支付（易支付/epay）。下单链路 + 回调验签在 `http::payment`，纯协议在 `epay`。

pub mod epay;

/// 把积分金额换算成"元"字符串（如 amount=100, credits_per_cny=100 → "1.00"）。
pub fn money_str(amount_credits: i64, credits_per_cny: i64) -> String {
    let cpc = credits_per_cny.max(1);
    format!("{:.2}", amount_credits as f64 / cpc as f64)
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
}

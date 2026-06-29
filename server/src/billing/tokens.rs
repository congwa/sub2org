//! 轻量 token 估算（A2）。
//! 网页 UI 中继拿不到上游真实 token，故用启发式而非引 tokenizer——对未知模型套 BPE 是伪精度，
//! 还会引入重依赖（与项目「只用 ring、不引 openssl/aws-lc/cmake」的依赖姿态相悖）。估算仅用于 `usage` 上报。

/// 估算一段文本的 token 数：chars/4 与词数取大（兜住 CJK / 无空格长串），非空至少 1。
pub fn estimate_tokens(text: &str) -> u32 {
    if text.is_empty() {
        return 0;
    }
    let chars = text.chars().count();
    let words = text.split_whitespace().filter(|w| !w.is_empty()).count();
    let by_chars = ((chars + 3) / 4) as u32; // chars/4 向上取整（OpenAI 经验法则）
    let by_words = words as u32;
    by_chars.max(by_words).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimates_reasonably() {
        assert_eq!(estimate_tokens(""), 0);
        assert!(estimate_tokens("hi") >= 1);
        assert!(estimate_tokens("hello world") >= 2);
        assert!(estimate_tokens("the quick brown fox jumps over") >= 6);
        // CJK：无空格，靠 chars/4 兜底（不会退化成 1）
        assert!(estimate_tokens("你好世界你好世界你好世界") >= 3);
        // 代码
        assert!(estimate_tokens("fn main() { println!(\"hi\"); }") >= 5);
    }
}

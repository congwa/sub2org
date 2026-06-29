# ---- builder ----
FROM rust:1-slim AS builder
WORKDIR /build
# build-essential：ring/rustls(lettre 走 SMTP TLS) 需要 C 编译器；pkg-config 备用。
RUN apt-get update && apt-get install -y --no-install-recommends build-essential pkg-config && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml ./
COPY Cargo.lock* ./
COPY src ./src
COPY migrations ./migrations
# sqlx 运行期查询 + migrate! 编译期嵌入迁移（无需编译期 DB）。
RUN cargo build --release

# ---- runtime ----
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /build/target/release/sub2org /usr/local/bin/sub2org
# 配置通过挂载 /app/config.toml 提供；敏感项走环境变量覆盖（见 config.example.toml）。
EXPOSE 3300
CMD ["sub2org"]

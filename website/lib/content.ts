// 业务职责：集中维护官网双语文案与页面结构，确保中英文切换只改变业务内容，不分叉页面实现。
export const locales = ["zh", "en"] as const;

export type Locale = (typeof locales)[number];

export type SiteDictionary = {
  languageName: string;
  alternateLanguageName: string;
  meta: {
    title: string;
    description: string;
  };
  nav: {
    product: string;
    flow: string;
    ops: string;
    docs: string;
  };
  hero: {
    eyebrow: string;
    title: string;
    lead: string;
    primaryCta: string;
    secondaryCta: string;
    proof: string[];
    codeLabel: string;
    imageAlt: string;
  };
  protocol: {
    title: string;
    intro: string;
    steps: Array<{
      label: string;
      value: string;
    }>;
  };
  features: {
    title: string;
    kicker: string;
    items: Array<{
      icon: "Cable" | "Gauge" | "ShieldCheck" | "ReceiptText" | "RadioTower" | "Workflow";
      title: string;
      body: string;
    }>;
  };
  workflow: {
    title: string;
    intro: string;
    steps: Array<{
      number: string;
      title: string;
      body: string;
    }>;
  };
  operations: {
    title: string;
    intro: string;
    metrics: Array<{
      value: string;
      label: string;
    }>;
    panels: Array<{
      title: string;
      body: string;
    }>;
  };
  footer: {
    tagline: string;
    repo: string;
    switchLanguage: string;
  };
};

export const dictionaries: Record<Locale, SiteDictionary> = {
  zh: {
    languageName: "中文",
    alternateLanguageName: "English",
    meta: {
      title: "sub2org - OpenAI-compatible Browser Relay",
      description:
        "sub2org 把浏览器里的 ChatGPT/Codex 会话池包装成 OpenAI-compatible relay API，支持 worker 调度、真流式、会话亲和、计费与日志。"
    },
    nav: {
      product: "产品",
      flow: "链路",
      ops: "运维",
      docs: "Docs"
    },
    hero: {
      eyebrow: "Browser sessions as infrastructure",
      title: "把 Browser Workers 变成 OpenAI-compatible Relay API",
      lead:
        "sub2org 将多台指纹浏览器、扩展 worker 和已登录的网页 AI 账号编排成一个可调度的 API relay。消费者仍然调用 /v1/chat/completions，背后由 worker pool 完成 dispatch、streaming、session affinity 与 failure handling。",
      primaryCta: "查看使用文档",
      secondaryCta: "打开 GitHub",
      proof: [
        "Rust control center",
        "Socket.IO worker protocol",
        "SSE token streaming",
        "Billing-ready console"
      ],
      codeLabel: "OpenAI-compatible request",
      imageAlt:
        "sub2org relay control surface showing API streams routed to browser worker sessions"
    },
    protocol: {
      title: "A relay control plane, not another model wrapper",
      intro:
        "请求从消费者 API 进入，调度器挑选 idle worker，扩展在网页端完成 prompt 输入，并把 chunk 逐字回传为标准 SSE。",
      steps: [
        { label: "Client", value: "OpenAI SDK / curl" },
        { label: "Server", value: "auth, billing, scheduler" },
        { label: "Worker", value: "browser extension + logged-in web AI" },
        { label: "Stream", value: "chat.completion.chunk" }
      ]
    },
    features: {
      title: "Built for operators who run many browser accounts",
      kicker: "把实验性浏览器自动化，收束成可观测、可限流、可计费的 API 产品。",
      items: [
        {
          icon: "Workflow",
          title: "Worker pool scheduling",
          body:
            "每台云机器以 worker 身份接入，调度器按状态、并发槽和 queue wait 选择可用节点，避免单账号成为瓶颈。"
        },
        {
          icon: "Cable",
          title: "Session affinity",
          body:
            "X-Sub2org-Session 能把后续请求钉回同一个 worker 与网页 conversation，让长上下文业务有连续性。"
        },
        {
          icon: "RadioTower",
          title: "True streaming bridge",
          body:
            "content loader 捕获网页 SSE，background 通过 Socket.IO 回传，server 重新封装为 OpenAI chunk。"
        },
        {
          icon: "ShieldCheck",
          title: "Auth and role surfaces",
          body:
            "消费 API key、用户 JWT、admin token 三套入口分层管理，适合把 relay 暴露给多个内部或外部用户。"
        },
        {
          icon: "ReceiptText",
          title: "Pre-deduct billing",
          body:
            "按 model 成本预扣，成功 commit，失败 refund；默认可关闭，方便从私有工具平滑进化到付费服务。"
        },
        {
          icon: "Gauge",
          title: "Live operations console",
          body:
            "日志、metrics、worker 状态和 live events 进入 console，让排队、掉线、限流和登录失效都能被看见。"
        }
      ]
    },
    workflow: {
      title: "从一个 API call 到网页 AI 的完整路径",
      intro:
        "sub2org 的关键不是替代模型，而是把浏览器会话池包装成稳定的 OpenAI-compatible serving layer。",
      steps: [
        {
          number: "01",
          title: "Consumer request",
          body:
            "客户端带 sk-sub2org API key 请求 /v1/chat/completions，server 完成鉴权、限流、余额检查与会话解析。"
        },
        {
          number: "02",
          title: "Scheduler dispatch",
          body:
            "pool registry 只选择 idle/capacity 可用的 worker；如果 session 已绑定，则优先回到原 worker 与 conversation。"
        },
        {
          number: "03",
          title: "Browser execution",
          body:
            "MV3 扩展管理标签页，content loader 注入页面 JS world，劫持 window.fetch 并解析网页 AI 的增量输出。"
        },
        {
          number: "04",
          title: "OpenAI-style response",
          body:
            "server 按 req_id 关联 chunk/done/error，把 worker 输出转换为 chat.completion.chunk 或非流式聚合结果。"
        }
      ]
    },
    operations: {
      title: "Production-minded, single-instance by design",
      intro:
        "当前架构明确选择单 sub2org 实例与内存实时状态，Postgres 只持久化身份、账号、账单和日志，降低运维复杂度。",
      metrics: [
        { value: "M1-M3", label: "relay milestones done" },
        { value: "24h", label: "request ring metrics" },
        { value: "10", label: "log categories" },
        { value: "3", label: "auth surfaces" }
      ],
      panels: [
        {
          title: "Fast failure",
          body:
            "worker 断连或心跳超时会立即 fail inflight request，而不是让消费者等 generation timeout。"
        },
        {
          title: "Store abstraction",
          body:
            "无 database_url 时用 in-memory dev store；配置 Postgres 后自动迁移并持久化 users、keys、sessions 与 logs。"
        },
        {
          title: "Console-first visibility",
          body:
            "admin/events 使用 SSE 推送 live events，日志 sink 非阻塞批量写入，relay 热路径不会被日志系统拖慢。"
        }
      ]
    },
    footer: {
      tagline: "sub2org keeps browser AI sessions operational, observable, and API-shaped.",
      repo: "GitHub Repository",
      switchLanguage: "Switch to English"
    }
  },
  en: {
    languageName: "English",
    alternateLanguageName: "中文",
    meta: {
      title: "sub2org - Browser AI sessions as an OpenAI-compatible API",
      description:
        "sub2org turns a pool of browser-driven ChatGPT/Codex sessions into an OpenAI-compatible relay API with scheduling, streaming, billing, logging, and a web console."
    },
    nav: {
      product: "Product",
      flow: "Flow",
      ops: "Ops",
      docs: "文档"
    },
    hero: {
      eyebrow: "浏览器会话池, served like infrastructure",
      title: "Turn browser workers into an OpenAI-compatible Relay API",
      lead:
        "sub2org coordinates fingerprint browsers, extension workers, and logged-in web AI accounts behind /v1/chat/completions. The consumer sees a familiar API; the control plane handles dispatch, streaming, session affinity, billing hooks, and fast failure.",
      primaryCta: "Read the docs",
      secondaryCta: "Open GitHub",
      proof: [
        "Rust control center",
        "Socket.IO worker protocol",
        "SSE token streaming",
        "Billing-ready console"
      ],
      codeLabel: "OpenAI-compatible request",
      imageAlt:
        "sub2org relay control surface showing API streams routed to browser worker sessions"
    },
    protocol: {
      title: "A control plane for web AI accounts",
      intro:
        "Requests enter through the consumer API, the scheduler chooses an idle worker, the extension executes in the browser UI, and token chunks return as standard SSE.",
      steps: [
        { label: "Client", value: "OpenAI SDK / curl" },
        { label: "Server", value: "auth, billing, scheduler" },
        { label: "Worker", value: "browser extension + logged-in web AI" },
        { label: "Stream", value: "chat.completion.chunk" }
      ]
    },
    features: {
      title: "Built for teams operating many browser accounts",
      kicker:
        "A practical layer that turns fragile browser automation into observable, rate-limited, billable API infrastructure.",
      items: [
        {
          icon: "Workflow",
          title: "Worker pool scheduling",
          body:
            "Cloud machines connect as workers. The scheduler picks healthy capacity by status, semaphore slots, and queue wait instead of overloading one account."
        },
        {
          icon: "Cable",
          title: "Session affinity",
          body:
            "X-Sub2org-Session pins follow-up calls to the same worker and native conversation, keeping long-running business context intact."
        },
        {
          icon: "RadioTower",
          title: "True streaming bridge",
          body:
            "The content loader captures web SSE, the background worker relays over Socket.IO, and the server reframes tokens as OpenAI chunks."
        },
        {
          icon: "ShieldCheck",
          title: "Auth and role surfaces",
          body:
            "Consumer API keys, user JWTs, and admin access are separated so the relay can serve internal teams or external customers."
        },
        {
          icon: "ReceiptText",
          title: "Pre-deduct billing",
          body:
            "Model-prefix costs reserve balance before dispatch, commit on success, and refund on failure; billing can stay off for private deployments."
        },
        {
          icon: "Gauge",
          title: "Live operations console",
          body:
            "Metrics, logs, worker state, and live events expose queueing, disconnects, cooling, logout, and rate-limit behavior."
        }
      ]
    },
    workflow: {
      title: "From one API call to a browser AI session",
      intro:
        "sub2org does not pretend to be a model. It shapes browser sessions into a stable OpenAI-compatible serving layer.",
      steps: [
        {
          number: "01",
          title: "Consumer request",
          body:
            "The client sends a sk-sub2org API key to /v1/chat/completions. The server checks auth, rate limits, balance, and session routing."
        },
        {
          number: "02",
          title: "Scheduler dispatch",
          body:
            "The registry only selects dispatchable workers. Existing sessions route back to the original worker and browser conversation when possible."
        },
        {
          number: "03",
          title: "Browser execution",
          body:
            "The MV3 extension manages tabs, injects a loader into the page JS world, and captures incremental output from the web AI UI."
        },
        {
          number: "04",
          title: "OpenAI-style response",
          body:
            "The server correlates chunk/done/error by req_id and returns chat.completion.chunk frames or an aggregated non-streaming response."
        }
      ]
    },
    operations: {
      title: "Production-minded, intentionally single-instance",
      intro:
        "Live presence stays in memory inside one sub2org server; Postgres persists identity, accounts, billing, sessions, and logs without adding Redis complexity.",
      metrics: [
        { value: "M1-M3", label: "relay milestones done" },
        { value: "24h", label: "request ring metrics" },
        { value: "10", label: "log categories" },
        { value: "3", label: "auth surfaces" }
      ],
      panels: [
        {
          title: "Fast failure",
          body:
            "Worker disconnects and heartbeat timeouts fail in-flight requests immediately instead of waiting for generation timeout."
        },
        {
          title: "Store abstraction",
          body:
            "Run in memory for dev. Add database_url for Postgres migrations and persistence across users, keys, sessions, and logs."
        },
        {
          title: "Console-first visibility",
          body:
            "admin/events streams live updates, while the non-blocking log sink batches writes without slowing the relay hot path."
        }
      ]
    },
    footer: {
      tagline: "sub2org keeps browser AI sessions operational, observable, and API-shaped.",
      repo: "GitHub Repository",
      switchLanguage: "切换到中文"
    }
  }
};

/**
 * 业务职责：校验 URL 中的语言标识是否为官网支持的市场版本，避免未知路径使用错误文案上线。
 */
export function isLocale(value: string): value is Locale {
  return locales.includes(value as Locale);
}

/**
 * 业务职责：按语言取出完整官网文案，保证页面渲染、SEO 和导航使用同一份市场叙事。
 */
export function getDictionary(locale: Locale): SiteDictionary {
  return dictionaries[locale];
}

/**
 * 业务职责：提供语言切换目标，让用户在中文说明和英文 pitch 之间保留同一页面上下文。
 */
export function getAlternateLocale(locale: Locale): Locale {
  return locale === "zh" ? "en" : "zh";
}

/**
 * 业务职责：把内部语言代码转换为 HTML 可理解的语言标记，辅助浏览器与辅助技术选择正确读音。
 */
export function getHtmlLang(locale: Locale): string {
  return locale === "zh" ? "zh-CN" : "en";
}

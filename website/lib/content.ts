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
    titleLines: string[];
    lead: string;
    primaryCta: string;
    secondaryCta: string;
    proof: string[];
    codeLabel: string;
    showCodePanel: boolean;
    imageAlt: string;
  };
  protocol: {
    eyebrow: string;
    title: string;
    intro: string;
    steps: Array<{
      label: string;
      value: string;
    }>;
  };
  features: {
    eyebrow: string;
    title: string;
    kicker: string;
    items: Array<{
      icon: "Cable" | "Gauge" | "ShieldCheck" | "ReceiptText" | "RadioTower" | "Workflow";
      title: string;
      body: string;
    }>;
  };
  workflow: {
    eyebrow: string;
    title: string;
    intro: string;
    steps: Array<{
      number: string;
      title: string;
      body: string;
    }>;
  };
  operations: {
    eyebrow: string;
    title: string;
    intro: string;
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
    alternateLanguageName: "英文",
    meta: {
      title: "浏览器会话池中继官网",
      description:
        "把浏览器里的网页智能会话池包装成兼容接口服务，支持工作节点调度、逐字流式回传、会话亲和、计费与日志。"
    },
    nav: {
      product: "产品",
      flow: "链路",
      ops: "运维",
      docs: "文档"
    },
    hero: {
      eyebrow: "浏览器会话池成为基础设施",
      title: "把浏览器工作节点编排成兼容接口中继",
      titleLines: ["把浏览器工作节点", "编排成", "兼容接口中继"],
      lead:
        "系统将多台指纹浏览器、扩展工作节点和已登录的网页智能账号编排成一个可调度的中继服务。调用方仍然使用熟悉的聊天补全入口，背后由工作节点池完成分发、逐字回传、会话保持与故障处理。",
      primaryCta: "查看使用文档",
      secondaryCta: "打开代码仓库",
      proof: [
        "服务控制中心",
        "工作节点协议",
        "逐字流式回传",
        "计费控制台"
      ],
      codeLabel: "兼容接口请求",
      showCodePanel: false,
      imageAlt:
        "中继控制台把接口请求分发到多个浏览器工作节点的产品示意图"
    },
    protocol: {
      eyebrow: "协议路径",
      title: "这是中继控制面，不是另一个模型外壳",
      intro:
        "请求从调用方入口进入，调度器挑选空闲工作节点，扩展在网页端完成提问输入，并把增量内容逐字回传为标准事件流。",
      steps: [
        { label: "调用方", value: "应用程序或命令行工具" },
        { label: "服务端", value: "鉴权、计费、调度" },
        { label: "工作节点", value: "浏览器扩展与登录态网页智能" },
        { label: "流式输出", value: "聊天增量事件" }
      ]
    },
    features: {
      eyebrow: "控制面",
      title: "为管理大量浏览器账号的团队而建",
      kicker: "把实验性的浏览器自动化，收束成可观测、可限流、可计费的接口产品。",
      items: [
        {
          icon: "Workflow",
          title: "工作节点池调度",
          body:
            "每台云机器以工作节点身份接入，调度器按状态、并发槽位和排队等待时间选择可用节点，避免单个账号成为瓶颈。"
        },
        {
          icon: "Cable",
          title: "会话亲和",
          body:
            "会话标识能把后续请求固定回同一个工作节点与同一个网页对话，让长上下文业务保持连续。"
        },
        {
          icon: "RadioTower",
          title: "真实流式桥接",
          body:
            "页面注入层捕获网页事件流，后台脚本经套接字通道回传，服务端再封装为调用方熟悉的增量响应。"
        },
        {
          icon: "ShieldCheck",
          title: "鉴权与角色分层",
          body:
            "消费密钥、用户登录令牌、管理员令牌三套入口分层管理，适合把中继服务提供给多个内部或外部用户。"
        },
        {
          icon: "ReceiptText",
          title: "预扣费计费",
          body:
            "按模型成本预扣余额，成功后确认扣费，失败后自动退还；默认可关闭，方便从私有工具平滑进化到付费服务。"
        },
        {
          icon: "Gauge",
          title: "实时运维控制台",
          body:
            "日志、指标、工作节点状态和实时事件进入控制台，让排队、掉线、限流和登录失效都能被看见。"
        }
      ]
    },
    workflow: {
      eyebrow: "请求生命周期",
      title: "从一次接口调用到网页智能会话",
      intro:
        "关键不是替代模型，而是把浏览器会话池包装成稳定的兼容接口服务层。",
      steps: [
        {
          number: "一",
          title: "调用方请求",
          body:
            "客户端携带消费密钥请求聊天补全入口，服务端完成鉴权、限流、余额检查与会话解析。"
        },
        {
          number: "二",
          title: "调度器分发",
          body:
            "节点注册表只选择空闲且有容量的工作节点；如果会话已绑定，则优先回到原工作节点与原网页对话。"
        },
        {
          number: "三",
          title: "浏览器执行",
          body:
            "浏览器扩展管理标签页，页面注入层进入网页运行环境，捕获网络请求并解析网页智能的增量输出。"
        },
        {
          number: "四",
          title: "兼容格式响应",
          body:
            "服务端按请求编号关联增量、完成和错误事件，把工作节点输出转换为流式响应或非流式聚合结果。"
        }
      ]
    },
    operations: {
      eyebrow: "可信运维",
      title: "面向生产，但刻意保持单实例",
      intro:
        "当前架构明确选择单实例与内存实时状态，关系数据库只持久化身份、账号、账单和日志，降低运维复杂度。",
      panels: [
        {
          title: "快速失败",
          body:
            "工作节点断连或心跳超时会立即结束在途请求，而不是让调用方等待生成超时。"
        },
        {
          title: "存储抽象",
          body:
            "未配置数据库时使用内存开发存储；配置关系数据库后自动迁移，并持久化用户、密钥、会话和日志。"
        },
        {
          title: "控制台优先可见",
          body:
            "管理事件流推送实时状态，日志通道以非阻塞方式批量写入，中继热路径不会被日志系统拖慢。"
        }
      ]
    },
    footer: {
      tagline: "让浏览器智能会话可运营、可观测，并以接口形态对外服务。",
      repo: "代码仓库",
      switchLanguage: "切换到英文"
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
      docs: "Docs"
    },
    hero: {
      eyebrow: "浏览器会话池, served like infrastructure",
      title: "Turn browser workers into an OpenAI API relay",
      titleLines: ["Turn browser", "workers into", "OpenAI API relay"],
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
      showCodePanel: true,
      imageAlt:
        "sub2org relay control surface showing API streams routed to browser worker sessions"
    },
    protocol: {
      eyebrow: "Protocol path",
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
      eyebrow: "Control plane",
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
      eyebrow: "Request lifecycle",
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
      eyebrow: "Operate with confidence",
      title: "Production-minded, intentionally single-instance",
      intro:
        "Live presence stays in memory inside one sub2org server; Postgres persists identity, accounts, billing, sessions, and logs without adding Redis complexity.",
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

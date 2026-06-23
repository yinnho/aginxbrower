# AginxBrowser

轻量级服务端浏览器引擎，内置 Obscura 浏览器内核，用于快速页面抓取、JS 交互和聚合搜索。

## 定位

**轻量级服务端浏览器 + 原生搜索引擎**——内置 V8 引擎，支持 JS 执行、CSS 选择器、页面导航；内置 Rust 原生搜索引擎（百度/Bing/搜狗/搜狗微信/Google），聚合搜索 + 抓正文一体化。定位是**纯外挂基础设施**：作为独立服务挂在系统里，谁需要谁调，不嵌入宿主代码。

- **抓取**：渲染 JS、过风控（微信公众号免 cookie）、提取正文
- **搜索**：`/search` 原生多引擎聚合（百度/Bing/搜狗/搜狗微信/Google），并可对前 N 条结果自动抓正文，Agent 一步完成"搜→读"
- **分层渲染**：静态页面纯 HTTP 直取（~100ms），需要 JS 渲染时才启动浏览器（~1-2s），80% 页面加速
- **Cloudflare Turnstile 自动绕过**：检测 "Just a moment..." 挑战页，自动等待 `cf_clearance` cookie
- **TLS 指纹伪装**：stealth 模式模拟 Chrome145/Firefox133/Safari/Edge 指纹，可按请求切换，绕过基于 TLS 指纹的反爬检测
- **MCP Server**：`--mcp` 模式暴露 fetch/eval/click/search 为 MCP 工具，Claude Code/Claude Desktop/Cursor 直接调用
- **Firecrawl 兼容**：`/v1/scrape` 端点，现有 Firecrawl 客户端改 base URL 即可迁移

## 目录结构

```
aginxbrowser/
├── Cargo.toml
├── build.rs              # V8 snapshot 生成
├── js/
│   └── bootstrap.js      # V8 启动脚本
├── README.md
├── docs/
│   ├── API.md            # 完整 API 参考（HTTP + MCP）
│   └── search-design.md  # 搜索引擎设计文档
└── src/
    ├── main.rs              # HTTP 服务入口与路由
    ├── server.rs            # 业务层（fetch/click/eval/search）
    ├── render.rs            # 分层渲染（HTTP 直取 → obscura 浏览器）
    ├── mcp.rs               # MCP Server（stdio，fetch/eval/click/search 工具）
    ├── firecrawl_compat.rs  # Firecrawl 兼容 /v1/scrape 端点
    ├── browser.rs           # 顶层 API：Browser、BrowserBuilder
    ├── page.rs              # 顶层 API：Page、Element
    ├── config.rs            # BrowserConfig
    ├── cookie.rs            # CookieStore
    ├── error.rs             # Error 类型
    ├── search/              # 原生搜索引擎
    │   ├── mod.rs           #   SearchEngine trait、Registry、合并去重、CAPTCHA 暂停
    │   ├── baidu.rs         #   百度（JSON API，wreq stealth）
    │   ├── bing.rs          #   Bing（HTML 解析，plain reqwest）
    │   ├── sogou.rs         #   搜狗通用（HTML 解析，plain reqwest）
    │   ├── sogou_wechat.rs  #   搜狗微信（HTML 解析，plain reqwest + /link 解析）
    │   └── google.rs        #   Google（HTML 解析，wreq stealth + proxy）
    │
    ├── obscura_dom/         # HTML 解析、DOM 树、CSS 选择器
    ├── obscura_net/         # HTTP 客户端、Cookie、编码、代理
    ├── obscura_js/          # V8 运行时、JS ops、模块加载
    └── obscura_browser/     # 页面导航、生命周期、浏览器上下文
```

## 构建

```bash
# 普通构建（不含 stealth，TLS 指纹等功能不生效）
cargo build --release

# 含 stealth（需 go + cmake + C++ 工具链，启用 TLS 指纹伪装）
cargo build --release --features stealth
```

依赖：Rust 1.78+，首次编译自动下载 V8 静态库（需网络）。启用 stealth 需额外 `go`、`cmake`、C++ 编译器。

## 运行

```bash
export OBSCURA_PROXY=socks5://127.0.0.1:8800   # 可选
./target/release/aginxbrowser
```

默认监听 `0.0.0.0:8089`，可通过 `AGINXBROWSER_BIND` 修改。

## 快速验证

```bash
# 健康检查
curl http://127.0.0.1:8089/health
# → {"status":"ok","engine":"obscura"}

# 抓取页面
curl -sS -X POST http://127.0.0.1:8089/fetch \
  -H "Content-Type: application/json" \
  -d '{"url":"https://example.com"}'

# 搜索
curl -sS -X POST http://127.0.0.1:8089/search \
  -H "Content-Type: application/json" \
  -d '{"q":"macbook 价格","max_results":5}'

# MCP 模式
./target/release/aginxbrowser --mcp
```

## API 文档

**完整 API 参考** → [`docs/API.md`](docs/API.md)

包含：
- 所有 HTTP 端点的完整参数表 + 请求/响应示例（`/fetch`、`/click`、`/eval`、`/search`、`/v1/scrape`）
- MCP Server 的 4 个工具及参数
- **Claude Code** / **Claude Desktop** / **Cursor** 客户端配置
- 远程服务器 SSH 接入方式
- 环境变量、错误码、站点抓取示例

## Features

| Feature | 默认 | 说明 |
|---------|------|------|
| `stealth` | 关闭 | TLS/JA3 指纹伪装（依赖 BoringSSL，需 `go` + C++ 工具链） |

## 运行时环境变量

| 变量 | 默认 | 说明 |
|------|------|------|
| `AGINXBROWSER_BIND` | `0.0.0.0:8089` | 监听地址 |
| `AGINXBROWSER_STEALTH` | 启用 | `0` 关闭 stealth（诊断用） |
| `AGINXBROWSER_UA` | Linux Chrome145 | 伪装 UA |
| `AGINXBROWSER_ACCEPT_LANGUAGE` | `zh-CN,zh;q=0.9,en;q=0.8` | Accept-Language |
| `OBSCURA_PROXY` | 无 | 代理地址，`use_proxy:true` 时使用 |
| `AGINXBROWSER_CACHE_TTL_SECS` | `600` | `/fetch` 缓存 TTL，`0` 禁用 |

## 作为外挂接入其他系统

AginxBrowser 定位是**纯外挂基础设施**——像真实浏览器一样作为独立服务挂在系统里，谁需要谁调用，不嵌入宿主代码、不污染宿主配置。同机部署一个实例（systemd 守护），所有需要"渲染 + 抓取"能力的应用共享它。

接入方式：读环境变量 `AGINXBROWSER_URL=http://127.0.0.1:8089`。未设 → 行为不变；设了 → 风控站自动调 AginxBrowser 渲染抓取，失败自动回退。

## 已知限制

1. **无法截图**：没有 layout/paint 引擎
2. **无元素坐标**：只能做 JS click，不能做基于屏幕坐标的点击
3. **JS 复杂组件可能失败**：React/Vue 事件委托可能不响应原生 `click()`
4. **代理支持**：HTTP/HTTPS/SOCKS5，通过 `OBSCURA_PROXY` 传入
5. **强风控站点**：百度文库暂不支持；知乎专栏需有效 `__zse_ck`

## 与 Chromium 对比

| 项目 | AginxBrowser | Chromium |
|------|------------|----------|
| 二进制体积 | ~70MB | ~256MB+ |
| 启动速度 | 快 | 慢 |
| 截图 | ❌ | ✅ |
| 坐标点击 | ❌ | ✅ |
| JS click / scraping | ✅ | ✅ |
| 复杂 SPA 兼容 | 中等 | 高 |
| 代理 | ✅ | ✅ |
| Cookie 持久化 | ✅ | ✅ |
| TLS 指纹伪装 | ✅ (stealth，可切换) | ✅ |
| 内置搜索 | ✅ (5 引擎) | ❌ |
| 分层渲染加速 | ✅ (HTTP 直取优先) | ❌ |
| Cloudflare 自动绕过 | ✅ | 需插件 |
| MCP Server | ✅ | ❌ |
| Firecrawl 兼容 API | ✅ | ❌ |

## 许可证

与 OpenCarrier 主项目保持一致。

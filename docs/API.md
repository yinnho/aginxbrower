# AginxBrowser API 参考

> 完整的 HTTP API + MCP Server 接入文档。5 分钟快速接入。

## 快速开始

```bash
# 构建并启动
cargo build --release
./target/release/aginxbrowser

# 验证服务
curl http://127.0.0.1:8089/health
# → {"status":"ok","engine":"obscura"}

# 抓取页面
curl -sS -X POST http://127.0.0.1:8089/fetch \
  -H "Content-Type: application/json" \
  -d '{"url":"https://example.com"}'
```

---

## HTTP API

默认监听 `0.0.0.0:8089`，可通过 `AGINXBROWSER_BIND` 环境变量修改。

### GET /health

健康检查。

```bash
curl http://127.0.0.1:8089/health
```

响应：

```json
{"status":"ok","engine":"obscura"}
```

---

### POST /fetch

抓取页面并返回内容。支持分层渲染、Cloudflare 自动绕过、TLS 指纹切换。

**请求字段：**

| 字段 | 类型 | 必填 | 默认 | 说明 |
|------|------|------|------|------|
| url | string | ✅ | — | 目标 URL |
| format | string | | `"markdown"` | 输出格式：`markdown` / `html` / `text` |
| selector | string | | `null` | CSS 选择器，仅提取匹配区域 |
| wait_secs | u64 | | `null` | 页面加载后额外等待秒数（等 JS 渲染完成） |
| use_proxy | bool | | `false` | 走 `OBSCURA_PROXY` 代理。国外站点设 `true` |
| cookies | string[] | | `[]` | 导航前注入的 cookie，格式 `["name=value", ...]` |
| max_chars | usize | | `50000` | 截断 `content` 到指定字符数。`0` 不限 |
| auto_bypass_challenge | bool | | `true` | 自动检测并绕过 Cloudflare Turnstile 挑战 |
| render_tier | string | | `"auto"` | 渲染策略（见下方说明） |
| tls_fingerprint | string | | `null` | TLS 指纹（stealth 模式），见下方说明 |

**render_tier 选项：**

| 值 | 说明 |
|----|------|
| `auto` | HTTP 直取优先，内容不足时自动回退浏览器（**推荐**，默认） |
| `http` | 纯 HTTP，不走浏览器。最快但拿不到 JS 渲染内容 |
| `obscura` | 强制走 obscura 浏览器渲染。最慢但最可靠 |

**tls_fingerprint 选项（需 `--features stealth`）：**

| 值 | 说明 |
|----|------|
| `null` | 默认 Chrome145 |
| `"chrome145"` | Chrome 145 |
| `"firefox133"` | Firefox 133 |
| `"firefox147"` | Firefox 147 |
| `"safari17_5"` | Safari 17.5 |
| `"safari18"` | Safari 18 |
| `"safari26"` | Safari 26 |
| `"edge145"` | Edge 145 |

**响应字段：**

| 字段 | 类型 | 说明 |
|------|------|------|
| url | string | 最终 URL（重定向后） |
| title | string? | 页面标题 |
| content | string | 抓取内容（markdown/html/text） |
| truncated | bool | `content` 是否被 `max_chars` 截断 |

**示例 — 基础抓取：**

```bash
curl -sS -X POST http://127.0.0.1:8089/fetch \
  -H "Content-Type: application/json" \
  -d '{"url":"https://example.com"}'
```

```json
{
  "url": "https://example.com/",
  "title": "Example Domain",
  "content": "# Example Domain\n\nThis domain is for use in illustrative examples...",
  "truncated": false
}
```

**示例 — 提取特定区域（CSS 选择器）：**

```bash
curl -sS -X POST http://127.0.0.1:8089/fetch \
  -H "Content-Type: application/json" \
  -d '{"url":"https://github.com/trending","format":"text","selector":"article","use_proxy":true}'
```

**示例 — 强制浏览器渲染：**

```bash
curl -sS -X POST http://127.0.0.1:8089/fetch \
  -H "Content-Type: application/json" \
  -d '{"url":"https://react-app.example.com","render_tier":"obscura"}'
```

**缓存**：`/fetch` 有进程内缓存（key 含 url/format/selector/cookies/use_proxy/max_chars/render_tier/tls_fingerprint），TTL 由 `AGINXBROWSER_CACHE_TTL_SECS` 控制（默认 600s，`0` 禁用）。重复抓取同一 URL 命中缓存（~0.01s vs 首次 ~1s）。

**安全**：内置 SSRF 防护（拦截非 http(s) scheme、私网/loopback IP）、robots.txt 遵守、tracker 拦截（stealth 模式）。

---

### POST /click

加载页面并点击指定元素（`element.click()`），返回点击后的页面文本。

**请求字段：**

| 字段 | 类型 | 必填 | 默认 | 说明 |
|------|------|------|------|------|
| url | string | ✅ | — | 目标 URL |
| selector | string | ✅ | — | CSS 选择器 |
| wait_secs | u64 | | `null` | 页面加载后额外等待秒数 |
| use_proxy | bool | | `false` | 走代理 |
| cookies | string[] | | `[]` | 导航前注入的 cookie |
| tls_fingerprint | string | | `null` | TLS 指纹（stealth 模式） |

**响应字段：**

| 字段 | 类型 | 说明 |
|------|------|------|
| url | string | 最终 URL |
| selector | string | 使用的选择器 |
| clicked | bool | 是否成功点击 |
| text_after | string? | 点击后的页面文本 |

**示例：**

```bash
curl -sS -X POST http://127.0.0.1:8089/click \
  -H "Content-Type: application/json" \
  -d '{"url":"https://example.com","selector":"a"}'
```

```json
{
  "url": "https://example.com/",
  "selector": "a",
  "clicked": true,
  "text_after": "..."
}
```

---

### POST /eval

在页面上执行任意 JavaScript 并返回结果。支持 `async`/`Promise`。

**请求字段：**

| 字段 | 类型 | 必填 | 默认 | 说明 |
|------|------|------|------|------|
| url | string | ✅ | — | 目标 URL |
| script | string | ✅ | — | JS 表达式或 async IIFE |
| wait_secs | u64 | | `null` | 页面加载后额外等待秒数 |
| use_proxy | bool | | `false` | 走代理 |
| cookies | string[] | | `[]` | 导航前注入的 cookie |
| tls_fingerprint | string | | `null` | TLS 指纹（stealth 模式） |

**响应字段：**

| 字段 | 类型 | 说明 |
|------|------|------|
| url | string | 最终 URL |
| result | any | JS 执行结果 |

**示例 — 简单表达式：**

```bash
curl -sS -X POST http://127.0.0.1:8089/eval \
  -H "Content-Type: application/json" \
  -d '{"url":"https://example.com","script":"document.title"}'
```

```json
{"url":"https://example.com/","result":"Example Domain"}
```

**示例 — async 脚本（等动态渲染）：**

```bash
curl -sS -X POST http://127.0.0.1:8089/eval \
  -H "Content-Type: application/json" \
  -d '{
    "url":"https://github.com/trending",
    "script":"(async()=>{await new Promise(r=>setTimeout(r,4000));return Array.from(document.querySelectorAll(\"article.Box-row\")).slice(0,5).map(a=>a.querySelector(\"h2 a\")?.textContent?.trim())})()",
    "use_proxy":true
  }'
```

> `/eval` 的 `script` 参数支持 **async 函数**：返回 Promise 会被自动 await。适合 React/Vue 等动态渲染页面——等渲染完成再提取数据。

---

### POST /search

原生聚合搜索 + 可选自动抓正文。Agent 一步完成"搜→读"。

**请求字段：**

| 字段 | 类型 | 必填 | 默认 | 说明 |
|------|------|------|------|------|
| q | string | ✅ | — | 搜索关键词 |
| fetch_top | usize | | `0` | 对前 N 条结果抓正文。`0` = 只返回摘要 |
| categories | string | | `"general"` | 搜索分类 |
| language | string | | `"zh-CN"` | 语言 |
| max_results | usize | | `10` | 返回结果上限 |
| max_chars_per | usize | | `4000` | 每条正文字符截断。`0` 不限 |
| wait_secs | u64 | | `3` | 抓正文时每页 JS 渲染等待秒数 |
| use_proxy | bool | | `false` | 抓正文时是否走代理（国外站） |

**内置搜索引擎：**

| 引擎 | 分类 | HTTP 客户端 | 说明 |
|------|------|------------|------|
| Baidu | general | wreq stealth | 百度 JSON API |
| Bing | general | plain reqwest | Bing HTML 解析 |
| Sogou | general | plain reqwest | 搜狗通用搜索 |
| Sogou WeChat | general, news | plain reqwest | 搜狗微信搜索 |
| Google | general | wreq stealth + proxy | Google HTML 解析，国内需代理 |

多引擎并发查询，结果合并去重：同一 URL（归一化后）合并为一条，`engines` 列出来源引擎，`score` 累加。引擎触发验证码后自动暂停（搜狗微信 60 分钟，其他 30 分钟），不影响其他引擎。

**响应字段：**

| 字段 | 类型 | 说明 |
|------|------|------|
| query | string | 搜索关键词 |
| number_of_results | usize | 结果总数 |
| results | array | 结果列表 |

**results 内每条：**

| 字段 | 类型 | 说明 |
|------|------|------|
| title | string | 标题 |
| url | string | 链接 |
| snippet | string | 搜索摘要 |
| engines | string[] | 来源引擎 |
| score | float | 综合得分 |
| content | string? | 正文（仅 `fetch_top` 范围内有值） |
| content_truncated | bool | 正文是否被截断 |
| fetch_error | string? | 抓正文失败原因 |

**示例 — 纯搜索（快，毫秒级）：**

```bash
curl -sS -X POST http://127.0.0.1:8089/search \
  -H "Content-Type: application/json" \
  -d '{"q":"macbook 价格","max_results":5}'
```

**示例 — 搜索 + 抓前 3 条正文：**

```bash
curl -sS -X POST http://127.0.0.1:8089/search \
  -H "Content-Type: application/json" \
  -d '{"q":"macbook 价格","fetch_top":3,"max_chars_per":2000}'
```

响应：

```json
{
  "query": "macbook 价格",
  "number_of_results": 1000,
  "results": [
    {
      "title": "MacBook Air - Apple",
      "url": "https://www.apple.com/mac/",
      "snippet": "...",
      "engines": ["bing", "baidu"],
      "score": 8.5,
      "content": "...(正文)...",
      "content_truncated": false,
      "fetch_error": null
    }
  ]
}
```

> `fetch_top=0`：纯搜索，毫秒级。`fetch_top>0`：前 N 条并发抓正文（复用 `/fetch` 的 stealth + JS 渲染），单条失败不影响其他（`fetch_error` 标记）。

---

### POST /v1/scrape（Firecrawl 兼容）

[Firecrawl](https://github.com/mendableai/firecrawl) 兼容端点。现有 Firecrawl 客户端只需改 base URL 即可迁移。

**请求字段：**

| 字段 | 类型 | 必填 | 默认 | 说明 |
|------|------|------|------|------|
| url | string | ✅ | — | 目标 URL |
| formats | string[] | | `["markdown"]` | 输出格式：`["markdown"]` / `["html"]` / `["markdown","html"]` |
| onlyMainContent | bool | | `false` | 仅主内容（接受参数，暂未实现） |
| waitFor | u64 | | `null` | 等待 JS 渲染毫秒数 |
| timeout | u32 | | `null` | 超时（毫秒，接受参数） |
| actions | object[] | | `[]` | 抓取前动作（见下方） |
| selector | string | | `null` | CSS 选择器 |
| tls_fingerprint | string | | `null` | TLS 指纹（stealth 模式） |

**actions 格式：**

```json
[
  {"type": "click", "selector": "button.accept"},
  {"type": "wait", "milliseconds": 1000}
]
```

| type | 字段 | 说明 |
|------|------|------|
| `click` | `selector` | 点击元素 |
| `wait` | `milliseconds` | 等待指定毫秒 |
| `screenshot` | — | 接受但不实现 |
| `scroll` | — | 接受但不实现 |
| `writeText` | `text`, `selector?` | 接受但不实现 |
| `pressKey` | `key` | 接受但不实现 |

**响应（Firecrawl 格式，成功/失败均返回 HTTP 200）：**

```json
{
  "success": true,
  "data": {
    "markdown": "...",
    "html": "...",
    "metadata": {
      "title": "Example Domain",
      "sourceURL": "https://example.com/",
      "description": "...",
      "statusCode": 200
    }
  }
}
```

失败时：

```json
{
  "success": false,
  "data": {
    "markdown": null,
    "html": null,
    "metadata": {
      "title": null,
      "sourceURL": "https://example.com/",
      "description": null,
      "statusCode": 500,
      "error": "..."
    }
  }
}
```

---

## MCP Server

`--mcp` 模式将 AginxBrowser 包装为 MCP（Model Context Protocol）Server，AI Agent 可直接调用，无需 HTTP 客户端。

### 启动方式

```bash
./target/release/aginxbrowser --mcp
```

MCP 走 stdio 协议，不启动 HTTP 服务器，通过 stdin/stdout 与 MCP 客户端通信。

### 提供的工具

#### fetch

抓取网页并返回内容。支持 JS 渲染、stealth 模式、多种输出格式。

| 参数 | 类型 | 必填 | 默认 | 说明 |
|------|------|------|------|------|
| url | string | ✅ | — | 目标 URL |
| format | string | | `"markdown"` | 输出格式：`markdown` / `html` / `text` |
| selector | string | | `null` | CSS 选择器 |
| wait_secs | u64 | | `null` | 页面加载后等待秒数 |
| use_proxy | bool | | `false` | 走代理 |
| max_chars | usize | | `50000` | 截断字符数 |
| auto_bypass_challenge | bool | | `true` | 自动绕过 Cloudflare Turnstile |
| render_tier | string | | `"auto"` | 渲染策略：`auto` / `http` / `obscura` |
| tls_fingerprint | string | | `null` | TLS 指纹 |

#### eval

在页面上执行 JavaScript 并返回结果。支持 async/Promise。

| 参数 | 类型 | 必填 | 默认 | 说明 |
|------|------|------|------|------|
| url | string | ✅ | — | 目标 URL |
| script | string | ✅ | — | JS 代码 |
| wait_secs | u64 | | `null` | 页面加载后等待秒数 |

#### click

点击页面上的元素。

| 参数 | 类型 | 必填 | 默认 | 说明 |
|------|------|------|------|------|
| url | string | ✅ | — | 目标 URL |
| selector | string | ✅ | — | CSS 选择器 |
| wait_secs | u64 | | `null` | 页面加载后等待秒数 |

#### search

多引擎搜索（百度/Bing/搜狗/搜狗微信/Google），可对前 N 条结果自动抓正文。

| 参数 | 类型 | 必填 | 默认 | 说明 |
|------|------|------|------|------|
| q | string | ✅ | — | 搜索关键词 |
| fetch_top | usize | | `0` | 对前 N 条结果抓正文 |
| categories | string | | `"general"` | 搜索分类 |
| max_results | usize | | `10` | 结果上限 |
| max_chars_per | usize | | `4000` | 每条正文截断字符数 |

### 客户端配置

#### Claude Code

编辑项目或全局的 settings 文件：

**项目级** `.claude/settings.json`：

```json
{
  "mcpServers": {
    "aginxbrowser": {
      "command": "/path/to/aginxbrowser",
      "args": ["--mcp"]
    }
  }
}
```

**全局级** `~/.claude/settings.json`：

```json
{
  "mcpServers": {
    "aginxbrowser": {
      "command": "/path/to/aginxbrowser",
      "args": ["--mcp"]
    }
  }
}
```

#### Claude Desktop

编辑 `~/Library/Application Support/Claude/claude_desktop_config.json`（macOS）或 `%APPDATA%\Claude\claude_desktop_config.json`（Windows）：

```json
{
  "mcpServers": {
    "aginxbrowser": {
      "command": "/path/to/aginxbrowser",
      "args": ["--mcp"]
    }
  }
}
```

#### Cursor

编辑项目根目录的 `.cursor/mcp.json`：

```json
{
  "mcpServers": {
    "aginxbrowser": {
      "command": "/path/to/aginxbrowser",
      "args": ["--mcp"]
    }
  }
}
```

#### 远程服务器（via SSH）

如果 AginxBrowser 部署在远程服务器上，通过 SSH 隧道接入：

```json
{
  "mcpServers": {
    "aginxbrowser": {
      "command": "ssh",
      "args": ["your-server", "/data/www/aginxbrowser/target/release/aginxbrowser", "--mcp"]
    }
  }
}
```

> **注意**：SSH 方式需要本机能免密登录远程服务器（`ssh-copy-id` 配置公钥），且远程服务器上已编译好 AginxBrowser。

---

## 环境变量

| 变量 | 默认 | 说明 |
|------|------|------|
| `AGINXBROWSER_BIND` | `0.0.0.0:8089` | HTTP 服务监听地址 |
| `AGINXBROWSER_STEALTH` | 启用 | `0` 关闭 stealth（诊断用） |
| `AGINXBROWSER_UA` | Linux Chrome145 | 伪装 UA |
| `AGINXBROWSER_ACCEPT_LANGUAGE` | `zh-CN,zh;q=0.9,en;q=0.8` | Accept-Language |
| `AGINXBROWSER_CACHE_TTL_SECS` | `600` | `/fetch` 缓存 TTL（秒），`0` 禁用 |
| `OBSCURA_PROXY` | 无 | 代理地址（`use_proxy:true` 时使用） |

---

## 错误码

| HTTP 状态码 | 场景 |
|------------|------|
| 400 | CSS 选择器语法错误、URL 解析失败 |
| 404 | 元素未找到 |
| 502 | 目标网站不可达（DNS/连接失败） |
| 504 | 请求超时 |
| 500 | 其他内部错误 |

---

## 站点抓取示例

### 微信公众号文章（公开，无需登录）

stealth 模式可直接抓取，**不需要 cookie**：

```bash
# 用 /eval 提取标题和正文
curl -sS -X POST http://127.0.0.1:8089/eval -H 'Content-Type: application/json' -d '{
  "url": "https://mp.weixin.qq.com/s/xxxxx",
  "script": "({title:document.querySelector(\"#activity-name\")?.textContent?.trim(), body:document.querySelector(\"#js_content\")?.innerText})"
}'

# 用 /search 搜索微信文章并自动抓正文
curl -sS -X POST http://127.0.0.1:8089/search -H 'Content-Type: application/json' \
  -d '{"q":"AI人工智能","categories":"news","fetch_top":3,"max_chars_per":2000}'
```

### 知乎专栏（需 cookie）

知乎专栏是公开内容，但需要有效 cookie 才能访问：

```bash
curl -sS -X POST http://127.0.0.1:8089/fetch -H 'Content-Type: application/json' -d '{
  "url": "https://zhuanlan.zhihu.com/p/xxxxx",
  "cookies": ["d_c0=YOUR_D_C0; __zse_ck=YOUR_ZSE_CK"]
}'
```

### Cloudflare 保护的站点

默认开启 `auto_bypass_challenge`，自动检测 "Just a moment..." 页面并等待 `cf_clearance` cookie：

```bash
# 默认行为，自动绕过
curl -sS -X POST http://127.0.0.1:8089/fetch -H 'Content-Type: application/json' -d '{
  "url": "https://cloudflare-protected-site.com"
}'

# 显式关闭自动绕过
curl -sS -X POST http://127.0.0.1:8089/fetch -H 'Content-Type: application/json' -d '{
  "url": "https://cloudflare-protected-site.com",
  "auto_bypass_challenge": false
}'
```

### 动态渲染页面（React/Vue SPA）

用 `/eval` 的 async 脚本等待渲染完成后再提取：

```bash
curl -sS -X POST http://127.0.0.1:8089/eval -H 'Content-Type: application/json' -d '{
  "url": "https://github.com/trending",
  "script": "(async()=>{await new Promise(r=>setTimeout(r,4000));return Array.from(document.querySelectorAll(\"article.Box-row\")).slice(0,5).map(a=>a.querySelector(\"h2 a\")?.textContent?.trim())})()",
  "use_proxy": true
}'
```

### TLS 指纹切换

部分站点会检测 TLS 指纹，Chrome 被拦时可以换 Firefox/Safari：

```bash
curl -sS -X POST http://127.0.0.1:8089/fetch -H 'Content-Type: application/json' -d '{
  "url": "https://strict-site.com",
  "tls_fingerprint": "firefox133",
  "use_proxy": true
}'
```

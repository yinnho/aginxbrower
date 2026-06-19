use crate::{
    ClickRequest, ClickResponse, EvalRequest, EvalResponse, FetchRequest, FetchResponse,
    OutputFormat,
};
use crate::browser::Browser;
use anyhow::{Context, Result};

/// Build a browser instance.
/// `use_proxy` decides whether the upstream `OBSCURA_PROXY` is applied. Domestic
/// sites should pass `false` (direct is faster and SOCKS5 often times out);
/// foreign sites that are blocked/unreachable directly pass `true`.
fn build_browser(use_proxy: bool) -> Result<Browser> {
    // Stealth defaults on; disable via AGINXBROWER_STEALTH=0 (diagnostic / when
    // the wreq stealth client misbehaves on a given site).
    let stealth = !matches!(std::env::var("AGINXBROWER_STEALTH").ok().as_deref(), Some("0"));
    let mut builder = Browser::builder().stealth(stealth);
    if use_proxy {
        if let Ok(proxy) = std::env::var("OBSCURA_PROXY") {
            builder = builder.proxy(&proxy);
        }
    }
    Ok(builder.build()?)
}

/// Run an Obscura operation on a dedicated single-threaded runtime.
///
/// Obscura's V8 runtime holds `Rc<RefCell<…>>` state, which is `!Send`, so a
/// `Page` cannot be held across `.await` points on Tokio's multi-threaded
/// runtime. We spin up a current-thread runtime on a blocking thread and drive
/// the whole navigation there — the V8 isolate stays on one thread for its
/// entire lifetime, which is what deno_core expects.
fn run_on_local_runtime<F, T>(f: F) -> Result<T>
where
    F: for<'a> FnOnce(&'a tokio::runtime::Runtime) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T>> + 'a>>
        + Send
        + 'static,
    T: Send + 'static,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let local = tokio::task::LocalSet::new();
    let result = local.block_on(&runtime, f(&runtime));
    // Drop the page/browser inside the LocalSet + runtime context so V8 cleanup
    // happens on the owning thread.
    drop(local);
    drop(runtime);
    result
}

/// Inject request-supplied cookies into the browser's cookie jar before
/// navigation. Each entry is a Set-Cookie style string (`name=value`). They
/// are scoped to the target URL's host so they attach to the first request —
/// needed for sites (e.g. WeChat articles) that gate content behind a
/// logged-in session cookie.
fn inject_cookies(browser: &Browser, cookies: &[String], target_url: &str) {
    if cookies.is_empty() {
        return;
    }
    let store = browser.cookies();
    let base = match url::Url::parse(target_url) {
        Ok(u) => u,
        Err(_) => return,
    };
    let domain = format!("Domain={}", base.host_str().unwrap_or(""));
    for c in cookies {
        // Allow callers to pass either a bare "name=value" or a full Set-Cookie.
        let full = if c.to_ascii_lowercase().contains("domain=") || c.to_ascii_lowercase().contains("path=") {
            c.clone()
        } else {
            format!("{}; {}; Path=/", c, domain)
        };
        let _ = store.set(&full, target_url);
    }
}

/// Fetch a page and return content in the requested format.
pub fn do_fetch(req: FetchRequest) -> Result<FetchResponse> {
    run_on_local_runtime(move |_rt| {
        Box::pin(async move {
            let browser = build_browser(req.use_proxy)?;
            inject_cookies(&browser, &req.cookies, &req.url);
            let mut page = browser.new_page().await?;
            page.goto(&req.url).await?;

            if let Some(wait) = req.wait_secs {
                page.settle(wait * 1000).await;
            }

            let html = page.content();
            let title = page
                .evaluate("document.title")
                .as_str()
                .map(|s| s.to_string());

            let content = match req.format {
                OutputFormat::Html => html,
                OutputFormat::Text => extract_text(&html, req.selector.as_deref())?,
                OutputFormat::Markdown => html_to_markdown(&html, req.selector.as_deref())?,
            };

            Ok(FetchResponse {
                url: page.url(),
                title,
                content,
            })
        })
    })
}

/// Click an element by CSS selector using JS `element.click()`.
pub fn do_click(req: ClickRequest) -> Result<ClickResponse> {
    run_on_local_runtime(move |_rt| {
        Box::pin(async move {
            let browser = build_browser(req.use_proxy)?;
            inject_cookies(&browser, &req.cookies, &req.url);
            let mut page = browser.new_page().await?;
            page.goto(&req.url).await?;

            if let Some(wait) = req.wait_secs {
                page.settle(wait * 1000).await;
            }

            let clicked = if let Some(el) = page.query_selector(&req.selector) {
                el.click().context("element.click() failed")?;
                true
            } else {
                false
            };

            page.settle(500).await;
            let text_after = page
                .evaluate("document.body.innerText")
                .as_str()
                .map(|s| s.to_string());

            Ok(ClickResponse {
                url: page.url(),
                selector: req.selector,
                clicked,
                text_after,
            })
        })
    })
}

/// Evaluate arbitrary JavaScript on the page.
pub fn do_eval(req: EvalRequest) -> Result<EvalResponse> {
    run_on_local_runtime(move |_rt| {
        Box::pin(async move {
            let browser = build_browser(req.use_proxy)?;
            inject_cookies(&browser, &req.cookies, &req.url);
            let mut page = browser.new_page().await?;
            page.goto(&req.url).await?;

            if let Some(wait) = req.wait_secs {
                page.settle(wait * 1000).await;
            }

            let result = page.evaluate_async(&req.script).await;

            Ok(EvalResponse {
                url: page.url(),
                result,
            })
        })
    })
}

fn extract_text(html: &str, selector: Option<&str>) -> Result<String> {
    let fragment = scraper::Html::parse_document(html);
    let selector = match selector {
        Some(s) => Some(
            scraper::Selector::parse(s).map_err(|e| anyhow::anyhow!("invalid selector: {e}"))?,
        ),
        None => None,
    };

    if let Some(sel) = selector {
        Ok(fragment
            .select(&sel)
            .map(|el| el.text().collect::<Vec<_>>().join(" "))
            .collect::<Vec<_>>()
            .join("\n"))
    } else {
        Ok(fragment
            .root_element()
            .text()
            .collect::<Vec<_>>()
            .join(" "))
    }
}

fn html_to_markdown(html: &str, selector: Option<&str>) -> Result<String> {
    let fragment = scraper::Html::parse_document(html);
    let selector = match selector {
        Some(s) => Some(
            scraper::Selector::parse(s).map_err(|e| anyhow::anyhow!("invalid selector: {e}"))?,
        ),
        None => None,
    };
    let node_ref = selector
        .and_then(|sel| fragment.select(&sel).next())
        .map(|el| el.clone())
        .unwrap_or_else(|| fragment.root_element().clone());

    Ok(html2md::parse_html(&node_ref.html()))
}

use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use reqwest::header::CONTENT_TYPE;
use scraper::{Html, Selector};
use std::collections::{BTreeSet, HashSet, VecDeque};
use std::io::Cursor;
use tokio::task::JoinSet;
use tracing::{debug, info, trace, warn};
use url::Url;

#[derive(Debug, Clone)]
pub struct CrawlConfig {
    pub max_pages: usize,
    pub max_depth: usize,
    pub concurrency: usize,
    pub user_agent: String,
}

impl Default for CrawlConfig {
    fn default() -> Self {
        Self {
            max_pages: 500,
            max_depth: 8,
            concurrency: 16,
            user_agent: "lychee-sitemap/0.1.0".to_string(),
        }
    }
}

#[derive(Debug)]
struct FetchResult {
    url: Url,
    depth: usize,
    is_success: bool,
    content_type: String,
    body: Option<String>,
}

pub async fn crawl_site(start_url: Url, config: &CrawlConfig) -> Result<BTreeSet<Url>, String> {
    if config.max_pages == 0 {
        return Ok(BTreeSet::new());
    }

    let client = reqwest::Client::builder()
        .user_agent(config.user_agent.clone())
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| format!("failed to create HTTP client: {e}"))?;

    let start = normalize_url(&start_url);
    let site_root = start.clone();
    let concurrency = config.concurrency.max(1);

    info!(
        start_url = %start,
        max_pages = config.max_pages,
        max_depth = config.max_depth,
        concurrency,
        "crawl started"
    );

    let mut queued = HashSet::new();
    let mut queue = VecDeque::new();
    let mut crawled = BTreeSet::new();
    let mut workers: JoinSet<FetchResult> = JoinSet::new();

    queued.insert(start.as_str().to_string());
    queue.push_back((start, 0usize));

    loop {
        while workers.len() < concurrency {
            let Some((current, depth)) = queue.pop_front() else {
                break;
            };

            trace!(url = %current, depth, "queued URL scheduled for fetch");
            let client = client.clone();
            workers.spawn(async move { fetch_page(client, current, depth).await });
        }

        if workers.is_empty() {
            break;
        }

        let Some(joined) = workers.join_next().await else {
            break;
        };

        let Ok(result) = joined else {
            warn!("a crawl worker task failed before producing a result");
            continue;
        };

        if crawled.len() >= config.max_pages {
            debug!(
                max_pages = config.max_pages,
                "max pages reached, aborting remaining workers"
            );
            workers.abort_all();
            break;
        }

        if !result.is_success {
            debug!(url = %result.url, depth = result.depth, "fetch was not successful");
            continue;
        }

        crawled.insert(result.url.clone());
        debug!(url = %result.url, depth = result.depth, crawled = crawled.len(), "page crawled");

        if result.depth >= config.max_depth || !result.content_type.starts_with("text/html") {
            continue;
        }

        let body = match result.body {
            Some(body) => body,
            None => continue,
        };

        for link in extract_links(&body, &result.url, &site_root) {
            if crawled.len() + queue.len() + workers.len() >= config.max_pages {
                break;
            }

            let key = link.as_str().to_string();
            if queued.insert(key) {
                trace!(url = %link, depth = result.depth + 1, "discovered new URL");
                queue.push_back((link, result.depth + 1));
            }
        }
    }

    info!(total_urls = crawled.len(), "crawl completed");

    Ok(crawled)
}

async fn fetch_page(client: reqwest::Client, url: Url, depth: usize) -> FetchResult {
    trace!(url = %url, depth, "sending request");
    let response = match client.get(url.as_str()).send().await {
        Ok(response) => response,
        Err(_) => {
            debug!(url = %url, depth, "request failed");
            return FetchResult {
                url,
                depth,
                is_success: false,
                content_type: String::new(),
                body: None,
            };
        }
    };

    if !response.status().is_success() {
        debug!(url = %url, depth, status = %response.status(), "received non-success response");
        return FetchResult {
            url,
            depth,
            is_success: false,
            content_type: String::new(),
            body: None,
        };
    }

    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();

    trace!(url = %url, depth, content_type = %content_type, "response received");

    let body = if content_type.starts_with("text/html") {
        response.text().await.ok()
    } else {
        None
    };

    FetchResult {
        url,
        depth,
        is_success: true,
        content_type,
        body,
    }
}

pub fn generate_sitemap_xml(urls: &BTreeSet<Url>) -> Result<String, String> {
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    writer
        .write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
        .map_err(|e| format!("failed to write XML declaration: {e}"))?;

    writer
        .write_event(Event::Text(BytesText::new("\n")))
        .map_err(|e| format!("failed to write newline: {e}"))?;

    let mut root = BytesStart::new("urlset");
    root.push_attribute(("xmlns", "http://www.sitemaps.org/schemas/sitemap/0.9"));
    writer
        .write_event(Event::Start(root))
        .map_err(|e| format!("failed to start urlset element: {e}"))?;

    for url in urls {
        writer
            .write_event(Event::Start(BytesStart::new("url")))
            .map_err(|e| format!("failed to start url element: {e}"))?;

        writer
            .write_event(Event::Start(BytesStart::new("loc")))
            .map_err(|e| format!("failed to start loc element: {e}"))?;

        writer
            .write_event(Event::Text(BytesText::new(url.as_str())))
            .map_err(|e| format!("failed to write URL text: {e}"))?;

        writer
            .write_event(Event::End(BytesEnd::new("loc")))
            .map_err(|e| format!("failed to end loc element: {e}"))?;

        writer
            .write_event(Event::End(BytesEnd::new("url")))
            .map_err(|e| format!("failed to end url element: {e}"))?;
    }

    writer
        .write_event(Event::End(BytesEnd::new("urlset")))
        .map_err(|e| format!("failed to end urlset element: {e}"))?;

    let output = writer.into_inner().into_inner();
    String::from_utf8(output).map_err(|e| format!("XML output was not UTF-8: {e}"))
}

fn extract_links(html: &str, current: &Url, site_root: &Url) -> Vec<Url> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("a[href]").expect("selector must be valid");

    document
        .select(&selector)
        .filter_map(|el| el.value().attr("href"))
        .filter_map(|href| current.join(href).ok())
        .map(|url| normalize_url(&url))
        .filter(is_crawlable)
        .filter(|url| is_same_site(site_root, url))
        .collect()
}

fn is_crawlable(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https")
}

fn is_same_site(site_root: &Url, candidate: &Url) -> bool {
    site_root.scheme() == candidate.scheme()
        && site_root.host_str() == candidate.host_str()
        && site_root.port_or_known_default() == candidate.port_or_known_default()
}

fn normalize_url(url: &Url) -> Url {
    let mut normalized = url.clone();

    normalized.set_fragment(None);

    let should_remove_port = (normalized.scheme() == "http" && normalized.port() == Some(80))
        || (normalized.scheme() == "https" && normalized.port() == Some(443));

    if should_remove_port {
        let _ = normalized.set_port(None);
    }

    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_url_removes_fragment_and_default_port() {
        let url = Url::parse("https://example.com:443/docs#intro").expect("valid url");
        let normalized = normalize_url(&url);
        assert_eq!(normalized.as_str(), "https://example.com/docs");
    }

    #[test]
    fn generate_sitemap_xml_contains_all_locations() {
        let mut urls = BTreeSet::new();
        urls.insert(Url::parse("https://example.com/").expect("valid url"));
        urls.insert(Url::parse("https://example.com/about").expect("valid url"));

        let xml = generate_sitemap_xml(&urls).expect("xml should be generated");

        assert!(xml.contains("<urlset"));
        assert!(xml.contains("<loc>https://example.com/</loc>"));
        assert!(xml.contains("<loc>https://example.com/about</loc>"));
    }
}

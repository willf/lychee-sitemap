use lychee_sitemap::{CrawlConfig, crawl_site};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tiny_http::{Header, Response, Server, StatusCode};
use url::Url;

fn html_response(body: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let header = Header::from_bytes("Content-Type", "text/html; charset=utf-8")
        .expect("header should be valid");
    Response::from_string(body)
        .with_status_code(StatusCode(200))
        .with_header(header)
}

fn spawn_test_server() -> (String, mpsc::Sender<()>, thread::JoinHandle<()>) {
    let server = Server::http("127.0.0.1:0").expect("server should start");
    let base_url = format!("http://{}", server.server_addr());

    let (stop_tx, stop_rx) = mpsc::channel::<()>();

    let handle = thread::spawn(move || {
        loop {
            if stop_rx.try_recv().is_ok() {
                break;
            }

            let maybe_request = server
                .recv_timeout(Duration::from_millis(100))
                .expect("recv_timeout should not fail");

            let Some(request) = maybe_request else {
                continue;
            };

            let response = match request.url() {
                "/" => html_response(
                    r#"
                    <a href="/about">About</a>
                    <a href="/about#team">About fragment duplicate</a>
                    <a href="/contact">Contact</a>
                    <a href="/contact?from=nav">Contact query variant</a>
                    <a href="https://example.com/">External</a>
                "#,
                ),
                "/about" => html_response(
                    r#"
                    <a href="/">Home</a>
                    <a href="/contact">Contact</a>
                "#,
                ),
                "/contact" => html_response("<p>Contact</p>"),
                "/contact?from=nav" => html_response("<p>Contact from nav</p>"),
                _ => Response::from_string("not found").with_status_code(StatusCode(404)),
            };

            request
                .respond(response)
                .expect("response should be sent successfully");
        }
    });

    (base_url, stop_tx, handle)
}

#[tokio::test]
async fn crawl_site_is_recursive_and_deduplicated() {
    let (base_url, stop_tx, handle) = spawn_test_server();

    let config = CrawlConfig {
        max_pages: 50,
        max_depth: 5,
        concurrency: 8,
        user_agent: "lychee-sitemap-test/0.1.0".to_string(),
    };

    let urls = crawl_site(Url::parse(&base_url).expect("valid base URL"), &config)
        .await
        .expect("crawl should succeed");

    stop_tx.send(()).expect("stop signal should be sent");
    handle.join().expect("server thread should terminate");

    let crawled: Vec<String> = urls.into_iter().map(|u| u.to_string()).collect();

    assert!(crawled.contains(&(base_url.clone() + "/")));
    assert!(crawled.contains(&(base_url.clone() + "/about")));
    assert!(crawled.contains(&(base_url.clone() + "/contact")));
    assert!(crawled.contains(&(base_url.clone() + "/contact?from=nav")));

    let about_count = crawled.iter().filter(|u| u.ends_with("/about")).count();
    assert_eq!(about_count, 1, "fragment variant should deduplicate");

    assert!(
        crawled
            .iter()
            .all(|u| !u.starts_with("https://example.com")),
        "external links should not be crawled"
    );
}

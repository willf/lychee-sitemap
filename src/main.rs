use clap::{ArgAction, Parser};
use lychee_sitemap::{CrawlConfig, crawl_site, generate_sitemap_xml};
use std::path::PathBuf;
use tracing::{Level, error, info};
use url::Url;

#[derive(Debug, Parser)]
#[command(author, version, about = "Crawl a site and generate sitemap XML")]
struct Cli {
    /// Starting URL to crawl, e.g. https://example.com
    url: String,

    /// Optional output path (for example sitemap.xml). Defaults to stdout.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Write to sitemap.xml in current directory.
    #[arg(long, conflicts_with = "output")]
    write_file: bool,

    /// Maximum number of pages to include in sitemap.
    #[arg(long, default_value_t = 500)]
    max_pages: usize,

    /// Maximum crawl depth from the starting URL.
    #[arg(long, default_value_t = 8)]
    max_depth: usize,

    /// Number of concurrent HTTP requests while crawling.
    #[arg(long, default_value_t = 16)]
    concurrency: usize,

    /// Increase verbosity (-v for info, -vv for debug, -vvv for trace).
    #[arg(short = 'v', long = "verbose", action = ArgAction::Count)]
    verbose: u8,

    /// User-Agent header for HTTP requests.
    #[arg(long, default_value = "lychee-sitemap/0.1.0")]
    user_agent: String,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    init_logging(cli.verbose);

    info!(
        output_to_file = cli.output.is_some() || cli.write_file,
        "starting sitemap generation"
    );

    let start_url = match Url::parse(&cli.url) {
        Ok(url) => url,
        Err(e) => {
            error!(url = %cli.url, error = %e, "invalid URL");
            std::process::exit(2);
        }
    };

    let config = CrawlConfig {
        max_pages: cli.max_pages,
        max_depth: cli.max_depth,
        concurrency: cli.concurrency,
        user_agent: cli.user_agent,
    };

    let urls = match crawl_site(start_url, &config).await {
        Ok(urls) => urls,
        Err(e) => {
            error!(error = %e, "failed to crawl site");
            std::process::exit(1);
        }
    };

    let xml = match generate_sitemap_xml(&urls) {
        Ok(xml) => xml,
        Err(e) => {
            error!(error = %e, "failed to generate sitemap XML");
            std::process::exit(1);
        }
    };

    let output_path = if cli.write_file {
        Some(PathBuf::from("sitemap.xml"))
    } else {
        cli.output
    };

    if let Some(path) = output_path {
        if let Err(e) = std::fs::write(&path, xml.as_bytes()) {
            error!(path = %path.display(), error = %e, "failed to write output file");
            std::process::exit(1);
        }
        info!(path = %path.display(), url_count = urls.len(), "sitemap written");
    } else {
        info!(url_count = urls.len(), "sitemap written to stdout");
        print!("{xml}");
    }
}

fn init_logging(verbose: u8) {
    let level = match verbose {
        0 => Level::WARN,
        1 => Level::INFO,
        2 => Level::DEBUG,
        _ => Level::TRACE,
    };

    let _ = tracing_subscriber::fmt()
        .json()
        .with_timer(tracing_subscriber::fmt::time::SystemTime)
        .with_max_level(level)
        .with_writer(std::io::stderr)
        .try_init();
}

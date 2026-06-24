# lychee-sitemap

[![CI](https://github.com/willf/lychee-sitemap/actions/workflows/ci.yml/badge.svg)](https://github.com/willf/lychee-sitemap/actions/workflows/ci.yml)
[![Release](https://github.com/willf/lychee-sitemap/actions/workflows/release.yml/badge.svg)](https://github.com/willf/lychee-sitemap/actions/workflows/release.yml)
[![Latest Release](https://img.shields.io/github/v/release/willf/lychee-sitemap?display_name=tag&logo=github)](https://github.com/willf/lychee-sitemap/releases)
[![Rust 2024](https://img.shields.io/badge/Rust-2024-orange?logo=rust)](https://www.rust-lang.org/)

A Rust CLI that crawls a website recursively and emits a sitemap XML document.

Architecture and testing details are documented in [ARCHITECTURE.md](ARCHITECTURE.md).

## Features

- Recursive crawling from a starting URL
- Same-site restriction (won't crawl external domains)
- URL deduplication (including fragment removal like `#section`)
- Configurable crawl limits (`max-pages`, `max-depth`)
- Bounded concurrent crawling (`--concurrency`)
- Structured JSON logs to stderr with timestamps
- Verbosity controls (`-v`, `-vv`, `-vvv` or repeated `--verbose`)
- Output to stdout by default
- Optional file output (`--output` or `--write-file` for `sitemap.xml`)

## Requirements

- Rust toolchain (stable)

## Build

```bash
cargo build --release
```

## Usage

Print sitemap to stdout:

```bash
cargo run -- https://example.com
```

Write sitemap to `sitemap.xml`:

```bash
cargo run -- https://example.com --write-file
```

Write sitemap to a custom file:

```bash
cargo run -- https://example.com --output my-sitemap.xml
```

Tune crawl limits:

```bash
cargo run -- https://example.com --max-pages 1000 --max-depth 10
```

Tune concurrency:

```bash
cargo run -- https://example.com --concurrency 32
```

Enable logs:

```bash
# info-level logs
cargo run -- https://example.com -v

# debug-level logs
cargo run -- https://example.com -vv

# trace-level logs
cargo run -- https://example.com -vvv
```

With logging enabled, sitemap XML still goes to stdout while structured logs go to stderr.
That means shell redirection like `> sitemap.xml` captures XML only.

## Example output

```xml
<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url>
    <loc>https://example.com/</loc>
  </url>
</urlset>
```

## Test

```bash
cargo test
```

The integration test spins up a local HTTP server and verifies recursive crawling, deduplication, and same-site filtering behavior.

## GitHub Actions

This repository includes two workflows:

- CI: [.github/workflows/ci.yml](.github/workflows/ci.yml)
  - Runs on pushes to `main` and pull requests
  - Checks formatting (`cargo fmt --check`)
  - Runs clippy with warnings denied
  - Runs tests on Linux, macOS, and Windows
  - Ensures release builds compile on Linux, macOS, and Windows

- Release: [.github/workflows/release.yml](.github/workflows/release.yml)
  - Runs when you push a tag matching `v*` (for example `v0.1.0`)
  - Builds release binaries on Linux, macOS, and Windows
  - Packages artifacts (`.tar.gz` for Unix, `.zip` for Windows)
  - Publishes a GitHub Release with generated notes and attached binaries

Create a release by tagging and pushing:

```bash
git tag v0.1.0
git push origin v0.1.0
```

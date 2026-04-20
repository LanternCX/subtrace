# subroute

Generate an HTML route report for proxy subscription nodes.

## Quick Start

### 1. Install Rust

Install the Rust toolchain from <https://rustup.rs/>.

### 2. Run from source

```bash
cargo run -- "https://example.com/subscription"
```

The command fetches the subscription, probes each parsed node, enriches route hops with AS information, and writes an HTML report in the current directory.

### 3. Choose an output path

```bash
cargo run -- "https://example.com/subscription" --output report.html
```

### 4. Control probing concurrency

```bash
cargo run -- "https://example.com/subscription" --concurrency 8
```

When `--concurrency` is omitted, all parsed nodes are probed concurrently.

### 5. Control traceroute depth

```bash
cargo run -- "https://example.com/subscription" --max-hops 20
```

### 6. Build a release binary

```bash
cargo build --release
./target/release/subroute "https://example.com/subscription" --output report.html
```

## Notes

- The tool expects a Base64 proxy subscription URL.
- The generated report is a standalone HTML file.
- Route probing relies on system `ping` and `traceroute` commands.
- AS metadata lookup is performed for discovered route IPs.

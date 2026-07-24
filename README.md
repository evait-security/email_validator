# 📧 Email Validator

> Fast, statically linked email list validator in Rust — CLI pipeline & HTTP API.

**Email Validator** extracts, deduplicates, and validates email addresses from any
text source (TXT, Markdown, XML, CSV, STDIN pipes). Three validation modes:
regex only, MX lookup, or full SMTP handshake. Ships as a single static binary
with both a **CLI pipeline** and a built-in **HTTP API server**.

---

## 🚀 Quickstart

```bash
# Simplest usage: file in, validated list out
email_validator run -i input.txt -o verified.txt

# Regex-only validation (no network)
email_validator run -i mails.txt -m regex

# GoPhish CSV output format
email_validator run -i mails.txt -o out.csv -f gophish

# JSON output for scripting / automation
email_validator run -i mails.txt -j | jq

# Via pipe (STDIN)
cat mails.txt | email_validator run -f list
```

---

## 🌐 HTTP API Server

The binary includes a built-in HTTP API via the `api` subcommand:

```bash
# Start the server (defaults to 0.0.0.0:8080)
email_validator api

# Custom bind address
email_validator api 127.0.0.1:3000

# Or via environment variable
BIND_ADDR=0.0.0.0:9000 email_validator api
```

### Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/health` | Health check — returns `{"status":"ok","version":"0.4.0"}` |
| `GET` | `/validate?email=…&method=…` | Validate a single email |
| `POST` | `/validate` | Batch validate up to 1000 emails |

### POST `/validate` Request

```json
{
  "emails": ["alice@example.com", "bogus", "bob@test.de"],
  "method": "regex",
  "disable_wildcard": false
}
```

### Response

```json
{
  "total": 3,
  "valid_count": 2,
  "invalid_count": 1,
  "catch_all_count": 0,
  "results": [
    { "email": "alice@example.com", "valid": true },
    { "email": "bogus", "valid": false },
    { "email": "bob@test.de", "valid": true }
  ]
}
```

- Emails are **deduplicated case-insensitively** before validation
- **Max 1000 emails** per request — returns HTTP 400 if exceeded
- Empty list returns HTTP 400
- `method` and `disable_wildcard` are optional (default: `smtp` / `false`)

---

## 🐳 Docker / Wolfi

```dockerfile
# Containerfile
FROM cgr.dev/chainguard/wolfi-base:latest
COPY email_validator /usr/local/bin/email_validator
EXPOSE 8080
USER nonroot
ENTRYPOINT ["email_validator", "api"]
```

```yaml
# docker-compose.yml
services:
  email-validator:
    image: ghcr.io/evait-security/email_validator:latest
    ports:
      - "8080:8080"          # Host:Container
    restart: unless-stopped
    read_only: true

  # Optional: auto-update container on new releases
  watchtower:
    image: containrrr/watchtower:latest
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
    command: --interval 300 email-validator
    restart: unless-stopped
```

The image is built automatically on every release and pushed to
`ghcr.io/evait-security/email_validator`. Add Watchtower to auto-update.

---

## 📥 Download (Standalone Binary)

The binary is **statically linked** (musl) and runs on **any Linux x86_64** —
Alpine, Wolfi, Arch, Debian, Ubuntu, CentOS, embedded systems.
No glibc, no runtime dependencies.

👉 **[Download latest release](../../releases/latest)**

Simply make it executable and go:

```bash
chmod +x email_validator
./email_validator run -i emails.txt -o clean.txt
```

---

## 🛠️ CLI Reference

### `email_validator run` — CLI Pipeline

| Flag | Description | Default |
|------|-------------|---------|
| `-i` | Input file (optional, STDIN otherwise) | `—` |
| `-o` | Output file (optional, STDOUT otherwise) | `—` |
| `-m` | Validation method: `regex`, `mx`, `smtp` | `smtp` |
| `-f` | Output format: `list`, `gophish` | `list` |
| `-j` | JSON array output (conflicts with `-f`) | `false` |
| `-d` | Disable wildcard domain check | `false` |
| `-v` | Verbose mode | `false` |

### `email_validator api` — HTTP Server

| Arg / Env | Description | Default |
|-----------|-------------|---------|
| `[BIND_ADDR]` | Address to bind (positional or `$BIND_ADDR` env) | `0.0.0.0:8080` |
| `-v` | Verbose STDERR logging | `false` |

```bash
# All equivalent:
email_validator api
email_validator api 127.0.0.1:3000
BIND_ADDR=127.0.0.1:3000 email_validator api
```

---

## 🔍 Validation Methods

| Method | Description | Network |
|--------|-------------|---------|
| `regex` | Syntax check via RFC-compliant regex | ❌ |
| `mx`   | Regex + MX record lookup of domain | ✅ |
| `smtp` | Regex + MX + SMTP handshake (RCPT TO) | ✅ |

---

## � Output Formats

| Flag | Format | Includes |
|------|--------|----------|
| `-f list` (default) | One valid email per line | Valid only |
| `-f gophish` | CSV: `First Name,Last Name,Email,Position` | Valid only |
| `-j` / `--json` | JSON array | All emails (valid + invalid) |

### JSON Output (`-j`)

Designed for scripting, automation pipelines, and API consumption. Each email is an object
with `email`, `valid`, and optionally `catch_all` (only present when `true`):

```json
[
  { "email": "alice@example.com", "valid": true },
  { "email": "bob@catch-all.tld", "valid": true, "catch_all": true },
  { "email": "nobody@no-mx-xyz123.de", "valid": false }
]
```

Use with `jq` for filtering and transformation:
```bash
# Extract only valid emails
email_validator -i mails.txt -j -m smtp | jq '[.[] | select(.valid)]'

# Pipe directly into a webhook or file
email_validator -i mails.txt -j -o result.json
```

---

## �📋 Supported Input Formats

The regex parser reliably extracts emails from:

- **TXT** — prose, lists, CSV exports
- **Markdown** — links, code blocks, tables, `mailto:` links
- **XML** — attributes, CDATA sections, text nodes
- **HTML** — tags, attributes, plaintext
- Any **mixed content** with noise, special characters, and broken entries

Duplicates (including case-insensitive variants) are automatically detected and removed.

---

## 🧪 Build from Source

```bash
# Clone the repository
git clone https://github.com/USERNAME/email_validator.git
cd email_validator

# Build a static binary (requires musl toolchain)
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl

# Optional: compress with UPX (~8 MB → ~3 MB)
upx --best --lzma target/x86_64-unknown-linux-musl/release/email_validator
```

Run tests (53 total, all green ✅):

```bash
cargo test
```

---

## � Architecture

### Program Flow

![Program Flow](doc/email_validator_flow.png)

### Sequence Diagram

![Sequence Diagram](doc/program_sequence.png)

```bash
# Regenerate diagrams (requires plantuml)
cd doc && plantuml *.puml
```

---

## 🛠️ Developer Documentation

Module-level docs for all internal types and functions.

### Browse Online

👉 **[Developer Docs](https://evait-security.github.io/email_validator/email_validator/)** — live on GitHub Pages.

### Build Locally

```bash
cargo doc --no-deps --open
```

### Download

A tarball of the docs is also attached to every [release](../../releases/latest)
as `email_validator_docs.tar.gz`.

This opens a local browser with docs for `ingestion`, `precheck`,
`validation`, `output`, and all public types.

---

## �📜 License

This project is licensed under the **MIT License**.

- [Full license text](LICENSE)
- [What MIT means (choosealicense.com)](https://choosealicense.com/licenses/mit/)

You may copy, modify, distribute, and use it in your own projects
(including commercial software) with proper attribution.

---

## 🧬 Tech Stack

- **Rust** (Edition 2024)
- **musl** — fully static linking
- **UPX** — binary compression for minimal download size
- **axum** — HTTP API server
- **Property-Based Testing** via `proptest` for fuzzing Markdown/XML/noise inputs
- **CI/CD** via GitHub Actions (tests + automatic release)

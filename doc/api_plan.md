# Plan: `api` Server-Modus für email_validator (überarbeitet)

**TL;DR:** Füge einen `api` Subcommand hinzu, der statt der CLI-Pipeline einen HTTP-Server (axum) startet. Validierungs-Parameter (`method`, `disable_wildcard`) werden pro API-Call im Request-Body/Query übergeben. Output ist **immer JSON**. Drei Endpunkte: `POST /validate` (Batch), `GET /validate?email=` (Einzel), `GET /health`.

## Sicherheitsprinzipien

- **Keine Datenpersistenz:** Emails kommen per API rein, Ergebnisse gehen per API raus — nichts wird auf Disk gespeichert
- **SSRF-Schutz bereits vorhanden:** `validation.rs::is_private_ip()` blockt private IPs (RFC 1918, loopback, link-local, CGN) — verhindert Zugriff auf interne Systeme
- **Keine Authentifizierung:** Aufruf erfolgt über nicht-öffentliche IP (z.B. `127.0.0.1` oder internes Netz)
- **Batch-Limit 1000:** Wird **vor** jeglicher Verarbeitung geprüft — bei Überschreitung sofort HTTP 400
- **Deduplizierung im API-Modus:** Trotz CLI-Auskopplung werden Emails dedupliziert (Schutz vor DoS durch Duplikate, billig bei 1000 Einträgen)

## Entscheidungen

- **Subcommand-Ansatz:** `email_validator run` (CLI) vs `email_validator api` (Server) — saubere Trennung, keine `conflicts_with`-Probleme mit Default-Werten
- axum 0.7 als HTTP-Framework (minimal, kein tower-http/CORS nötig)
- Validierungs-Flags pro Request, nicht global beim Server-Start
- Output-Format im API-Modus **immer JSON** — kein `format`-Parameter
- `-v` (verbose) steuert STDERR-Logging im Server-Modus
- `Method`-Enum bekommt `serde::Deserialize` für JSON-Parsing
- `precheck::run()` und `validation::run()` werden von `&Cli` entkoppelt (Option A: einzelne Parameter)
- Fehler-Handling: Malformed JSON → 422 (axum auto), fehlendes `emails` → 400, >1000 → 400, leere Liste → 400

---

## Phase 1: Abhängigkeiten & bestehenden Code entkoppeln

### Schritt 1.1: `Cargo.toml` — axum hinzufügen

- `axum = "0.7"` — HTTP-Framework
- `tokio = { version = "1", features = ["full"] }` — bereits vorhanden
- `serde` + `serde_json` — bereits vorhanden
- Kein `tower-http`, kein `tracing` — Overkill für internen Service
- `reqwest = "0.12"` als `dev-dependency` für Integrationstests

### Schritt 1.2: `Method`-Enum um `Deserialize` erweitern (`src/main.rs`)

- `#[derive(Serialize, Deserialize)]` zu `Method` hinzufügen (`Serialize` schon via `serde`)
- `serde::Deserialize` in den Import aufnehmen
- Gleiches für `Format`? Nein — `Format` wird im API-Modus nicht verwendet

### Schritt 1.3: `precheck::run()` entkoppeln (`src/precheck.rs`)

Signatur ändern von:
```rust
pub async fn run(cli: &Cli, unique_emails: &[String], is_quiet: bool) -> HashSet<String>
```
zu:
```rust
pub async fn run(method: Method, disable_wildcard: bool, verbose: bool, unique_emails: &[String], is_quiet: bool) -> HashSet<String>
```
- Interne Verwendung von `cli.method` → `method`, `cli.disable_wildcard` → `disable_wildcard`, `cli.verbose` → `verbose`

### Schritt 1.4: `validation::run()` entkoppeln (`src/validation.rs`)

Signatur ändern von:
```rust
pub async fn run(cli: &Cli, unique_emails: &[String], wildcard_domains: &HashSet<String>, is_quiet: bool) -> Vec<ValidationResult>
```
zu:
```rust
pub async fn run(method: Method, disable_wildcard: bool, unique_emails: &[String], wildcard_domains: &HashSet<String>, is_quiet: bool) -> Vec<ValidationResult>
```
- Interne Verwendung von `cli.method` → `method`, `cli.disable_wildcard` → `disable_wildcard`

### Schritt 1.5: `main()`-Aufrufe anpassen

- `precheck::run(&cli, ...)` → `precheck::run(cli.method, cli.disable_wildcard, cli.verbose, ...)`
- `validation::run(&cli, ...)` → `validation::run(cli.method, cli.disable_wildcard, ...)`

### Schritt 1.6: Bestehende Tests prüfen

- `cargo test` muss nach 1.3–1.5 weiterhin grün sein

*Parallel:* Schritte 1.3 und 1.4 können parallel laufen. 1.5 hängt von beiden ab. 1.2 und 1.6 sind unabhängig.

---

## Phase 2: CLI auf Subcommands umbauen

### Schritt 2.1: `Cli`-Struct in Enum umwandeln (`src/main.rs`)

```rust
#[derive(Parser, Debug)]
#[command(name = "verify")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Run the CLI pipeline (file/STDIN in → file/STDOUT out)
    Run(RunArgs),
    /// Start HTTP API server
    Api(ApiArgs),
}

#[derive(Parser, Debug)]
pub struct RunArgs {
    #[arg(short, long)]
    pub input: Option<String>,
    #[arg(short, long)]
    pub output: Option<String>,
    #[arg(short, long, value_enum, default_value_t = Format::List)]
    pub format: Format,
    #[arg(short = 'j', long, conflicts_with = "format")]
    pub json: bool,
    #[arg(short, long, value_enum, default_value_t = Method::Smtp)]
    pub method: Method,
    #[arg(short, long)]
    pub disable_wildcard: bool,
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Parser, Debug)]
pub struct ApiArgs {
    /// Bind address (e.g. 127.0.0.1:8080)
    pub bind_addr: String,
    /// Verbose STDERR logging
    #[arg(short, long)]
    pub verbose: bool,
}
```

### Schritt 2.2: `main()`-Verzweigung anpassen

```rust
match cli.command {
    Command::Run(args) => {
        // bestehende Pipeline-Logik, mit args statt cli
    }
    Command::Api(args) => {
        server::run(&args.bind_addr, args.verbose).await
    }
}
```

### Schritt 2.3: Bestehende CLI-Tests aktualisieren

- Alle `Cli::parse_from(...)` Tests auf `RunArgs`-basiertes Parsing umstellen
- Beispiel: `email_validator -i list.txt` → `email_validator run -i list.txt`

### Schritt 2.4: Hilfetexte aktualisieren

- `after_help` an Subcommand-Struktur anpassen
- `cargo run -- --help` zeigt `run` und `api` als Subcommands

---

## Phase 3: Neues Modul `src/server.rs`

### Schritt 3.1: Request/Response-Typen definieren

```rust
use serde::{Deserialize, Serialize};

// POST /validate Body
#[derive(Deserialize)]
struct ValidateRequest {
    emails: Vec<String>,              // required, wird extra validiert
    method: Option<Method>,           // default: Smtp
    disable_wildcard: Option<bool>,   // default: false
}

// GET /validate Query-Params
#[derive(Deserialize)]
struct SingleValidateQuery {
    email: String,
    method: Option<Method>,
    disable_wildcard: Option<bool>,
}

// GET /health Response
#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: String,
}
```

### Schritt 3.2: `POST /validate` Handler

Reihenfolge der Validierung (**wichtig: Batch-Limit VOR Verarbeitung!**):

1. **Leere Liste:** `emails.is_empty()` → 400 `{"error": "email list is empty"}`
2. **Batch-Limit:** `emails.len() > 1000` → 400 `{"error": "too many emails, max 1000"}`
3. **Deduplizierung:** `emails.sort(); emails.dedup();` (oder via `HashSet`)
4. **Defaults setzen:** `method = request.method.unwrap_or(Method::Smtp)`, `disable_wildcard = request.disable_wildcard.unwrap_or(false)`
5. `precheck::run(method, disable_wildcard, verbose, &emails, is_quiet).await`
6. `validation::run(method, disable_wildcard, &emails, &wildcard_domains, is_quiet).await`
7. Response bauen mit Summary (`total`, `valid_count`, `invalid_count`, `catch_all_count`, `results`)
8. Concurrency über `FuturesUnordered` + `Arc<Semaphore>` mit Limit 25

**Wichtig:** `is_quiet = !verbose` im API-Modus, da STDERR-Output im Server-Kontext stört.

### Schritt 3.3: `GET /validate?email=` Handler

- Einzelne Email, gleiche Pipeline wie Batch
- Leere/fehlende Email → 400
- Response als Array mit einem Element (Konsistenz mit Batch-Format)

### Schritt 3.4: `GET /health` Handler

```json
{ "status": "ok", "version": "0.3.0" }
```

- `version` aus `env!("CARGO_PKG_VERSION")` lesen, nicht hartcodieren

### Schritt 3.5: Server-Router & `run()`-Funktion

```rust
pub async fn run(bind_addr: &str, verbose: bool) {
    let app = Router::new()
        .route("/validate", post(validate_batch).get(validate_single))
        .route("/health", get(health));

    let listener = tokio::net::TcpListener::bind(bind_addr).await.unwrap();
    eprintln!("API server listening on http://{bind_addr}");
    axum::serve(listener, app).await.unwrap();
}
```

- `is_quiet = !verbose` wird über AppState in die Handler gegeben

### Schritt 3.6: Shared State für Handler

```rust
#[derive(Clone)]
struct AppState {
    verbose: bool,
}

async fn validate_batch(
    State(state): State<AppState>,
    Json(request): Json<ValidateRequest>,
) -> Result<Json<Value>, StatusCode> { ... }
```

- Fehler-Response immer mit JSON-Body (nicht nur StatusCode)

### Schritt 3.7: Fehler-Response-Struktur

```rust
#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}
```

Einheitliche Fehler über alle Endpunkte:
- **400:** Validierungsfehler (leere Liste, zu viele Emails, fehlender Parameter)
- **422:** Malformed JSON (axum handled das automatisch via `Json`-Extractor)
- **405:** Falsche HTTP-Methode (axum handled das automatisch)
- **404:** Unbekannte Route (axum handled das automatisch)

---

## Phase 4: Concurrency in validation.rs & precheck.rs

### Schritt 4.1: `src/validation.rs` — `run()` um `concurrency: Option<usize>` erweitern

- `Some(n)`: `FuturesUnordered` + `Arc<Semaphore>` mit Limit `n`
- `None`: sequentiell (CLI-Modus, unverändert)

### Schritt 4.2: `src/precheck.rs` — gleiche Änderung

### Schritt 4.3: Aufruf

- API-Modus: `concurrency = Some(25)`
- CLI-Modus: `concurrency = None`

---

## Phase 5: Tests & Verifikation

### Schritt 5.1: Unit-Tests in `src/server.rs` (mit `#[cfg(test)]`)

- `POST /validate` mit leerer Liste → 400 + `{"error":"email list is empty"}`
- `POST /validate` mit >1000 Emails → 400 + `{"error":"too many emails, max 1000"}`
- `POST /validate` mit 1001 Emails → **sofort** 400 (vor jeglicher Verarbeitung)
- `POST /validate` mit validem Body → 200 + korrekte JSON-Response-Struktur
- `POST /validate` ohne `emails`-Feld → 422 (axum auto)
- `POST /validate` mit Malformed JSON → 422 (axum auto)
- `GET /validate?email=test@example.com` → 200 + einzelnes Result im Array
- `GET /validate` ohne `email` → 400
- `GET /health` → 200 + `{"status":"ok","version":"0.3.0"}`

### Schritt 5.2: Integrationstest (`tests/api_e2e.rs`)

- `reqwest` als `dev-dependency`
- Server in eigenem Thread starten, dann Requests senden
- Mindestens: Health, Batch-Validierung, Einzel-Validierung, Fehlerfälle

### Schritt 5.3: Manuelle Verifikation

```bash
# Neues CLI-Format testen
cargo run -- run -i test.txt -m regex
cargo run -- api 127.0.0.1:8080

# API testen
curl -s -X POST http://127.0.0.1:8080/validate \
  -H 'Content-Type: application/json' \
  -d '{"emails":["test@example.com"],"method":"regex"}' | jq

curl -s 'http://127.0.0.1:8080/validate?email=test@example.com' | jq

curl -s http://127.0.0.1:8080/health | jq

# Batch-Limit testen (muss SOFORT 400 geben)
curl -s -X POST http://127.0.0.1:8080/validate \
  -H 'Content-Type: application/json' \
  -d '{"emails":["a@b.com","c@d.com","x@y.com"]}' | jq
```

### Schritt 5.4: Regressionstests

- `cargo test` — alle bestehenden + neuen Tests grün (53 total)
- `cargo run -- run -i test.txt` — CLI-Modus unverändert
- `echo "test@example.com" | cargo run -- run` — STDIN-Modus unverändert

---

## Abweichungen vom Plan (tatsächliche Implementierung)

| Abweichung | Grund |
|---|---|
| Concurrency (`_concurrency`) ist Platzhalter, nicht implementiert | `check-if-email-exists` hat kein `Send` auf `CheckEmailInput`, macht `FuturesUnordered` schwierig — wird später gelöst |
| `Method::Regex` validiert jetzt tatsächlich per Regex (`EMAIL_RE.is_match(email)`) | Im API-Modus läuft `ingestion` nicht, daher muss die Validation selbst prüfen |
| API-Deduplizierung mit `to_lowercase()` | Case-insensitive Dedup wie in `ingestion.rs` |
| `ApiArgs.bind_addr` optional mit `default_value = "0.0.0.0:8080"` + `env = "BIND_ADDR"` | Container-freundlich: kein CLI-Arg nötig |
| `clap` Feature `"env"` hinzugefügt | Für `BIND_ADDR` Env-Var-Support |
| Fehler: Malformed JSON gibt 400 statt 422 | axum's `Json`-Extractor mapped das automatisch auf 400 (nicht 422) |

## Aktueller Stand (2026-07-24)

- **53 Tests** (28 unit + 8 API E2E + 5 fuzz + 8 mass-input + 4 SMTP boundary) — alle grün
- **0 Build-Warnings**
- CLI: `email_validator run [OPTIONS]`
- API: `email_validator api [BIND_ADDR]` mit `BIND_ADDR` env-var
- Docker/Wolfi-ready: `ENTRYPOINT ["email_validator", "api"]` ohne weitere Args

---

## Relevante Dateien

| Datei | Änderung |
|---|---|
| `Cargo.toml` | `axum = "0.7"`, `reqwest = "0.12"` (dev), `clap` + `"env"` feature |
| `src/main.rs` | Subcommand-Enum, `Method` + `Deserialize`, `main()`-Verzweigung, Tests anpassen, `ApiArgs.bind_addr` mit `default_value` + `env = "BIND_ADDR"` |
| `src/server.rs` | **NEU** — Router, Handler, Request/Response-Typen, Fehler-Handling, Tests |
| `src/validation.rs` | Signatur entkoppelt (`&Cli` → einzelne Parameter), `_concurrency: Option<usize>`, `Method::Regex` validiert jetzt tatsächlich per `EMAIL_RE.is_match()` statt blind `true` |
| `src/precheck.rs` | Signatur entkoppelt (`&Cli` → einzelne Parameter), `_concurrency: Option<usize>` |
| `tests/api_e2e.rs` | **NEU** — Integrationstests mit reqwest |

## Verifikation (geordnet)

1. `cargo build` — kompiliert ohne Fehler
2. `cargo test` — alle bestehenden + neuen Tests grün
3. `cargo run -- run -i test.txt -m regex` — CLI-Modus unverändert
4. `echo "test@example.com" | cargo run -- run` — STDIN-Modus unverändert
5. `cargo run -- api 127.0.0.1:8080` + curl-Tests (alle 5 Szenarien aus Schritt 5.3)
6. Batch-Limit-Test: >1000 Emails → SOFORT 400, keine Email wird verarbeitet

## Ausgeschlossen

- TLS/HTTPS (Reverse-Proxy)
- Authentication/API-Keys (Aufruf über nicht-öffentliche IP)
- Rate-Limiting
- CORS / tower-http
- Datenpersistenz jeglicher Art
- `format`-Parameter (immer JSON)
- `-i`/`-o`/`-f`/`-j` im API-Modus (existieren nicht im `ApiArgs`)
# mw-nexus — Provenance ontology, correlation graph, and advisory Mizan/CRAYON engine.

**Repository:** [`A-L-L-A-H/mw-nexus`](https://github.com/A-L-L-A-H/mw-nexus) · **Stack:** Rust 1.89 · axum · SQLite WAL

## What it does

Enforces seven-domain boundaries where correlation, causation, identity, and fulfillment remain distinct. Mizan output is advisory only.

## Quick start

```bash
cargo run
```

## Verify

```bash
cargo test
```

## Configuration

- **Port:** 4430
- **Key configuration:** MW_NEXUS_ADDR · MW_NEXUS_DB_PATH · RUST_LOG

```bash
docker build -t ghcr.io/a-l-l-a-h/mw-nexus:latest .
docker run -p 4430:4430 ghcr.io/a-l-l-a-h/mw-nexus:latest
```

## Notes

Unknown evidence stays unknown; CHILD_SENSITIVE records fail closed and are excluded from listings.

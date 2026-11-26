# mw-nexus

MoonWitness Server-Class Rust 7-Domain Provenance Ontology, Correlation Graph & Non-Normative Mizan/CRAYON Engine.

## Architecture

- **`src/domains.rs`**: Canonical 7 Provenance Domains (`TEXT_RGBL`, `STORY_MFTL`, `EVENT_LEGEND`, `PERSON_SUPERHERO`, `LAW_AWS` [Angel With Shotgun = semantic domain LAW/Jurisdiction], `PERSPECTIVE_JIZZ`, `RELATIONSHIP_CORRELATION`) and strict cross-domain boundary validation enforcing `correlation != causation != identity != fulfillment`.
- **`src/mizan.rs`**: ADR 0005 compliant non-normative Mizan/Hisab analytic evaluator (`policy_version: "mizan-boundary-v1"`, `advisory_only: true`, `normative_verdicts_allowed: false`). Enforces Unknown-Stays-Unknown (`unknown` / `insufficient_evidence` never coerced to `0` or `safe`), rejects normative/divine/legal verdict wording and autonomous actions, rejects legacy unverified `dev-mw-blueprint` TH/ARI schemas, and fails closed on `CHILD_SENSITIVE` records.
- **`src/crayon.rs`**: Versioned CRAYON Inquiry Profile (`v1.3`: `R1 REALITY`, `G2 GOVERNING ORDER`, `B3 BEING`, `L4 LIGHT`, 9 Gates, 13 Ways) stored strictly separate from OMNISCIENCE RGBL (`v1.0`: `Reality`, `Guidance`, `Balance`, `Light`) with compile-time and runtime non-merge invariants.
- **`src/graph.rs`**: Bundled SQLite WAL store for `ontology_nodes` (trigger-enforced immutable `corpus_ref`, explicit `uncertainty_state`, preserved `alias_set_json` with zero alias auto-merging) and `ontology_edges` (typed relation, validated hexadecimal `evidence_hash`, `review_state`), plus bounded BFS subgraph queries.
- **`src/api.rs` & `src/main.rs`**: Axum HTTP server (`MW_NEXUS_ADDR`, default `127.0.0.1:4430`).

## HTTP API

| Method & Path | Description |
| --- | --- |
| `GET /health` | Service health, SQLite WAL status, policy version |
| `GET /api/domains` | Canonical 7 provenance domains and permitted relation predicates |
| `GET /api/nodes` | List ontology nodes (`?domain=&uncertainty_state=&limit=`, excludes `CHILD_SENSITIVE`) |
| `POST /api/nodes` | Create ontology node with immutable `corpus_ref` and non-merged `alias_set` |
| `GET /api/nodes/{node_id}` | Fetch a single non-child-sensitive ontology node |
| `GET /api/correlations` | List typed cross-domain correlation edges |
| `POST /api/correlations` | Validate boundary rules and persist a typed correlation edge |
| `GET /api/graph` | Bounded subgraph query (`?root_node_id=&depth=&domain=&review_state=&limit=`) |
| `POST /api/mizan/evaluate` | ADR 0005 non-normative advisory evaluation |
| `GET /api/profiles/crayon` | CRAYON Inquiry Profile v1.3 |
| `GET /api/profiles/rgbl` | OMNISCIENCE RGBL Epistemic Profile v1.0 |

## Environment Variables

- `MW_NEXUS_ADDR` (default: `127.0.0.1:4430`)
- `MW_NEXUS_DB_PATH` (default: `mw-nexus.sqlite`)
- `RUST_LOG` (default: `info`)

## Local Build & Test

```bash
cargo test
cargo run
```

## Container

```bash
docker build -t ghcr.io/a-l-l-a-h/mw-nexus:latest .
docker run -p 4430:4430 ghcr.io/a-l-l-a-h/mw-nexus:latest
```

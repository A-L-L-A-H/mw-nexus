use crate::domains::{ProvenanceDomain, UncertaintyState};
use crate::graph::{CreateNodeRequest, GraphError, NexusGraphStore, OntologyNode};
use crate::mizan::SensitivityClass;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::TcpStream;
use thiserror::Error;

pub const DEFAULT_CORPUS_ADDR: &str = "127.0.0.1:4420";

#[derive(Debug, Error)]
pub enum CorpusSyncError {
    #[error("corpus HTTP connection error: {0}")]
    Io(#[from] std::io::Error),

    #[error("corpus HTTP response parse error: {0}")]
    Parse(String),

    #[error("corpus returned HTTP status {status}: {body}")]
    UpstreamStatus { status: u16, body: String },

    #[error("graph store error during corpus sync: {0}")]
    Graph(#[from] GraphError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CorpusPassageRecord {
    pub kind: String,
    pub epistemic_layer: String,
    pub edition_id: String,
    #[serde(default)]
    pub expression_id: Option<String>,
    #[serde(default)]
    pub source_id: Option<String>,
    pub locale: String,
    pub locator: String,
    pub text: String,
    pub sha256: String,
    pub attribution: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusPassagesResponse {
    pub count: usize,
    pub passages: Vec<CorpusPassageRecord>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CorpusSyncRequest {
    #[serde(default)]
    pub corpus_addr: Option<String>,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusSyncResponse {
    pub corpus_addr: String,
    pub fetched_count: usize,
    pub created_count: usize,
    pub skipped_existing_count: usize,
    pub synced_nodes: Vec<OntologyNode>,
}

/// Performs a lightweight HTTP/1.1 GET request against `mw-corpus` (default `127.0.0.1:4420`).
pub async fn fetch_corpus_passages(
    corpus_addr: &str,
    query: Option<&str>,
    kind: Option<&str>,
) -> Result<CorpusPassagesResponse, CorpusSyncError> {
    let mut path = "/api/passages".to_owned();
    let mut params = Vec::new();
    if let Some(k) = kind.filter(|val| !val.trim().is_empty()) {
        params.push(format!("kind={}", k.trim()));
    }
    if let Some(q) = query.filter(|val| !val.trim().is_empty()) {
        let encoded: String = q
            .trim()
            .bytes()
            .map(|b| {
                if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b':' {
                    (b as char).to_string()
                } else {
                    format!("%{b:02X}")
                }
            })
            .collect();
        params.push(format!("q={encoded}"));
    }
    if !params.is_empty() {
        path = format!("{path}?{}", params.join("&"));
    }

    let addr = corpus_addr
        .trim()
        .strip_prefix("http://")
        .unwrap_or(corpus_addr.trim())
        .trim_end_matches('/')
        .to_owned();

    let response_text = tokio::task::spawn_blocking(move || -> Result<String, std::io::Error> {
        let mut stream = TcpStream::connect(&addr)?;
        let request_bytes = format!(
            "GET {path} HTTP/1.1\r\nHost: {addr}\r\nAccept: application/json\r\nConnection: close\r\n\r\n"
        );
        stream.write_all(request_bytes.as_bytes())?;
        let mut raw_response = Vec::new();
        stream.read_to_end(&mut raw_response)?;
        Ok(String::from_utf8_lossy(&raw_response).into_owned())
    })
    .await
    .map_err(|join_err| CorpusSyncError::Parse(join_err.to_string()))??;

    let (headers_part, body_part) = response_text
        .split_once("\r\n\r\n")
        .ok_or_else(|| CorpusSyncError::Parse("missing HTTP header terminator".to_owned()))?;

    let status_line = headers_part.lines().next().unwrap_or_default();
    let status_code: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|token| token.parse::<u16>().ok())
        .ok_or_else(|| CorpusSyncError::Parse(format!("invalid HTTP status line: {status_line}")))?;

    let decoded_body = if headers_part
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        decode_chunked_body(body_part)
    } else {
        body_part.trim().to_owned()
    };

    if !(200..300).contains(&status_code) {
        return Err(CorpusSyncError::UpstreamStatus {
            status: status_code,
            body: decoded_body,
        });
    }

    serde_json::from_str::<CorpusPassagesResponse>(&decoded_body)
        .map_err(|err| CorpusSyncError::Parse(err.to_string()))
}

fn decode_chunked_body(raw_body: &str) -> String {
    let mut output = String::new();
    let mut rest = raw_body;
    while let Some((size_hex, after_size)) = rest.split_once("\r\n") {
        let trimmed_size = size_hex.trim();
        let Ok(chunk_len) = usize::from_str_radix(trimmed_size, 16) else {
            return raw_body.trim().to_owned();
        };
        if chunk_len == 0 {
            break;
        }
        if after_size.len() < chunk_len {
            output.push_str(after_size);
            break;
        }
        output.push_str(&after_size[..chunk_len]);
        let remainder = &after_size[chunk_len..];
        rest = remainder.strip_prefix("\r\n").unwrap_or(remainder);
    }
    if output.is_empty() {
        raw_body.trim().to_owned()
    } else {
        output
    }
}

/// Synchronizes `mw-corpus` passages into `NexusGraphStore` as `TEXT_RGBL` nodes with
/// immutable `mw-corpus:sha256:<sha256>` references and `HumanReviewed` state.
pub fn sync_passages_into_store(
    store: &NexusGraphStore,
    corpus_addr: &str,
    passages: Vec<CorpusPassageRecord>,
) -> Result<CorpusSyncResponse, CorpusSyncError> {
    let fetched_count = passages.len();
    let mut created_count = 0usize;
    let mut skipped_existing_count = 0usize;
    let mut synced_nodes = Vec::new();

    for passage in passages {
        let sanitized_locator: String = passage
            .locator
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() {
                    ch.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect();
        let node_id = format!(
            "corpus-{}-{}",
            passage.edition_id.to_ascii_lowercase(),
            sanitized_locator.trim_matches('-')
        );
        let corpus_ref = format!("mw-corpus:sha256:{}", passage.sha256);

        match store.get_node(&node_id) {
            Ok(existing) => {
                skipped_existing_count += 1;
                synced_nodes.push(existing);
            }
            Err(GraphError::NodeNotFound { .. }) => {
                let created = store.create_node(CreateNodeRequest {
                    id: Some(node_id),
                    domain: ProvenanceDomain::TextRgbl,
                    canonical_label: format!("{} ({})", passage.locator, passage.edition_id),
                    corpus_ref,
                    uncertainty_state: Some(UncertaintyState::HumanReviewed),
                    alias_set: Some(vec![passage.locator.clone(), passage.edition_id.clone()]),
                    sensitivity_class: Some(SensitivityClass::Internal),
                    metadata: Some(serde_json::json!({
                        "kind": passage.kind,
                        "epistemic_layer": passage.epistemic_layer,
                        "locale": passage.locale,
                        "attribution": passage.attribution,
                        "sha256": passage.sha256,
                        "synced_from": corpus_addr,
                    })),
                })?;
                created_count += 1;
                synced_nodes.push(created);
            }
            Err(other_err) => return Err(CorpusSyncError::Graph(other_err)),
        }
    }

    Ok(CorpusSyncResponse {
        corpus_addr: corpus_addr.to_owned(),
        fetched_count,
        created_count,
        skipped_existing_count,
        synced_nodes,
    })
}

pub async fn sync_from_corpus_server(
    store: &NexusGraphStore,
    request: CorpusSyncRequest,
) -> Result<CorpusSyncResponse, CorpusSyncError> {
    let addr = request
        .corpus_addr
        .unwrap_or_else(|| DEFAULT_CORPUS_ADDR.to_owned());
    let response = fetch_corpus_passages(
        &addr,
        request.query.as_deref(),
        request.kind.as_deref(),
    )
    .await?;
    sync_passages_into_store(store, &addr, response.passages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[tokio::test]
    async fn fetches_passages_from_mock_corpus_and_syncs_text_rgbl_nodes_idempotently() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock corpus");
        let addr = listener.local_addr().expect("local addr").to_string();

        std::thread::spawn(move || {
            if let Ok((mut socket, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = socket.read(&mut buf);
                let payload = serde_json::json!({
                    "count": 1,
                    "passages": [{
                        "kind": "quran",
                        "epistemicLayer": "source_text",
                        "editionId": "quran-uthmani-tanzil-v1",
                        "locale": "ar",
                        "locator": "Q7:46",
                        "text": "وَبَيْنَهُمَا حِجَابٌ",
                        "sha256": "6ff83b01099fb7992b5f7fa101cfab3d6556607ca6b1fe69d53cb76f97027760",
                        "attribution": "Tanzil Project"
                    }]
                });
                let body = payload.to_string();
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = socket.write_all(resp.as_bytes());
            }
        });

        let store = NexusGraphStore::open_in_memory().expect("in-memory store");
        let sync_res = sync_from_corpus_server(
            &store,
            CorpusSyncRequest {
                corpus_addr: Some(addr.clone()),
                query: Some("Q7:46".to_owned()),
                kind: Some("quran".to_owned()),
            },
        )
        .await
        .expect("sync succeeds");

        assert_eq!(sync_res.fetched_count, 1);
        assert_eq!(sync_res.created_count, 1);
        assert_eq!(sync_res.skipped_existing_count, 0);
        assert_eq!(
            sync_res.synced_nodes[0].corpus_ref,
            "mw-corpus:sha256:6ff83b01099fb7992b5f7fa101cfab3d6556607ca6b1fe69d53cb76f97027760"
        );

        let second = sync_passages_into_store(
            &store,
            &addr,
            vec![CorpusPassageRecord {
                kind: "quran".to_owned(),
                epistemic_layer: "source_text".to_owned(),
                edition_id: "quran-uthmani-tanzil-v1".to_owned(),
                expression_id: None,
                source_id: None,
                locale: "ar".to_owned(),
                locator: "Q7:46".to_owned(),
                text: "وَبَيْنَهُمَا حِجَابٌ".to_owned(),
                sha256: "6ff83b01099fb7992b5f7fa101cfab3d6556607ca6b1fe69d53cb76f97027760".to_owned(),
                attribution: "Tanzil Project".to_owned(),
            }],
        )
        .expect("idempotent resync");
        assert_eq!(second.created_count, 0);
        assert_eq!(second.skipped_existing_count, 1);
    }
}

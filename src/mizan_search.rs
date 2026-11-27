use crate::domains::{ProvenanceDomain, UncertaintyState};
use crate::graph::{NexusGraphStore, NodeListFilter, OntologyNode};
use crate::mizan::{
    ADR_REFERENCE, MizanRejection, POLICY_VERSION, PROFILE_ID, SensitivityClass,
    validate_mizan_text_boundary,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

pub const DEFAULT_EMBEDDING_DIM: usize = 16;

fn default_uncertainty_state() -> UncertaintyState {
    UncertaintyState::Unknown
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MizanSearchCandidateInput {
    pub id: String,
    pub domain: ProvenanceDomain,
    pub canonical_label: String,
    pub corpus_ref: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub vector: Option<Vec<f64>>,
    #[serde(default = "default_uncertainty_state")]
    pub uncertainty_state: UncertaintyState,
    #[serde(default)]
    pub sensitivity_class: SensitivityClass,
    #[serde(default)]
    pub involves_minor: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MizanHybridSearchRequest {
    pub query: String,
    #[serde(default)]
    pub query_vector: Option<Vec<f64>>,
    #[serde(default)]
    pub domain: Option<ProvenanceDomain>,
    #[serde(default)]
    pub top_k: Option<usize>,
    #[serde(default)]
    pub bm25_weight: Option<f64>,
    #[serde(default)]
    pub vector_weight: Option<f64>,
    #[serde(default)]
    pub sensitivity_class: SensitivityClass,
    #[serde(default)]
    pub involves_minor: bool,
    #[serde(default)]
    pub requested_action: Option<String>,
    #[serde(default)]
    pub candidates: Vec<MizanSearchCandidateInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MizanSearchHit {
    pub id: String,
    pub domain: ProvenanceDomain,
    pub canonical_label: String,
    pub corpus_ref: String,
    pub uncertainty_state: UncertaintyState,
    pub bm25_score: f64,
    pub normalized_bm25_score: f64,
    pub cosine_similarity: f64,
    pub hybrid_score: f64,
    pub ethical_filter_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MizanHybridSearchResponse {
    pub query: String,
    pub top_k: usize,
    pub bm25_weight: f64,
    pub vector_weight: f64,
    pub total_evaluated: usize,
    pub filtered_out_count: usize,
    pub results: Vec<MizanSearchHit>,
    pub advisory_only: bool,
    pub normative_verdicts_allowed: bool,
    pub autonomous_execution_authority: bool,
    pub policy_version: String,
    pub profile_id: String,
    pub adr_reference: String,
    pub disclaimer: String,
}

/// Tokenizes text into lowercase alphanumeric tokens for BM25 lexical matching.
pub fn tokenize_terms(text: &str) -> Vec<String> {
    text.split(|ch: char| !ch.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| part.to_ascii_lowercase())
        .collect()
}

/// Computes deterministic unit-length embedding vector from text using SHA-256 blocks.
pub fn compute_deterministic_vector(text: &str, dimension: usize) -> Vec<f64> {
    let dim = dimension.clamp(1, 256);
    let normalized = text.trim().to_ascii_lowercase();
    let mut values = Vec::with_capacity(dim);
    let mut block_idx = 0usize;

    while values.len() < dim {
        let mut hasher = Sha256::new();
        hasher.update(format!("{block_idx}:{normalized}").as_bytes());
        let digest = hasher.finalize();
        for chunk in digest.chunks_exact(2) {
            if values.len() >= dim {
                break;
            }
            let raw_u16 = u16::from_be_bytes([chunk[0], chunk[1]]) as f64;
            let centered = (raw_u16 / 32767.5) - 1.0;
            values.push(centered);
        }
        block_idx += 1;
    }

    let norm = values.iter().map(|v| v * v).sum::<f64>().sqrt();
    if norm <= f64::EPSILON {
        let mut fallback = vec![0.0; dim];
        fallback[0] = 1.0;
        return fallback;
    }
    values.into_iter().map(|v| v / norm).collect()
}

/// Computes cosine similarity in [0.0, 1.0] (clamped non-negative for ranking).
pub fn compute_cosine_similarity(vec_a: &[f64], vec_b: &[f64]) -> f64 {
    if vec_a.is_empty() || vec_b.is_empty() || vec_a.len() != vec_b.len() {
        return 0.0;
    }
    let dot: f64 = vec_a.iter().zip(vec_b.iter()).map(|(a, b)| a * b).sum();
    let norm_a = vec_a.iter().map(|a| a * a).sum::<f64>().sqrt();
    let norm_b = vec_b.iter().map(|b| b * b).sum::<f64>().sqrt();
    if norm_a <= f64::EPSILON || norm_b <= f64::EPSILON {
        return 0.0;
    }
    let raw_cos = (dot / (norm_a * norm_b)).clamp(-1.0, 1.0);
    // Map cosine similarity to non-negative [0.0, 1.0] where negative correlation is 0.0
    raw_cos.max(0.0)
}

/// Computes Okapi BM25 scores for a slice of tokenized documents against query terms.
pub fn compute_bm25_scores(query_terms: &[String], doc_tokens_list: &[Vec<String>]) -> Vec<f64> {
    if doc_tokens_list.is_empty() || query_terms.is_empty() {
        return vec![0.0; doc_tokens_list.len()];
    }

    let k1 = 1.2_f64;
    let b = 0.75_f64;
    let total_docs = doc_tokens_list.len() as f64;
    let total_tokens: usize = doc_tokens_list.iter().map(Vec::len).sum();
    let avg_dl = (total_tokens as f64 / total_docs).max(1.0);

    let unique_query_terms: Vec<String> = {
        let mut seen = HashSet::new();
        query_terms
            .iter()
            .filter(|t| seen.insert((*t).clone()))
            .cloned()
            .collect()
    };

    let mut doc_freqs: HashMap<String, usize> = HashMap::new();
    for term in &unique_query_terms {
        let df = doc_tokens_list
            .iter()
            .filter(|tokens| tokens.iter().any(|tok| tok == term))
            .count();
        doc_freqs.insert(term.clone(), df);
    }

    doc_tokens_list
        .iter()
        .map(|doc_tokens| {
            if doc_tokens.is_empty() {
                return 0.0;
            }
            let doc_len = doc_tokens.len() as f64;
            let mut tf_map: HashMap<&str, usize> = HashMap::new();
            for tok in doc_tokens {
                *tf_map.entry(tok.as_str()).or_insert(0) += 1;
            }

            let mut score = 0.0_f64;
            for term in &unique_query_terms {
                let tf = *tf_map.get(term.as_str()).unwrap_or(&0) as f64;
                if tf <= 0.0 {
                    continue;
                }
                let df = *doc_freqs.get(term).unwrap_or(&0) as f64;
                let idf = ((total_docs - df + 0.5) / (df + 0.5) + 1.0).ln();
                let numerator = tf * (k1 + 1.0);
                let denominator = tf + k1 * (1.0 - b + b * (doc_len / avg_dl));
                score += idf * (numerator / denominator);
            }
            score
        })
        .collect()
}

struct PreparedCandidate {
    id: String,
    domain: ProvenanceDomain,
    canonical_label: String,
    corpus_ref: String,
    uncertainty_state: UncertaintyState,
    searchable_text: String,
    vector: Option<Vec<f64>>,
}

fn node_to_candidate(node: OntologyNode) -> MizanSearchCandidateInput {
    let alias_text = node.alias_set.join(" ");
    let summary_text = node
        .metadata
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let combined_text = format!(
        "{} {} {} {}",
        node.canonical_label, alias_text, node.corpus_ref, summary_text
    );
    MizanSearchCandidateInput {
        id: node.id,
        domain: node.domain,
        canonical_label: node.canonical_label,
        corpus_ref: node.corpus_ref,
        text: combined_text,
        vector: None,
        uncertainty_state: node.uncertainty_state,
        sensitivity_class: node.sensitivity_class,
        involves_minor: false,
    }
}

/// Executes Hybrid BM25 + Cosine Vector search with fail-closed Mizan ethical filtering (ADR 0005).
/// - Rejects queries or actions containing prohibited normative/divine/legal verdicts or CHILD_SENSITIVE flags.
/// - Filters out candidate records that violate Mizan ethical boundaries or are CHILD_SENSITIVE.
/// - Ranks surviving records by weighted BM25 + Cosine similarity.
pub fn execute_mizan_hybrid_search(
    store: &NexusGraphStore,
    request: &MizanHybridSearchRequest,
) -> Result<MizanHybridSearchResponse, MizanRejection> {
    if request.sensitivity_class == SensitivityClass::ChildSensitive || request.involves_minor {
        return Err(MizanRejection::ChildSensitiveBlocked);
    }

    let trimmed_query = request.query.trim();
    if trimmed_query.is_empty() && request.query_vector.is_none() {
        return Err(MizanRejection::InvalidInput {
            reason: "query or query_vector must be provided".to_owned(),
        });
    }

    // Gate: Query & requested_action must pass Mizan ADR-0005 ethical boundary checks.
    if !trimmed_query.is_empty() {
        validate_mizan_text_boundary("query", trimmed_query)?;
    }
    if let Some(ref action) = request.requested_action {
        validate_mizan_text_boundary("requested_action", action)?;
    }

    let bm25_weight = request.bm25_weight.unwrap_or(0.5);
    let vector_weight = request.vector_weight.unwrap_or(0.5);
    if !bm25_weight.is_finite()
        || !vector_weight.is_finite()
        || bm25_weight < 0.0
        || vector_weight < 0.0
        || (bm25_weight + vector_weight) <= f64::EPSILON
    {
        return Err(MizanRejection::InvalidInput {
            reason: "bm25_weight and vector_weight must be non-negative finite numbers with positive sum".to_owned(),
        });
    }
    let weight_sum = bm25_weight + vector_weight;
    let norm_bm25_w = bm25_weight / weight_sum;
    let norm_vec_w = vector_weight / weight_sum;
    let top_k = request.top_k.unwrap_or(10).clamp(1, 100);

    let mut raw_candidates: Vec<MizanSearchCandidateInput> = request.candidates.clone();
    if raw_candidates.is_empty() {
        let store_nodes = store
            .list_nodes(NodeListFilter {
                domain: request.domain,
                uncertainty_state: None,
                limit: Some(500),
            })
            .map_err(|err| MizanRejection::InvalidInput {
                reason: format!("failed to load ontology nodes for search: {err}"),
            })?;
        raw_candidates = store_nodes.into_iter().map(node_to_candidate).collect();
    }

    let total_evaluated = raw_candidates.len();
    let mut filtered_out_count = 0usize;
    let mut valid_candidates: Vec<PreparedCandidate> = Vec::new();

    for candidate in raw_candidates {
        if let Some(req_domain) = request.domain
            && candidate.domain != req_domain
        {
            continue;
        }
        // Ethical filter on candidate: drop CHILD_SENSITIVE, minor-involved, or normative/autonomous boundary violations.
        if candidate.sensitivity_class == SensitivityClass::ChildSensitive
            || candidate.involves_minor
        {
            filtered_out_count += 1;
            continue;
        }
        let full_text = format!(
            "{} {} {}",
            candidate.canonical_label, candidate.text, candidate.corpus_ref
        );
        if validate_mizan_text_boundary("candidate", &full_text).is_err() {
            filtered_out_count += 1;
            continue;
        }

        valid_candidates.push(PreparedCandidate {
            id: candidate.id,
            domain: candidate.domain,
            canonical_label: candidate.canonical_label,
            corpus_ref: candidate.corpus_ref,
            uncertainty_state: candidate.uncertainty_state,
            searchable_text: full_text,
            vector: candidate.vector,
        });
    }

    let query_terms = tokenize_terms(trimmed_query);
    let doc_tokens: Vec<Vec<String>> = valid_candidates
        .iter()
        .map(|c| tokenize_terms(&c.searchable_text))
        .collect();

    let raw_bm25_scores = compute_bm25_scores(&query_terms, &doc_tokens);
    let max_bm25 = raw_bm25_scores
        .iter()
        .copied()
        .fold(0.0_f64, f64::max);

    let target_dim = request
        .query_vector
        .as_ref()
        .map(|v| v.len().max(1))
        .unwrap_or(DEFAULT_EMBEDDING_DIM);
    let query_vec: Vec<f64> = match &request.query_vector {
        Some(v) if !v.is_empty() => v.clone(),
        _ => compute_deterministic_vector(trimmed_query, target_dim),
    };

    let mut hits: Vec<MizanSearchHit> = valid_candidates
        .into_iter()
        .enumerate()
        .map(|(idx, candidate)| {
            let bm25_raw = raw_bm25_scores.get(idx).copied().unwrap_or(0.0);
            let bm25_norm = if max_bm25 > f64::EPSILON {
                (bm25_raw / max_bm25).clamp(0.0, 1.0)
            } else {
                0.0
            };

            let candidate_vec = match candidate.vector {
                Some(ref v) if v.len() == query_vec.len() => v.clone(),
                _ => compute_deterministic_vector(&candidate.searchable_text, query_vec.len()),
            };
            let cosine_sim = compute_cosine_similarity(&query_vec, &candidate_vec);
            let hybrid = ((norm_bm25_w * bm25_norm + norm_vec_w * cosine_sim).clamp(0.0, 1.0)
                * 10_000.0)
                .round()
                / 10_000.0;

            MizanSearchHit {
                id: candidate.id,
                domain: candidate.domain,
                canonical_label: candidate.canonical_label,
                corpus_ref: candidate.corpus_ref,
                uncertainty_state: candidate.uncertainty_state,
                bm25_score: (bm25_raw * 10_000.0).round() / 10_000.0,
                normalized_bm25_score: (bm25_norm * 10_000.0).round() / 10_000.0,
                cosine_similarity: (cosine_sim * 10_000.0).round() / 10_000.0,
                hybrid_score: hybrid,
                ethical_filter_status: "passed_mizan_boundary_v1".to_owned(),
            }
        })
        .collect();

    hits.sort_by(|a, b| {
        b.hybrid_score
            .partial_cmp(&a.hybrid_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });
    hits.truncate(top_k);

    Ok(MizanHybridSearchResponse {
        query: trimmed_query.to_owned(),
        top_k,
        bm25_weight: (norm_bm25_w * 10_000.0).round() / 10_000.0,
        vector_weight: (norm_vec_w * 10_000.0).round() / 10_000.0,
        total_evaluated,
        filtered_out_count,
        results: hits,
        advisory_only: true,
        normative_verdicts_allowed: false,
        autonomous_execution_authority: false,
        policy_version: POLICY_VERSION.to_owned(),
        profile_id: PROFILE_ID.to_owned(),
        adr_reference: ADR_REFERENCE.to_owned(),
        disclaimer: "Non-normative hybrid BM25 + Cosine Vector search filtered by Mizan ADR 0005 ethical boundary. Carries zero normative or autonomous execution authority.".to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::CreateNodeRequest;

    #[test]
    fn bm25_and_cosine_hybrid_ranking_scores_matching_documents_highest() {
        let store = NexusGraphStore::open_in_memory().expect("in-memory store");
        let response = execute_mizan_hybrid_search(
            &store,
            &MizanHybridSearchRequest {
                query: "tanzil uthmani quran passage".to_owned(),
                query_vector: Some(vec![1.0, 0.0, 0.0, 0.0]),
                domain: None,
                top_k: Some(5),
                bm25_weight: Some(0.6),
                vector_weight: Some(0.4),
                sensitivity_class: SensitivityClass::Internal,
                involves_minor: false,
                requested_action: None,
                candidates: vec![
                    MizanSearchCandidateInput {
                        id: "cand-exact".to_owned(),
                        domain: ProvenanceDomain::TextRgbl,
                        canonical_label: "Tanzil Uthmani Quran Passage Q7:46".to_owned(),
                        corpus_ref: "mw-corpus:sha256:6ff83b01099fb7992b5f7fa101cfab3d6556607ca6b1fe69d53cb76f97027760".to_owned(),
                        text: "Verified Tanzil Uthmani Quran passage witness".to_owned(),
                        vector: Some(vec![0.95, 0.05, 0.0, 0.0]),
                        uncertainty_state: UncertaintyState::HumanReviewed,
                        sensitivity_class: SensitivityClass::Internal,
                        involves_minor: false,
                    },
                    MizanSearchCandidateInput {
                        id: "cand-partial".to_owned(),
                        domain: ProvenanceDomain::StoryMftl,
                        canonical_label: "Forest Signal Relay".to_owned(),
                        corpus_ref: "mw-corpus:sha256:1111111111111111111111111111111111111111111111111111111111111111".to_owned(),
                        text: "A passage across the electric forest horizon".to_owned(),
                        vector: Some(vec![0.2, 0.8, 0.0, 0.0]),
                        uncertainty_state: UncertaintyState::Candidate,
                        sensitivity_class: SensitivityClass::Internal,
                        involves_minor: false,
                    },
                    MizanSearchCandidateInput {
                        id: "cand-unrelated".to_owned(),
                        domain: ProvenanceDomain::EventLegend,
                        canonical_label: "Solar Telemetry Anomaly".to_owned(),
                        corpus_ref: "mw-corpus:sha256:2222222222222222222222222222222222222222222222222222222222222222".to_owned(),
                        text: "Geomagnetic storm recorded by orbital station".to_owned(),
                        vector: Some(vec![0.0, 0.0, 1.0, 0.0]),
                        uncertainty_state: UncertaintyState::Unknown,
                        sensitivity_class: SensitivityClass::Internal,
                        involves_minor: false,
                    },
                ],
            },
        )
        .expect("hybrid search succeeds");

        assert_eq!(response.results.len(), 3);
        assert_eq!(response.results[0].id, "cand-exact");
        assert_eq!(response.results[0].normalized_bm25_score, 1.0);
        assert!(response.results[0].cosine_similarity > 0.99);
        assert!(response.results[0].hybrid_score > response.results[1].hybrid_score);
        assert!(response.results[1].hybrid_score > response.results[2].hybrid_score);
    }

    #[test]
    fn ethical_filter_strips_violating_candidates_and_rejects_normative_or_child_sensitive_queries() {
        let store = NexusGraphStore::open_in_memory().expect("in-memory store");
        store
            .create_node(CreateNodeRequest {
                id: Some("node-clean".to_owned()),
                domain: ProvenanceDomain::TextRgbl,
                canonical_label: "Clean Witness Archive".to_owned(),
                corpus_ref: "corpus://rgbl/clean".to_owned(),
                uncertainty_state: Some(UncertaintyState::HumanReviewed),
                alias_set: Some(vec!["Archive".to_owned()]),
                sensitivity_class: Some(SensitivityClass::Internal),
                metadata: None,
            })
            .expect("create clean node");

        let response = execute_mizan_hybrid_search(
            &store,
            &MizanHybridSearchRequest {
                query: "witness archive".to_owned(),
                query_vector: None,
                domain: None,
                top_k: Some(10),
                bm25_weight: Some(0.5),
                vector_weight: Some(0.5),
                sensitivity_class: SensitivityClass::Internal,
                involves_minor: false,
                requested_action: None,
                candidates: vec![
                    MizanSearchCandidateInput {
                        id: "c-valid".to_owned(),
                        domain: ProvenanceDomain::TextRgbl,
                        canonical_label: "Valid Witness Archive".to_owned(),
                        corpus_ref: "corpus://valid".to_owned(),
                        text: "Verified witness archive record".to_owned(),
                        vector: None,
                        uncertainty_state: UncertaintyState::HumanReviewed,
                        sensitivity_class: SensitivityClass::Internal,
                        involves_minor: false,
                    },
                    MizanSearchCandidateInput {
                        id: "c-child-blocked".to_owned(),
                        domain: ProvenanceDomain::PersonSuperhero,
                        canonical_label: "Minor Witness Record".to_owned(),
                        corpus_ref: "corpus://child".to_owned(),
                        text: "Witness archive minor record".to_owned(),
                        vector: None,
                        uncertainty_state: UncertaintyState::Unknown,
                        sensitivity_class: SensitivityClass::ChildSensitive,
                        involves_minor: true,
                    },
                    MizanSearchCandidateInput {
                        id: "c-normative-blocked".to_owned(),
                        domain: ProvenanceDomain::LawAws,
                        canonical_label: "Unlawful Verdict Record".to_owned(),
                        corpus_ref: "corpus://normative".to_owned(),
                        text: "Witness archive declaring divine judgment and salvation status".to_owned(),
                        vector: None,
                        uncertainty_state: UncertaintyState::Unknown,
                        sensitivity_class: SensitivityClass::Internal,
                        involves_minor: false,
                    },
                ],
            },
        )
        .expect("search filters violating candidates");

        assert_eq!(response.total_evaluated, 3);
        assert_eq!(response.filtered_out_count, 2);
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].id, "c-valid");

        // Normative query must fail closed
        let bad_query = execute_mizan_hybrid_search(
            &store,
            &MizanHybridSearchRequest {
                query: "compute sin score and divine verdict".to_owned(),
                query_vector: None,
                domain: None,
                top_k: None,
                bm25_weight: None,
                vector_weight: None,
                sensitivity_class: SensitivityClass::Internal,
                involves_minor: false,
                requested_action: None,
                candidates: vec![],
            },
        );
        assert!(matches!(
            bad_query,
            Err(MizanRejection::NormativeBoundaryViolation { .. })
        ));

        // Child-sensitive query must fail closed
        let child_query = execute_mizan_hybrid_search(
            &store,
            &MizanHybridSearchRequest {
                query: "witness archive".to_owned(),
                query_vector: None,
                domain: None,
                top_k: None,
                bm25_weight: None,
                vector_weight: None,
                sensitivity_class: SensitivityClass::ChildSensitive,
                involves_minor: true,
                requested_action: None,
                candidates: vec![],
            },
        );
        assert_eq!(child_query.unwrap_err(), MizanRejection::ChildSensitiveBlocked);
    }
}

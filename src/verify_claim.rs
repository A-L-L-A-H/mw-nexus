use crate::domains::{ProvenanceDomain, ReviewState, UncertaintyState};
use crate::graph::{CorrelationListFilter, GraphError, NexusGraphStore, NodeListFilter};
use crate::mizan::{
    ADR_REFERENCE, EvidenceFactorInput, EvidenceReviewState, EvidenceStance, MizanDiagnostic,
    MizanEvaluationRequest, MizanRejection, MizanStatus, POLICY_VERSION, PROFILE_ID,
    SensitivityClass, evaluate_mizan,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyClaimRequest {
    pub claim_id: Option<String>,
    pub claim_text: String,
    #[serde(default)]
    pub subject_id: Option<String>,
    #[serde(default)]
    pub domain: Option<ProvenanceDomain>,
    #[serde(default)]
    pub corpus_refs: Vec<String>,
    #[serde(default)]
    pub node_ids: Vec<String>,
    #[serde(default)]
    pub sensitivity_class: SensitivityClass,
    #[serde(default)]
    pub involves_minor: bool,
    #[serde(default)]
    pub evidence_inputs: Vec<EvidenceFactorInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyClaimResponse {
    pub claim_id: String,
    pub claim_sha256: String,
    pub status: MizanStatus,
    pub verified_corpus_refs: Vec<String>,
    pub unresolved_corpus_refs: Vec<String>,
    pub matched_node_ids: Vec<String>,
    pub correlation_count: usize,
    pub mizan_diagnostic: MizanDiagnostic,
    pub non_normative_notice: String,
    pub verified_at: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MizanScoreQuery {
    pub subject_id: Option<String>,
    pub node_id: Option<String>,
    pub domain: Option<ProvenanceDomain>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MizanScoreSummaryResponse {
    pub subject_id: String,
    pub status: MizanStatus,
    pub advisory_only: bool,
    pub normative_verdicts_allowed: bool,
    pub autonomous_execution_authority: bool,
    pub human_review_required: bool,
    pub policy_version: String,
    pub profile_id: String,
    pub adr_reference: String,
    pub advisory_score: Option<f64>,
    pub evaluated_node_count: usize,
    pub human_reviewed_count: usize,
    pub candidate_or_unknown_count: usize,
    pub conflicting_count: usize,
    pub diagnostic: MizanDiagnostic,
}

pub fn verify_claim(
    store: &NexusGraphStore,
    request: &VerifyClaimRequest,
) -> Result<VerifyClaimResponse, MizanRejection> {
    if request.sensitivity_class == SensitivityClass::ChildSensitive || request.involves_minor {
        return Err(MizanRejection::ChildSensitiveBlocked);
    }

    let trimmed_claim = request.claim_text.trim();
    if trimmed_claim.is_empty() {
        return Err(MizanRejection::InvalidInput {
            reason: "claim_text must not be empty".to_owned(),
        });
    }

    let mut hasher = Sha256::new();
    hasher.update(trimmed_claim.as_bytes());
    let claim_sha256 = hex::encode(hasher.finalize());
    let claim_id = request
        .claim_id
        .clone()
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| format!("claim-{}", &claim_sha256[..16]));
    let subject_id = request
        .subject_id
        .clone()
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| claim_id.clone());

    let all_nodes = store
        .list_nodes(NodeListFilter {
            domain: request.domain,
            uncertainty_state: None,
            limit: Some(500),
        })
        .map_err(|err| MizanRejection::InvalidInput {
            reason: format!("unable to query ontology nodes: {err}"),
        })?;

    let mut verified_corpus_refs = Vec::new();
    let mut unresolved_corpus_refs = Vec::new();
    let mut matched_node_ids = Vec::new();
    let mut synthesized_evidence: Vec<EvidenceFactorInput> = request.evidence_inputs.clone();

    for requested_node_id in &request.node_ids {
        match store.get_node(requested_node_id) {
            Ok(node) => {
                if !matched_node_ids.contains(&node.id) {
                    matched_node_ids.push(node.id.clone());
                }
                if !verified_corpus_refs.contains(&node.corpus_ref) {
                    verified_corpus_refs.push(node.corpus_ref.clone());
                }
                synthesized_evidence.push(node_to_evidence_factor(&node));
            }
            Err(GraphError::ChildSensitiveBlocked) => {
                return Err(MizanRejection::ChildSensitiveBlocked);
            }
            Err(_) => {
                synthesized_evidence.push(EvidenceFactorInput {
                    evidence_id: format!("missing-node-{requested_node_id}"),
                    corpus_ref: None,
                    domain: request.domain.unwrap_or(ProvenanceDomain::TextRgbl),
                    review_state: EvidenceReviewState::Missing,
                    stance: EvidenceStance::Indeterminate,
                    signal_strength: None,
                    sensitivity_class: SensitivityClass::Internal,
                    summary: format!("Referenced node '{requested_node_id}' not found in ontology store"),
                });
            }
        }
    }

    for requested_ref in &request.corpus_refs {
        let trimmed_ref = requested_ref.trim();
        if trimmed_ref.is_empty() {
            continue;
        }
        if let Some(node) = all_nodes.iter().find(|n| n.corpus_ref == trimmed_ref) {
            if !verified_corpus_refs.contains(&node.corpus_ref) {
                verified_corpus_refs.push(node.corpus_ref.clone());
            }
            if !matched_node_ids.contains(&node.id) {
                matched_node_ids.push(node.id.clone());
                synthesized_evidence.push(node_to_evidence_factor(node));
            }
        } else {
            unresolved_corpus_refs.push(trimmed_ref.to_owned());
            synthesized_evidence.push(EvidenceFactorInput {
                evidence_id: format!("unresolved-ref-{}", verified_corpus_refs.len() + unresolved_corpus_refs.len()),
                corpus_ref: Some(trimmed_ref.to_owned()),
                domain: request.domain.unwrap_or(ProvenanceDomain::TextRgbl),
                review_state: EvidenceReviewState::Missing,
                stance: EvidenceStance::Indeterminate,
                signal_strength: None,
                sensitivity_class: SensitivityClass::Internal,
                summary: format!("Corpus reference '{trimmed_ref}' is not yet anchored in ontology store"),
            });
        }
    }

    let mut correlation_count = 0usize;
    for node_id in &matched_node_ids {
        if let Ok(edges) = store.list_correlations(CorrelationListFilter {
            node_id: Some(node_id.clone()),
            relation_kind: None,
            review_state: None,
            limit: Some(100),
        }) {
            correlation_count += edges.len();
        }
    }

    let eval_request = MizanEvaluationRequest {
        subject_id,
        subject_label: claim_id.clone(),
        scope_description: format!("Epistemic claim provenance verification: {trimmed_claim}"),
        sensitivity_class: request.sensitivity_class,
        involves_minor: request.involves_minor,
        requested_claim: Some(trimmed_claim.to_owned()),
        requested_action: None,
        factor_schema: None,
        evidence_inputs: synthesized_evidence,
    };

    let diagnostic = evaluate_mizan(&eval_request)?;

    Ok(VerifyClaimResponse {
        claim_id,
        claim_sha256,
        status: diagnostic.status,
        verified_corpus_refs,
        unresolved_corpus_refs,
        matched_node_ids,
        correlation_count,
        mizan_diagnostic: diagnostic,
        non_normative_notice: "Advisory provenance verification only (ADR-0005). Correlation does not establish causation, identity equivalence, or normative/divine/legal verdicts.".to_owned(),
        verified_at: Utc::now().to_rfc3339(),
    })
}

pub fn compute_mizan_score_summary(
    store: &NexusGraphStore,
    query: &MizanScoreQuery,
) -> Result<MizanScoreSummaryResponse, MizanRejection> {
    let subject_id = query
        .subject_id
        .clone()
        .or_else(|| query.node_id.clone())
        .unwrap_or_else(|| "nexus-global-mizan".to_owned());

    let mut target_nodes = Vec::new();
    if let Some(ref specific_node_id) = query.node_id {
        match store.get_node(specific_node_id) {
            Ok(node) => target_nodes.push(node),
            Err(GraphError::ChildSensitiveBlocked) => {
                return Err(MizanRejection::ChildSensitiveBlocked);
            }
            Err(GraphError::NodeNotFound { .. }) => {}
            Err(err) => {
                return Err(MizanRejection::InvalidInput {
                    reason: err.to_string(),
                });
            }
        }
    } else {
        target_nodes = store
            .list_nodes(NodeListFilter {
                domain: query.domain,
                uncertainty_state: None,
                limit: Some(200),
            })
            .map_err(|err| MizanRejection::InvalidInput {
                reason: err.to_string(),
            })?;
    }

    let mut human_reviewed_count = 0usize;
    let mut candidate_or_unknown_count = 0usize;
    let mut conflicting_count = 0usize;
    let mut evidence_inputs = Vec::new();

    for node in &target_nodes {
        match node.uncertainty_state {
            UncertaintyState::HumanReviewed => human_reviewed_count += 1,
            UncertaintyState::Conflicting => conflicting_count += 1,
            UncertaintyState::Unknown
            | UncertaintyState::Candidate
            | UncertaintyState::NeedsSource => candidate_or_unknown_count += 1,
        }
        evidence_inputs.push(node_to_evidence_factor(node));
    }

    if let Some(ref specific_node_id) = query.node_id
        && let Ok(edges) = store.list_correlations(CorrelationListFilter {
            node_id: Some(specific_node_id.clone()),
            relation_kind: None,
            review_state: None,
            limit: Some(100),
        })
    {
        for edge in edges {
            let (review_state, stance, signal) = match edge.review_state {
                ReviewState::HumanReviewed => (
                    EvidenceReviewState::VerifiedHumanReviewed,
                    EvidenceStance::Supporting,
                    Some(1.0),
                ),
                ReviewState::Conflicting => (
                    EvidenceReviewState::Conflicting,
                    EvidenceStance::Counter,
                    None,
                ),
                ReviewState::NeedsSource | ReviewState::Rejected => (
                    EvidenceReviewState::Missing,
                    EvidenceStance::Indeterminate,
                    None,
                ),
                ReviewState::Candidate => (
                    EvidenceReviewState::Unreviewed,
                    EvidenceStance::Neutral,
                    None,
                ),
            };
            evidence_inputs.push(EvidenceFactorInput {
                evidence_id: format!("edge-{}", edge.id),
                corpus_ref: Some(edge.evidence_hash),
                domain: edge.source_domain,
                review_state,
                stance,
                signal_strength: signal,
                sensitivity_class: SensitivityClass::Internal,
                summary: format!("Correlation edge {}", edge.evidence_locator),
            });
        }
    }

    let diagnostic = evaluate_mizan(&MizanEvaluationRequest {
        subject_id: subject_id.clone(),
        subject_label: subject_id.clone(),
        scope_description: "Non-normative Mizan provenance diagnostic summary".to_owned(),
        sensitivity_class: SensitivityClass::Internal,
        involves_minor: false,
        requested_claim: None,
        requested_action: None,
        factor_schema: None,
        evidence_inputs,
    })?;

    Ok(MizanScoreSummaryResponse {
        subject_id,
        status: diagnostic.status,
        advisory_only: diagnostic.advisory_only,
        normative_verdicts_allowed: diagnostic.normative_verdicts_allowed,
        autonomous_execution_authority: diagnostic.autonomous_execution_authority,
        human_review_required: diagnostic.human_review_required,
        policy_version: POLICY_VERSION.to_owned(),
        profile_id: PROFILE_ID.to_owned(),
        adr_reference: ADR_REFERENCE.to_owned(),
        advisory_score: diagnostic.advisory_score,
        evaluated_node_count: target_nodes.len(),
        human_reviewed_count,
        candidate_or_unknown_count,
        conflicting_count,
        diagnostic,
    })
}

fn node_to_evidence_factor(node: & crate::graph::OntologyNode) -> EvidenceFactorInput {
    let (review_state, stance, signal_strength) = match node.uncertainty_state {
        UncertaintyState::HumanReviewed => (
            EvidenceReviewState::VerifiedHumanReviewed,
            EvidenceStance::Supporting,
            Some(1.0),
        ),
        UncertaintyState::Conflicting => (
            EvidenceReviewState::Conflicting,
            EvidenceStance::Counter,
            None,
        ),
        UncertaintyState::NeedsSource => (
            EvidenceReviewState::Missing,
            EvidenceStance::Indeterminate,
            None,
        ),
        UncertaintyState::Unknown | UncertaintyState::Candidate => (
            EvidenceReviewState::Unreviewed,
            EvidenceStance::Indeterminate,
            None,
        ),
    };

    EvidenceFactorInput {
        evidence_id: format!("node-{}", node.id),
        corpus_ref: Some(node.corpus_ref.clone()),
        domain: node.domain,
        review_state,
        stance,
        signal_strength,
        sensitivity_class: node.sensitivity_class,
        summary: format!("Ontology node {}", node.canonical_label),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::CreateNodeRequest;

    #[test]
    fn verify_claim_validates_anchored_corpus_ref_and_enforces_adr0005() {
        let store = NexusGraphStore::open_in_memory().expect("in-memory store");
        let node = store
            .create_node(CreateNodeRequest {
                id: Some("node-q7-46".to_owned()),
                domain: ProvenanceDomain::TextRgbl,
                canonical_label: "Q7:46 Tanzil".to_owned(),
                corpus_ref: "mw-corpus:sha256:6ff83b01099fb7992b5f7fa101cfab3d6556607ca6b1fe69d53cb76f97027760".to_owned(),
                uncertainty_state: Some(UncertaintyState::HumanReviewed),
                alias_set: Some(vec!["Q7:46".to_owned()]),
                sensitivity_class: Some(SensitivityClass::Internal),
                metadata: None,
            })
            .expect("create node");

        let response = verify_claim(
            &store,
            &VerifyClaimRequest {
                claim_id: Some("claim-test-1".to_owned()),
                claim_text: "Passage Q7:46 is anchored in Tanzil Uthmani edition.".to_owned(),
                subject_id: Some("subj-q7-46".to_owned()),
                domain: Some(ProvenanceDomain::TextRgbl),
                corpus_refs: vec![node.corpus_ref.clone()],
                node_ids: vec![],
                sensitivity_class: SensitivityClass::Internal,
                involves_minor: false,
                evidence_inputs: vec![],
            },
        )
        .expect("verify claim succeeds");

        assert_eq!(response.status, MizanStatus::AdvisoryNonNormative);
        assert_eq!(response.verified_corpus_refs, vec![node.corpus_ref]);
        assert!(response.unresolved_corpus_refs.is_empty());
        assert_eq!(response.mizan_diagnostic.advisory_score, Some(1.0));

        let score_summary = compute_mizan_score_summary(
            &store,
            &MizanScoreQuery {
                subject_id: Some("subj-q7-46".to_owned()),
                node_id: Some("node-q7-46".to_owned()),
                domain: None,
            },
        )
        .expect("mizan score summary");
        assert_eq!(score_summary.status, MizanStatus::AdvisoryNonNormative);
        assert_eq!(score_summary.advisory_score, Some(1.0));
        assert_eq!(score_summary.human_reviewed_count, 1);

        let rejected = verify_claim(
            &store,
            &VerifyClaimRequest {
                claim_id: None,
                claim_text: "Determine divine judgment and salvation status of subject.".to_owned(),
                subject_id: None,
                domain: None,
                corpus_refs: vec![],
                node_ids: vec![],
                sensitivity_class: SensitivityClass::Internal,
                involves_minor: false,
                evidence_inputs: vec![],
            },
        );
        assert!(matches!(
            rejected,
            Err(MizanRejection::NormativeBoundaryViolation { .. })
        ));
    }
}

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use mw_nexus::api::{AppState, build_router};
use mw_nexus::crayon::{
    CRAYON_PROFILE_ID, RGBL_PROFILE_ID, assert_strict_profile_separation, crayon_profile_v1_3,
    omniscience_rgbl_profile_v1_0, reject_profile_merge,
};
use mw_nexus::domains::{
    BoundaryViolation, ProvenanceDomain, RelationKind, ReviewState, UncertaintyState,
    canonical_domains, validate_cross_domain_boundary,
};
use mw_nexus::graph::{
    CreateCorrelationRequest, CreateNodeRequest, GraphError, NexusGraphStore, SubgraphQuery,
    compute_evidence_hash,
};
use mw_nexus::mizan::{
    EvidenceFactorInput, EvidenceReviewState, EvidenceStance, MizanEvaluationRequest,
    MizanRejection, MizanStatus, POLICY_VERSION, SensitivityClass, evaluate_mizan,
};
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

async fn send_json_request(
    router: &axum::Router,
    method: Method,
    uri: &str,
    payload: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    let body = match payload {
        Some(json_payload) => {
            builder = builder.header("content-type", "application/json");
            Body::from(json_payload.to_string())
        }
        None => Body::empty(),
    };
    let request = builder.body(body).expect("valid HTTP request");
    let response = router
        .clone()
        .oneshot(request)
        .await
        .expect("router oneshot succeeds");
    let status = response.status();
    let collected = response
        .into_body()
        .collect()
        .await
        .expect("response body readable")
        .to_bytes();
    let parsed: Value = serde_json::from_slice(&collected).expect("valid JSON response body");
    (status, parsed)
}

#[test]
fn all_seven_canonical_provenance_domains_are_present_and_distinct() {
    let domains = canonical_domains();
    assert_eq!(domains.len(), 7);

    let codes: Vec<&str> = domains
        .iter()
        .map(|metadata| metadata.code.as_str())
        .collect();
    assert_eq!(
        codes,
        vec![
            "TEXT_RGBL",
            "STORY_MFTL",
            "EVENT_LEGEND",
            "PERSON_SUPERHERO",
            "LAW_AWS",
            "PERSPECTIVE_JIZZ",
            "RELATIONSHIP_CORRELATION",
        ]
    );

    let law_meta = domains
        .iter()
        .find(|metadata| metadata.domain == ProvenanceDomain::LawAws)
        .expect("LAW_AWS domain must exist");
    assert!(law_meta.semantic_label.contains("Angel With Shotgun"));
    assert!(law_meta.alias_note.contains("LAW/Jurisdiction"));
    assert!(law_meta.alias_note.contains("NOT Amazon Web Services"));

    let correlation_meta = domains
        .iter()
        .find(|metadata| metadata.domain == ProvenanceDomain::RelationshipCorrelation)
        .expect("RELATIONSHIP_CORRELATION domain must exist");
    assert!(correlation_meta.boundary_rule.contains("NOT causation"));
    assert!(correlation_meta.boundary_rule.contains("NOT identity"));
}

#[test]
fn cross_domain_boundary_validator_rejects_causation_identity_fulfillment_and_ownership_conflation() {
    let digest = compute_evidence_hash("witness-ledger-evidence-01");

    let causation_err = validate_cross_domain_boundary(
        "node-a",
        "node-b",
        ProvenanceDomain::EventLegend,
        ProvenanceDomain::EventLegend,
        "causes",
        &digest,
    )
    .expect_err("causation must be rejected");
    assert!(matches!(
        causation_err,
        BoundaryViolation::CorrelationNotCausation { .. }
    ));

    let identity_err = validate_cross_domain_boundary(
        "node-person-1",
        "node-person-2",
        ProvenanceDomain::PersonSuperhero,
        ProvenanceDomain::PersonSuperhero,
        "same_as",
        &digest,
    )
    .expect_err("identity equivalence must be rejected");
    assert!(matches!(
        identity_err,
        BoundaryViolation::CorrelationNotIdentity { .. }
    ));

    let auto_merge_err = validate_cross_domain_boundary(
        "node-person-1",
        "node-person-2",
        ProvenanceDomain::PersonSuperhero,
        ProvenanceDomain::PersonSuperhero,
        "auto_merge_alias",
        &digest,
    )
    .expect_err("alias auto-merge relation must be rejected");
    assert!(matches!(
        auto_merge_err,
        BoundaryViolation::CorrelationNotIdentity { .. }
    ));

    let fulfillment_err = validate_cross_domain_boundary(
        "node-text-1",
        "node-event-1",
        ProvenanceDomain::TextRgbl,
        ProvenanceDomain::EventLegend,
        "fulfills_prophecy",
        &digest,
    )
    .expect_err("prophetic fulfillment must be rejected");
    assert!(matches!(
        fulfillment_err,
        BoundaryViolation::CorrelationNotFulfillment { .. }
    ));

    let ownership_err = validate_cross_domain_boundary(
        "node-persp-1",
        "node-text-1",
        ProvenanceDomain::PerspectiveJizz,
        ProvenanceDomain::TextRgbl,
        "owns_source",
        &digest,
    )
    .expect_err("perspective owning underlying source must be rejected");
    assert!(matches!(
        ownership_err,
        BoundaryViolation::CrossDomainOwnershipViolation { .. }
    ));

    let wrong_target_err = validate_cross_domain_boundary(
        "node-story-1",
        "node-event-1",
        ProvenanceDomain::StoryMftl,
        ProvenanceDomain::EventLegend,
        "governed_by_jurisdiction",
        &digest,
    )
    .expect_err("governed_by_jurisdiction must target LAW_AWS");
    assert!(matches!(
        wrong_target_err,
        BoundaryViolation::CrossDomainOwnershipViolation { .. }
    ));

    let cross_domain_alias_err = validate_cross_domain_boundary(
        "node-story-1",
        "node-event-1",
        ProvenanceDomain::StoryMftl,
        ProvenanceDomain::EventLegend,
        "unresolved_alias_of",
        &digest,
    )
    .expect_err("cross-domain alias link must be rejected");
    assert!(matches!(
        cross_domain_alias_err,
        BoundaryViolation::CrossDomainOwnershipViolation { .. }
    ));

    let self_loop_err = validate_cross_domain_boundary(
        "node-same",
        "node-same",
        ProvenanceDomain::TextRgbl,
        ProvenanceDomain::TextRgbl,
        "correlated_with",
        &digest,
    )
    .expect_err("self-loop must be rejected");
    assert!(matches!(
        self_loop_err,
        BoundaryViolation::SelfLoopProhibited { .. }
    ));

    let bad_hash_err = validate_cross_domain_boundary(
        "node-a",
        "node-b",
        ProvenanceDomain::StoryMftl,
        ProvenanceDomain::TextRgbl,
        "references_passage",
        "not-hex!",
    )
    .expect_err("invalid evidence hash must be rejected");
    assert!(matches!(
        bad_hash_err,
        BoundaryViolation::InvalidEvidenceHash { .. }
    ));

    let valid_boundary = validate_cross_domain_boundary(
        "node-person-1",
        "node-event-1",
        ProvenanceDomain::PersonSuperhero,
        ProvenanceDomain::EventLegend,
        "witnessed_event",
        &digest,
    )
    .expect("valid PERSON_SUPERHERO -> EVENT_LEGEND edge must pass");
    assert_eq!(valid_boundary.relation_kind, RelationKind::WitnessedEvent);
    assert!(!valid_boundary.implies_causation);
    assert!(!valid_boundary.implies_identity);
    assert!(!valid_boundary.implies_fulfillment);
    assert!(valid_boundary.alias_auto_merge_prohibited);
}

#[test]
fn mizan_preserves_unknown_for_missing_unreviewed_and_conflicting_evidence() {
    // CAL-01: Empty evidence -> insufficient_evidence, never coerced to 0 or safe.
    let empty_request = MizanEvaluationRequest {
        subject_id: "case-cal-01".to_owned(),
        subject_label: "Synthetic evidentiary completeness check".to_owned(),
        scope_description: "Verify missing evidence remains insufficient_evidence".to_owned(),
        sensitivity_class: SensitivityClass::Internal,
        involves_minor: false,
        requested_claim: None,
        requested_action: None,
        factor_schema: None,
        evidence_inputs: Vec::new(),
    };
    let empty_outcome = evaluate_mizan(&empty_request).expect("empty evidence yields diagnostic");
    assert_eq!(empty_outcome.status, MizanStatus::InsufficientEvidence);
    assert_eq!(empty_outcome.advisory_score, None);
    assert!(empty_outcome.advisory_only);
    assert!(!empty_outcome.normative_verdicts_allowed);
    assert!(!empty_outcome.autonomous_execution_authority);
    assert_eq!(empty_outcome.policy_version, POLICY_VERSION);

    // Missing signal_strength -> insufficient_evidence, never zero-filled.
    let null_signal_request = MizanEvaluationRequest {
        subject_id: "case-cal-01b".to_owned(),
        subject_label: "Evidence with missing quantitative signal".to_owned(),
        scope_description: "Verify absent signal_strength is never defaulted to zero".to_owned(),
        sensitivity_class: SensitivityClass::Internal,
        involves_minor: false,
        requested_claim: None,
        requested_action: None,
        factor_schema: None,
        evidence_inputs: vec![EvidenceFactorInput {
            evidence_id: "ev-missing-signal".to_owned(),
            corpus_ref: Some("corpus://witness/001".to_owned()),
            domain: ProvenanceDomain::EventLegend,
            review_state: EvidenceReviewState::VerifiedHumanReviewed,
            stance: EvidenceStance::Supporting,
            signal_strength: None,
            sensitivity_class: SensitivityClass::Internal,
            summary: "Observation logged without measured signal".to_owned(),
        }],
    };
    let null_signal_outcome = evaluate_mizan(&null_signal_request).expect("valid diagnostic");
    assert_eq!(null_signal_outcome.status, MizanStatus::InsufficientEvidence);
    assert_eq!(null_signal_outcome.advisory_score, None);

    // Unreviewed evidence -> unknown.
    let unreviewed_request = MizanEvaluationRequest {
        subject_id: "case-unreviewed".to_owned(),
        subject_label: "Unreviewed source record".to_owned(),
        scope_description: "Check unknown propagation for unreviewed evidence".to_owned(),
        sensitivity_class: SensitivityClass::Internal,
        involves_minor: false,
        requested_claim: None,
        requested_action: None,
        factor_schema: None,
        evidence_inputs: vec![EvidenceFactorInput {
            evidence_id: "ev-unreviewed-1".to_owned(),
            corpus_ref: Some("corpus://witness/002".to_owned()),
            domain: ProvenanceDomain::TextRgbl,
            review_state: EvidenceReviewState::Unreviewed,
            stance: EvidenceStance::Supporting,
            signal_strength: Some(0.95),
            sensitivity_class: SensitivityClass::Internal,
            summary: "Pending independent human reviewer sign-off".to_owned(),
        }],
    };
    let unreviewed_outcome = evaluate_mizan(&unreviewed_request).expect("valid diagnostic");
    assert_eq!(unreviewed_outcome.status, MizanStatus::Unknown);
    assert_eq!(unreviewed_outcome.advisory_score, None);

    // CAL-02: Opposing verified evidence -> unknown with conflict explicitly preserved.
    let conflict_request = MizanEvaluationRequest {
        subject_id: "case-cal-02".to_owned(),
        subject_label: "Conflicting witness timelines".to_owned(),
        scope_description: "Verify opposing evidence is never averaged into certainty".to_owned(),
        sensitivity_class: SensitivityClass::Internal,
        involves_minor: false,
        requested_claim: None,
        requested_action: None,
        factor_schema: None,
        evidence_inputs: vec![
            EvidenceFactorInput {
                evidence_id: "ev-support-1".to_owned(),
                corpus_ref: Some("corpus://event/a".to_owned()),
                domain: ProvenanceDomain::EventLegend,
                review_state: EvidenceReviewState::VerifiedHumanReviewed,
                stance: EvidenceStance::Supporting,
                signal_strength: Some(0.85),
                sensitivity_class: SensitivityClass::Internal,
                summary: "Archive log corroborates timestamp A".to_owned(),
            },
            EvidenceFactorInput {
                evidence_id: "ev-counter-1".to_owned(),
                corpus_ref: Some("corpus://event/b".to_owned()),
                domain: ProvenanceDomain::EventLegend,
                review_state: EvidenceReviewState::VerifiedHumanReviewed,
                stance: EvidenceStance::Counter,
                signal_strength: Some(0.80),
                sensitivity_class: SensitivityClass::Internal,
                summary: "Independent witness report contradicts timestamp A".to_owned(),
            },
        ],
    };
    let conflict_outcome = evaluate_mizan(&conflict_request).expect("valid diagnostic");
    assert_eq!(conflict_outcome.status, MizanStatus::Unknown);
    assert!(conflict_outcome.conflict_preserved);
    assert_eq!(conflict_outcome.advisory_score, None);
    assert_eq!(conflict_outcome.conflicting_evidence_ids.len(), 2);

    // CAL-03: Reviewed, non-conflicting evidence -> bounded advisory diagnostic.
    let reviewed_request = MizanEvaluationRequest {
        subject_id: "case-cal-03".to_owned(),
        subject_label: "Synthetic corroborated provenance record".to_owned(),
        scope_description: "Advisory coverage diagnostic for reviewed archive citation".to_owned(),
        sensitivity_class: SensitivityClass::Internal,
        involves_minor: false,
        requested_claim: Some("source citation coverage diagnostic".to_owned()),
        requested_action: None,
        factor_schema: Some("mizan-boundary-v1".to_owned()),
        evidence_inputs: vec![
            EvidenceFactorInput {
                evidence_id: "ev-verified-1".to_owned(),
                corpus_ref: Some("corpus://text/01".to_owned()),
                domain: ProvenanceDomain::TextRgbl,
                review_state: EvidenceReviewState::VerifiedHumanReviewed,
                stance: EvidenceStance::Supporting,
                signal_strength: Some(0.75),
                sensitivity_class: SensitivityClass::Internal,
                summary: "Exact edition locator verified by independent reviewer".to_owned(),
            },
            EvidenceFactorInput {
                evidence_id: "ev-verified-2".to_owned(),
                corpus_ref: Some("corpus://law/02".to_owned()),
                domain: ProvenanceDomain::LawAws,
                review_state: EvidenceReviewState::VerifiedHumanReviewed,
                stance: EvidenceStance::Supporting,
                signal_strength: Some(0.85),
                sensitivity_class: SensitivityClass::Internal,
                summary: "Jurisdiction scope note reviewed and recorded".to_owned(),
            },
        ],
    };
    let reviewed_outcome = evaluate_mizan(&reviewed_request).expect("valid advisory diagnostic");
    assert_eq!(reviewed_outcome.status, MizanStatus::AdvisoryNonNormative);
    assert_eq!(reviewed_outcome.advisory_score, Some(0.8));
    assert!(reviewed_outcome.advisory_only);
    assert!(!reviewed_outcome.normative_verdicts_allowed);
    assert!(!reviewed_outcome.autonomous_execution_authority);
    assert!(reviewed_outcome.human_review_required);
    assert_eq!(reviewed_outcome.policy_version, "mizan-boundary-v1");
}

#[test]
fn mizan_rejects_normative_verdicts_autonomous_actions_legacy_factors_and_child_sensitive() {
    // F3: Normative / divine / legal verdict wording rejected fail-closed.
    for forbidden_claim in [
        "determine salvation_status of actor",
        "emit divine judgment on persona",
        "issue legal verdict declaring subject guilty",
        "compute human worth ranking",
    ] {
        let normative_req = MizanEvaluationRequest {
            subject_id: "case-normative".to_owned(),
            subject_label: "Boundary test".to_owned(),
            scope_description: "Testing normative boundary refusal".to_owned(),
            sensitivity_class: SensitivityClass::Internal,
            involves_minor: false,
            requested_claim: Some(forbidden_claim.to_owned()),
            requested_action: None,
            factor_schema: None,
            evidence_inputs: Vec::new(),
        };
        let rejection = evaluate_mizan(&normative_req)
            .expect_err("normative/divine/legal wording must be rejected");
        assert!(matches!(
            rejection,
            MizanRejection::NormativeBoundaryViolation { .. }
        ));
    }

    // F4: Autonomous publication or harm rejected fail-closed.
    let auto_pub_req = MizanEvaluationRequest {
        subject_id: "case-auto-pub".to_owned(),
        subject_label: "Publication trigger test".to_owned(),
        scope_description: "Testing autonomous publication refusal".to_owned(),
        sensitivity_class: SensitivityClass::Internal,
        involves_minor: false,
        requested_claim: None,
        requested_action: Some("promote_public_approved".to_owned()),
        factor_schema: None,
        evidence_inputs: Vec::new(),
    };
    let action_rejection =
        evaluate_mizan(&auto_pub_req).expect_err("autonomous publication must be rejected");
    assert!(matches!(
        action_rejection,
        MizanRejection::AutonomousActionProhibited { .. }
    ));

    // Legacy dev-mw-blueprint TH/ARI schema rejected.
    let legacy_req = MizanEvaluationRequest {
        subject_id: "case-legacy".to_owned(),
        subject_label: "Legacy TH/ARI check".to_owned(),
        scope_description: "Verify legacy blueprint factor rejection".to_owned(),
        sensitivity_class: SensitivityClass::Internal,
        involves_minor: false,
        requested_claim: None,
        requested_action: None,
        factor_schema: Some("dev-mw-blueprint/legacy_th".to_owned()),
        evidence_inputs: Vec::new(),
    };
    let legacy_rejection =
        evaluate_mizan(&legacy_req).expect_err("legacy TH/ARI schema must be rejected");
    assert!(matches!(
        legacy_rejection,
        MizanRejection::LegacyBlueprintFactorRejected { .. }
    ));

    // FM-04: CHILD_SENSITIVE blocked fail-closed (top-level and nested evidence input).
    let child_req = MizanEvaluationRequest {
        subject_id: "minor-case-013".to_owned(),
        subject_label: "Protected safeguarding entry".to_owned(),
        scope_description: "Must fail closed without leaking metadata".to_owned(),
        sensitivity_class: SensitivityClass::ChildSensitive,
        involves_minor: false,
        requested_claim: None,
        requested_action: None,
        factor_schema: None,
        evidence_inputs: Vec::new(),
    };
    assert_eq!(
        evaluate_mizan(&child_req).expect_err("CHILD_SENSITIVE must fail closed"),
        MizanRejection::ChildSensitiveBlocked
    );

    let nested_child_req = MizanEvaluationRequest {
        subject_id: "case-nested-child".to_owned(),
        subject_label: "General case referencing protected record".to_owned(),
        scope_description: "Nested CHILD_SENSITIVE must fail closed".to_owned(),
        sensitivity_class: SensitivityClass::Internal,
        involves_minor: false,
        requested_claim: None,
        requested_action: None,
        factor_schema: None,
        evidence_inputs: vec![EvidenceFactorInput {
            evidence_id: "ev-protected-013".to_owned(),
            corpus_ref: None,
            domain: ProvenanceDomain::PersonSuperhero,
            review_state: EvidenceReviewState::VerifiedHumanReviewed,
            stance: EvidenceStance::Neutral,
            signal_strength: Some(0.5),
            sensitivity_class: SensitivityClass::ChildSensitive,
            summary: "Protected minor record".to_owned(),
        }],
    };
    assert_eq!(
        evaluate_mizan(&nested_child_req).expect_err("nested CHILD_SENSITIVE must fail closed"),
        MizanRejection::ChildSensitiveBlocked
    );
}

#[test]
fn crayon_v1_3_and_omniscience_rgbl_remain_strictly_separated() {
    let crayon = crayon_profile_v1_3();
    let rgbl = omniscience_rgbl_profile_v1_0();

    assert_strict_profile_separation(&crayon, &rgbl)
        .expect("canonical profiles must satisfy separation invariant");

    assert_eq!(crayon.version, "1.3");
    assert_eq!(crayon.cumulative_levels.len(), 4);
    assert_eq!(crayon.cumulative_levels[0].code, "R1");
    assert_eq!(crayon.cumulative_levels[0].label, "REALITY");
    assert_eq!(crayon.cumulative_levels[1].code, "G2");
    assert_eq!(crayon.cumulative_levels[1].label, "GOVERNING ORDER");
    assert_eq!(crayon.cumulative_levels[2].code, "B3");
    assert_eq!(crayon.cumulative_levels[2].label, "BEING");
    assert_eq!(crayon.cumulative_levels[3].code, "L4");
    assert_eq!(crayon.cumulative_levels[3].label, "LIGHT");
    assert_eq!(crayon.nine_gates.len(), 9);
    assert_eq!(crayon.thirteen_ways.len(), 13);
    assert!(!crayon.merge_permitted_with_rgbl);

    assert_eq!(rgbl.epistemic_layers.len(), 4);
    assert_eq!(rgbl.epistemic_layers[0].label, "Reality");
    assert_eq!(rgbl.epistemic_layers[1].label, "Guidance");
    assert_eq!(rgbl.epistemic_layers[2].label, "Balance");
    assert_eq!(rgbl.epistemic_layers[3].label, "Light");
    assert!(!rgbl.merge_permitted_with_crayon);

    // Explicitly reject any merge attempt between CRAYON and RGBL.
    reject_profile_merge(CRAYON_PROFILE_ID, RGBL_PROFILE_ID)
        .expect_err("merging CRAYON with OMNISCIENCE RGBL must fail closed");
    reject_profile_merge(RGBL_PROFILE_ID, CRAYON_PROFILE_ID)
        .expect_err("merging OMNISCIENCE RGBL with CRAYON must fail closed");
}

#[test]
fn sqlite_wal_graph_persists_nodes_without_alias_auto_merge_and_enforces_immutable_corpus_ref() {
    let temp_dir = TempDir::new().expect("temp dir created");
    let db_path = temp_dir.path().join("nexus-wal-test.sqlite");
    let store = NexusGraphStore::open(&db_path).expect("WAL store opens");

    assert_eq!(
        store.journal_mode().expect("journal mode readable").to_ascii_lowercase(),
        "wal"
    );

    // Create two PERSON_SUPERHERO nodes with overlapping aliases: MUST NOT auto-merge!
    let node_shadow = store
        .create_node(CreateNodeRequest {
            id: None,
            domain: ProvenanceDomain::PersonSuperhero,
            canonical_label: "THE SHADOW".to_owned(),
            corpus_ref: "corpus://mw-ii/characters/shadow#rev1".to_owned(),
            uncertainty_state: Some(UncertaintyState::Candidate),
            alias_set: Some(vec!["ZILL".to_owned(), "Shadow Witness".to_owned()]),
            sensitivity_class: None,
            metadata: None,
        })
        .expect("first alias node created");

    let node_zill = store
        .create_node(CreateNodeRequest {
            id: None,
            domain: ProvenanceDomain::PersonSuperhero,
            canonical_label: "THE SHADOW".to_owned(),
            corpus_ref: "corpus://mw-xii/characters/zill#rev1".to_owned(),
            uncertainty_state: Some(UncertaintyState::Unknown),
            alias_set: Some(vec!["ZILL".to_owned(), "THE SHADOW".to_owned()]),
            sensitivity_class: None,
            metadata: None,
        })
        .expect("second overlapping-alias node created distinctly");

    assert_ne!(node_shadow.id, node_zill.id);

    // Appending aliases on node_shadow keeps node_zill completely independent.
    let updated_shadow = store
        .append_node_aliases(&node_shadow.id, vec!["MOONWITNESS_SHADOW".to_owned()])
        .expect("alias append succeeds");
    assert_eq!(updated_shadow.alias_set.len(), 3);
    let unchanged_zill = store.get_node(&node_zill.id).expect("sibling node intact");
    assert_eq!(unchanged_zill.alias_set.len(), 2);

    // Immutable corpus_ref enforced by SQLite BEFORE UPDATE trigger.
    let mutation_err = store
        .attempt_mutate_corpus_ref(&node_shadow.id, "corpus://rewritten/unauthorized")
        .expect_err("mutating corpus_ref must fail");
    assert!(matches!(
        mutation_err,
        GraphError::ImmutableCorpusRefViolation { .. }
    ));

    // Persist an explicit unresolved_alias_of edge (without merging nodes).
    let edge_hash = compute_evidence_hash("S01-S27-alias-review-packet");
    let alias_edge = store
        .create_correlation(CreateCorrelationRequest {
            source_node_id: node_shadow.id.clone(),
            target_node_id: node_zill.id.clone(),
            relation_kind: "unresolved_alias_of".to_owned(),
            evidence_hash: edge_hash.clone(),
            evidence_locator: "S01/S27#alias-review".to_owned(),
            review_state: Some(ReviewState::Conflicting),
            uncertainty_note: Some("Preserve both records until human identity decision".to_owned()),
        })
        .expect("typed unresolved_alias_of correlation persisted");
    assert_eq!(alias_edge.relation_kind, RelationKind::UnresolvedAliasOf);
    assert_eq!(alias_edge.evidence_hash, edge_hash);
    assert!(!alias_edge.implies_identity);
    assert!(!alias_edge.implies_causation);

    // Reopen SQLite file from disk to prove durability across restarts.
    drop(store);
    let reopened_store = NexusGraphStore::open(&db_path).expect("store reopens cleanly");
    let subgraph = reopened_store
        .query_subgraph(SubgraphQuery {
            root_node_id: Some(node_shadow.id.clone()),
            depth: Some(2),
            domain: None,
            review_state: None,
            limit: Some(50),
        })
        .expect("subgraph query succeeds after reopen");

    assert_eq!(subgraph.node_count, 2);
    assert_eq!(subgraph.edge_count, 1);
    assert!(!subgraph.alias_auto_merge_enabled);
    assert_eq!(subgraph.edges[0].review_state, ReviewState::Conflicting);
}

#[tokio::test]
async fn axum_http_api_end_to_end_covers_all_endpoints_and_guardrails() {
    let store = NexusGraphStore::open_in_memory().expect("in-memory store initializes");
    let router = build_router(AppState::new(store));

    // 1. GET /health
    let (health_status, health_json) =
        send_json_request(&router, Method::GET, "/health", None).await;
    assert_eq!(health_status, StatusCode::OK);
    assert_eq!(health_json["status"], "ok");
    assert_eq!(health_json["domains_count"], 7);
    assert_eq!(health_json["policy_version"], "mizan-boundary-v1");

    // 2. GET /api/domains
    let (domains_status, domains_json) =
        send_json_request(&router, Method::GET, "/api/domains", None).await;
    assert_eq!(domains_status, StatusCode::OK);
    assert_eq!(domains_json["domain_count"], 7);

    // 3. POST /api/nodes across multiple domains (plus a CHILD_SENSITIVE node hidden from general routes)
    let (text_status, text_node) = send_json_request(
        &router,
        Method::POST,
        "/api/nodes",
        Some(json!({
            "domain": "TEXT_RGBL",
            "canonical_label": "Edition Witness Alpha",
            "corpus_ref": "corpus://rgbl/alpha/p1",
            "uncertainty_state": "human_reviewed",
            "alias_set": ["Witness-A"]
        })),
    )
    .await;
    assert_eq!(text_status, StatusCode::CREATED);
    let text_node_id = text_node["id"].as_str().expect("text node id").to_owned();

    let (law_status, law_node) = send_json_request(
        &router,
        Method::POST,
        "/api/nodes",
        Some(json!({
            "domain": "LAW_AWS",
            "canonical_label": "Jurisdiction Charter 01",
            "corpus_ref": "corpus://aws-law/charter-01",
            "uncertainty_state": "candidate",
            "alias_set": ["AWS-Charter-01"]
        })),
    )
    .await;
    assert_eq!(law_status, StatusCode::CREATED);
    let law_node_id = law_node["id"].as_str().expect("law node id").to_owned();

    let (child_status, child_node) = send_json_request(
        &router,
        Method::POST,
        "/api/nodes",
        Some(json!({
            "domain": "PERSON_SUPERHERO",
            "canonical_label": "Protected Minor Record",
            "corpus_ref": "corpus://safeguarding/slot-013",
            "sensitivity_class": "CHILD_SENSITIVE"
        })),
    )
    .await;
    assert_eq!(child_status, StatusCode::CREATED);
    let child_node_id = child_node["id"].as_str().expect("child node id").to_owned();

    // GET /api/nodes excludes CHILD_SENSITIVE records fail-closed.
    let (list_nodes_status, listed_nodes) =
        send_json_request(&router, Method::GET, "/api/nodes", None).await;
    assert_eq!(list_nodes_status, StatusCode::OK);
    assert_eq!(listed_nodes["count"], 2);
    assert_eq!(listed_nodes["alias_auto_merge_enabled"], false);

    // GET /api/nodes/{child_node_id} returns 403 fail-closed.
    let (child_fetch_status, child_fetch_err) = send_json_request(
        &router,
        Method::GET,
        &format!("/api/nodes/{child_node_id}"),
        None,
    )
    .await;
    assert_eq!(child_fetch_status, StatusCode::FORBIDDEN);
    assert_eq!(child_fetch_err["error_code"], "CHILD_SENSITIVE_BLOCKED");

    // 4. POST /api/correlations: reject causation (422), accept typed correlation (201)
    let evidence_digest = compute_evidence_hash("law-references-text-01");
    let (bad_corr_status, bad_corr_json) = send_json_request(
        &router,
        Method::POST,
        "/api/correlations",
        Some(json!({
            "source_node_id": law_node_id,
            "target_node_id": text_node_id,
            "relation_kind": "causes",
            "evidence_hash": evidence_digest,
            "evidence_locator": "charter-01:section-2"
        })),
    )
    .await;
    assert_eq!(bad_corr_status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(bad_corr_json["error_code"], "CORRELATION_NOT_CAUSATION");

    let (good_corr_status, good_corr_json) = send_json_request(
        &router,
        Method::POST,
        "/api/correlations",
        Some(json!({
            "source_node_id": law_node_id,
            "target_node_id": text_node_id,
            "relation_kind": "references_passage",
            "evidence_hash": evidence_digest,
            "evidence_locator": "charter-01:section-2",
            "review_state": "human_reviewed"
        })),
    )
    .await;
    assert_eq!(good_corr_status, StatusCode::CREATED);
    assert_eq!(good_corr_json["relation_kind"], "references_passage");
    assert_eq!(good_corr_json["implies_causation"], false);
    assert_eq!(good_corr_json["implies_identity"], false);

    // 5. GET /api/correlations & GET /api/graph
    let (list_corr_status, list_corr_json) =
        send_json_request(&router, Method::GET, "/api/correlations", None).await;
    assert_eq!(list_corr_status, StatusCode::OK);
    assert_eq!(list_corr_json["count"], 1);

    let (graph_status, graph_json) = send_json_request(
        &router,
        Method::GET,
        &format!("/api/graph?root_node_id={law_node_id}&depth=2"),
        None,
    )
    .await;
    assert_eq!(graph_status, StatusCode::OK);
    assert_eq!(graph_json["node_count"], 2);
    assert_eq!(graph_json["edge_count"], 1);
    assert_eq!(graph_json["alias_auto_merge_enabled"], false);

    // 6. POST /api/mizan/evaluate (unknown, normative rejection, CHILD_SENSITIVE block)
    let (mizan_unknown_status, mizan_unknown_json) = send_json_request(
        &router,
        Method::POST,
        "/api/mizan/evaluate",
        Some(json!({
            "subject_id": "subj-01",
            "subject_label": "Provenance evidence audit",
            "scope_description": "Check empty evidence behavior",
            "evidence_inputs": []
        })),
    )
    .await;
    assert_eq!(mizan_unknown_status, StatusCode::OK);
    assert_eq!(mizan_unknown_json["status"], "insufficient_evidence");
    assert_eq!(mizan_unknown_json["advisory_only"], true);
    assert_eq!(mizan_unknown_json["policy_version"], "mizan-boundary-v1");
    assert!(mizan_unknown_json["advisory_score"].is_null());

    let (mizan_norm_status, mizan_norm_json) = send_json_request(
        &router,
        Method::POST,
        "/api/mizan/evaluate",
        Some(json!({
            "subject_id": "subj-02",
            "subject_label": "Normative violation attempt",
            "scope_description": "Attempting divine_judgment score",
            "evidence_inputs": []
        })),
    )
    .await;
    assert_eq!(mizan_norm_status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        mizan_norm_json["error_code"],
        "NORMATIVE_BOUNDARY_VIOLATION"
    );

    let (mizan_child_status, mizan_child_json) = send_json_request(
        &router,
        Method::POST,
        "/api/mizan/evaluate",
        Some(json!({
            "subject_id": "subj-013",
            "subject_label": "Safeguarded minor record",
            "scope_description": "Must block fail-closed",
            "sensitivity_class": "CHILD_SENSITIVE",
            "evidence_inputs": []
        })),
    )
    .await;
    assert_eq!(mizan_child_status, StatusCode::FORBIDDEN);
    assert_eq!(mizan_child_json["error_code"], "CHILD_SENSITIVE_BLOCKED");

    // 7. GET /api/profiles/crayon & GET /api/profiles/rgbl
    let (crayon_status, crayon_json) =
        send_json_request(&router, Method::GET, "/api/profiles/crayon", None).await;
    assert_eq!(crayon_status, StatusCode::OK);
    assert_eq!(crayon_json["version"], "1.3");
    assert_eq!(crayon_json["merge_permitted_with_rgbl"], false);
    assert_eq!(crayon_json["cumulative_levels"][1]["label"], "GOVERNING ORDER");
    assert_eq!(crayon_json["cumulative_levels"][2]["label"], "BEING");
    assert_eq!(crayon_json["nine_gates"].as_array().expect("9 gates").len(), 9);
    assert_eq!(
        crayon_json["thirteen_ways"].as_array().expect("13 ways").len(),
        13
    );

    let (rgbl_status, rgbl_json) =
        send_json_request(&router, Method::GET, "/api/profiles/rgbl", None).await;
    assert_eq!(rgbl_status, StatusCode::OK);
    assert_eq!(rgbl_json["version"], "1.0");
    assert_eq!(rgbl_json["merge_permitted_with_crayon"], false);
    assert_eq!(rgbl_json["epistemic_layers"][1]["label"], "Guidance");
    assert_eq!(rgbl_json["epistemic_layers"][2]["label"], "Balance");
    assert_ne!(crayon_json["profile_id"], rgbl_json["profile_id"]);
}

#[tokio::test]
async fn mtime_api_converts_timestamps_and_never_claims_religious_authority() {
    let store = NexusGraphStore::open_in_memory().expect("in-memory store");
    let router = build_router(AppState::new(store));

    let (j2000_status, j2000_json) = send_json_request(
        &router,
        Method::GET,
        "/api/time/convert?ts=2000-01-01T12:00:00Z",
        None,
    )
    .await;
    assert_eq!(j2000_status, StatusCode::OK);
    assert_eq!(j2000_json["profile"], "antikythera-mtime-v1");
    assert_eq!(j2000_json["religious_authority"], false);
    assert_eq!(j2000_json["unix_timestamp"], 946_728_000);
    let jd = j2000_json["julian_day"].as_f64().expect("julian_day float");
    assert!((jd - 2_451_545.0).abs() < 1e-6);
    assert!(j2000_json["hijri_tabular"]["year"].as_i64().is_some());
    assert!(j2000_json["solar_longitude_deg"].as_f64().is_some());

    let (now_status, now_json) =
        send_json_request(&router, Method::GET, "/api/time/convert?ts=now", None).await;
    assert_eq!(now_status, StatusCode::OK);
    assert_eq!(now_json["religious_authority"], false);
    assert_eq!(now_json["profile"], "antikythera-mtime-v1");

    let (bad_status, bad_json) = send_json_request(
        &router,
        Method::GET,
        "/api/time/convert?ts=invalid-timestamp",
        None,
    )
    .await;
    assert_eq!(bad_status, StatusCode::BAD_REQUEST);
    assert_eq!(bad_json["error_code"], "INVALID_TIMESTAMP");
}

#[tokio::test]
async fn radar_ingest_persists_event_legend_preserves_uncertainty_and_links_domain_node() {
    let store = NexusGraphStore::open_in_memory().expect("in-memory store");
    // Seed a domain/source node so POST /api/radar/ingest can link a RELATIONSHIP_CORRELATION edge.
    let domain_node = store
        .create_node(CreateNodeRequest {
            id: Some("domain-geopolitics".to_owned()),
            domain: ProvenanceDomain::PerspectiveJizz,
            canonical_label: "geopolitics".to_owned(),
            corpus_ref: "mw-echo://domain/geopolitics".to_owned(),
            uncertainty_state: Some(UncertaintyState::HumanReviewed),
            alias_set: None,
            sensitivity_class: None,
            metadata: None,
        })
        .expect("seed domain node");

    let router = build_router(AppState::new(store.clone()));

    let (create_status, create_json) = send_json_request(
        &router,
        Method::POST,
        "/api/radar/ingest",
        Some(json!({
            "eventId": "evt-radar-001",
            "title": "Maritime corridor disruption reported",
            "summary": "Initial multi-source radar cluster detected.",
            "primaryDomain": "geopolitics",
            "clusterKey": "cluster-maritime-01",
            "sourceUri": "https://echo.moonwitness.org/radar/evt-radar-001",
            "occurredAt": "2026-11-26T12:00:00Z",
            "domains": ["geopolitics", "maritime"]
        })),
    )
    .await;
    assert_eq!(create_status, StatusCode::CREATED);
    assert_eq!(create_json["status"], "created");
    assert_eq!(create_json["uncertainty_state_preserved"], true);
    assert_eq!(create_json["event_node"]["id"], "radar-event-evt-radar-001");
    assert_eq!(create_json["event_node"]["domain"], "EVENT_LEGEND");
    assert_eq!(create_json["event_node"]["uncertainty_state"], "unknown");
    assert_eq!(
        create_json["event_node"]["corpus_ref"],
        "https://echo.moonwitness.org/radar/evt-radar-001"
    );
    assert_eq!(
        create_json["linked_target_node_id"],
        domain_node.id.as_str()
    );
    assert_eq!(
        create_json["correlation_edge"]["relation_kind"],
        "correlated_with"
    );
    assert_eq!(create_json["correlation_edge"]["implies_causation"], false);
    assert_eq!(create_json["correlation_edge"]["implies_identity"], false);

    // Re-ingest with updated title & summary: must update without mutating corpus_ref or uncertainty_state.
    let (update_status, update_json) = send_json_request(
        &router,
        Method::POST,
        "/api/radar/ingest",
        Some(json!({
            "eventId": "evt-radar-001",
            "title": "Maritime corridor disruption updated",
            "summary": "Corroborated by second radar sweep.",
            "primaryDomain": "geopolitics",
            "clusterKey": "cluster-maritime-02",
            "sourceUri": "https://echo.moonwitness.org/radar/different-uri-ignored-for-corpus-ref",
            "occurredAt": "2026-11-26T12:30:00Z",
            "domains": ["geopolitics"]
        })),
    )
    .await;
    assert_eq!(update_status, StatusCode::OK);
    assert_eq!(update_json["status"], "updated");
    assert_eq!(update_json["uncertainty_state_preserved"], true);
    assert_eq!(
        update_json["event_node"]["canonical_label"],
        "Maritime corridor disruption updated"
    );
    assert_eq!(update_json["event_node"]["uncertainty_state"], "unknown");
    assert_eq!(
        update_json["event_node"]["corpus_ref"],
        "https://echo.moonwitness.org/radar/evt-radar-001"
    );

    // Verify persistence via GET /api/nodes/{node_id}
    let (get_status, get_json) = send_json_request(
        &router,
        Method::GET,
        "/api/nodes/radar-event-evt-radar-001",
        None,
    )
    .await;
    assert_eq!(get_status, StatusCode::OK);
    assert_eq!(
        get_json["canonical_label"],
        "Maritime corridor disruption updated"
    );
    assert_eq!(get_json["metadata"]["mtime"]["religious_authority"], false);
}

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use thiserror::Error;

/// Canonical 7 Provenance Domains of MoonWitness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProvenanceDomain {
    TextRgbl,
    StoryMftl,
    EventLegend,
    PersonSuperhero,
    LawAws,
    PerspectiveJizz,
    RelationshipCorrelation,
}

impl ProvenanceDomain {
    pub const ALL: [ProvenanceDomain; 7] = [
        ProvenanceDomain::TextRgbl,
        ProvenanceDomain::StoryMftl,
        ProvenanceDomain::EventLegend,
        ProvenanceDomain::PersonSuperhero,
        ProvenanceDomain::LawAws,
        ProvenanceDomain::PerspectiveJizz,
        ProvenanceDomain::RelationshipCorrelation,
    ];

    pub const fn code(self) -> &'static str {
        match self {
            ProvenanceDomain::TextRgbl => "TEXT_RGBL",
            ProvenanceDomain::StoryMftl => "STORY_MFTL",
            ProvenanceDomain::EventLegend => "EVENT_LEGEND",
            ProvenanceDomain::PersonSuperhero => "PERSON_SUPERHERO",
            ProvenanceDomain::LawAws => "LAW_AWS",
            ProvenanceDomain::PerspectiveJizz => "PERSPECTIVE_JIZZ",
            ProvenanceDomain::RelationshipCorrelation => "RELATIONSHIP_CORRELATION",
        }
    }

    pub fn metadata(self) -> DomainMetadata {
        match self {
            ProvenanceDomain::TextRgbl => DomainMetadata {
                domain: self,
                code: self.code().to_owned(),
                semantic_label: "TEXT / RGBL".to_owned(),
                alias_note: "RGBL corpus lineage; distinct from CRAYON inquiry profile and RGB color metaphors".to_owned(),
                owned_question: "What exact text, edition, passage, rights, and provenance are being referenced?".to_owned(),
                boundary_rule: "Owns exact textual witnesses and corpus citations only; does not own interpretation or story canon.".to_owned(),
                reference_lineage: "moonwitness-corpus".to_owned(),
                forbids_claiming: vec![
                    "story_canon_ownership".to_owned(),
                    "doctrinal_verdict".to_owned(),
                    "automatic_translation_equivalence".to_owned(),
                ],
            },
            ProvenanceDomain::StoryMftl => DomainMetadata {
                domain: self,
                code: self.code().to_owned(),
                semantic_label: "STORY / MFTL".to_owned(),
                alias_note: "Narrative, lore, myth, and belief-source register".to_owned(),
                owned_question: "What narrative, story seed, or belief source is being recorded?".to_owned(),
                boundary_rule: "A narrative, myth, or belief can be recorded without being accepted or promoted as empirical historical fact.".to_owned(),
                reference_lineage: "rocksoul-mftl".to_owned(),
                forbids_claiming: vec![
                    "empirical_historical_fact".to_owned(),
                    "real_world_identity_proof".to_owned(),
                ],
            },
            ProvenanceDomain::EventLegend => DomainMetadata {
                domain: self,
                code: self.code().to_owned(),
                semantic_label: "EVENT / LEGEND".to_owned(),
                alias_note: "Evidence-bound historical and observational event reconstruction".to_owned(),
                owned_question: "What historical or observed event is supported by explicit evidence?".to_owned(),
                boundary_rule: "Event reconstruction remains strictly separate from later narrative framing, legend embellishment, or causal extrapolation.".to_owned(),
                reference_lineage: "rocksoul-legend".to_owned(),
                forbids_claiming: vec![
                    "narrative_canon_override".to_owned(),
                    "unverified_causation".to_owned(),
                ],
            },
            ProvenanceDomain::PersonSuperhero => DomainMetadata {
                domain: self,
                code: self.code().to_owned(),
                semantic_label: "PERSON / SUPERHERO".to_owned(),
                alias_note: "Actors, witnesses, personas, authors, and transmitters".to_owned(),
                owned_question: "Who acted, witnessed, authored, or transmitted a record?".to_owned(),
                boundary_rule: "Role, persona, or alias coexistence does not prove shared identity, auto-merge aliases, or establish moral worth.".to_owned(),
                reference_lineage: "rocksoul-superhero".to_owned(),
                forbids_claiming: vec![
                    "alias_auto_merge".to_owned(),
                    "moral_worth_verdict".to_owned(),
                    "sacred_identity_inference".to_owned(),
                ],
            },
            ProvenanceDomain::LawAws => DomainMetadata {
                domain: self,
                code: self.code().to_owned(),
                semantic_label: "LAW / AWS (Angel With Shotgun)".to_owned(),
                alias_note: "AWS denotes ANGEL WITH SHOTGUN as the MoonWitness semantic domain for LAW/Jurisdiction, NOT Amazon Web Services".to_owned(),
                owned_question: "What rule, authority, mandate, or jurisdiction applies?".to_owned(),
                boundary_rule: "Tracks scoped legal/jurisdictional rules and mandates; never issues autonomous legal verdicts or divine rulings.".to_owned(),
                reference_lineage: "rocksoul-aws".to_owned(),
                forbids_claiming: vec![
                    "autonomous_legal_verdict".to_owned(),
                    "divine_judgment".to_owned(),
                    "corpus_text_rewrite".to_owned(),
                ],
            },
            ProvenanceDomain::PerspectiveJizz => DomainMetadata {
                domain: self,
                code: self.code().to_owned(),
                semantic_label: "PERSPECTIVE / JIZZ".to_owned(),
                alias_note: "Framing, viewpoint, and interpretive lens register".to_owned(),
                owned_question: "How is the phenomenon framed, interpreted, or seen?".to_owned(),
                boundary_rule: "Perspective never owns, overwrites, or canonically mutates the underlying text, event, or person.".to_owned(),
                reference_lineage: "rocksoul-jizz".to_owned(),
                forbids_claiming: vec![
                    "underlying_text_ownership".to_owned(),
                    "underlying_event_ownership".to_owned(),
                    "underlying_person_ownership".to_owned(),
                ],
            },
            ProvenanceDomain::RelationshipCorrelation => DomainMetadata {
                domain: self,
                code: self.code().to_owned(),
                semantic_label: "RELATIONSHIP / CORRELATION".to_owned(),
                alias_note: "Typed cross-domain linkage with explicit evidence hashes and uncertainty".to_owned(),
                owned_question: "How do reviewed records relate across provenance domains?".to_owned(),
                boundary_rule: "Correlation is strictly NOT causation, NOT identity equivalence, and NOT prophetic or normative fulfillment.".to_owned(),
                reference_lineage: "rocksoul-correlation".to_owned(),
                forbids_claiming: vec![
                    "causation".to_owned(),
                    "identity_equivalence".to_owned(),
                    "prophetic_fulfillment".to_owned(),
                    "normative_verdict".to_owned(),
                ],
            },
        }
    }
}

impl fmt::Display for ProvenanceDomain {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl FromStr for ProvenanceDomain {
    type Err = BoundaryViolation;

    fn from_str(raw_domain: &str) -> Result<Self, Self::Err> {
        match raw_domain.trim().to_ascii_uppercase().as_str() {
            "TEXT_RGBL" => Ok(ProvenanceDomain::TextRgbl),
            "STORY_MFTL" => Ok(ProvenanceDomain::StoryMftl),
            "EVENT_LEGEND" => Ok(ProvenanceDomain::EventLegend),
            "PERSON_SUPERHERO" => Ok(ProvenanceDomain::PersonSuperhero),
            "LAW_AWS" => Ok(ProvenanceDomain::LawAws),
            "PERSPECTIVE_JIZZ" => Ok(ProvenanceDomain::PerspectiveJizz),
            "RELATIONSHIP_CORRELATION" => Ok(ProvenanceDomain::RelationshipCorrelation),
            other => Err(BoundaryViolation::UnknownDomain {
                attempted_domain: other.to_owned(),
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainMetadata {
    pub domain: ProvenanceDomain,
    pub code: String,
    pub semantic_label: String,
    pub alias_note: String,
    pub owned_question: String,
    pub boundary_rule: String,
    pub reference_lineage: String,
    pub forbids_claiming: Vec<String>,
}

pub fn canonical_domains() -> Vec<DomainMetadata> {
    ProvenanceDomain::ALL
        .into_iter()
        .map(ProvenanceDomain::metadata)
        .collect()
}

/// Explicit review lifecycle states for ontology edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewState {
    Candidate,
    NeedsSource,
    Conflicting,
    Rejected,
    HumanReviewed,
}

impl ReviewState {
    pub const fn as_str(self) -> &'static str {
        match self {
            ReviewState::Candidate => "candidate",
            ReviewState::NeedsSource => "needs_source",
            ReviewState::Conflicting => "conflicting",
            ReviewState::Rejected => "rejected",
            ReviewState::HumanReviewed => "human_reviewed",
        }
    }
}

impl FromStr for ReviewState {
    type Err = BoundaryViolation;

    fn from_str(raw_state: &str) -> Result<Self, Self::Err> {
        match raw_state.trim().to_ascii_lowercase().as_str() {
            "candidate" => Ok(ReviewState::Candidate),
            "needs_source" => Ok(ReviewState::NeedsSource),
            "conflicting" => Ok(ReviewState::Conflicting),
            "rejected" => Ok(ReviewState::Rejected),
            "human_reviewed" => Ok(ReviewState::HumanReviewed),
            other => Err(BoundaryViolation::UnknownReviewState {
                attempted_state: other.to_owned(),
            }),
        }
    }
}

/// Explicit uncertainty states for ontology nodes (Unknown-Stays-Unknown).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UncertaintyState {
    Unknown,
    Candidate,
    NeedsSource,
    Conflicting,
    HumanReviewed,
}

impl UncertaintyState {
    pub const fn as_str(self) -> &'static str {
        match self {
            UncertaintyState::Unknown => "unknown",
            UncertaintyState::Candidate => "candidate",
            UncertaintyState::NeedsSource => "needs_source",
            UncertaintyState::Conflicting => "conflicting",
            UncertaintyState::HumanReviewed => "human_reviewed",
        }
    }
}

impl FromStr for UncertaintyState {
    type Err = BoundaryViolation;

    fn from_str(raw_state: &str) -> Result<Self, Self::Err> {
        match raw_state.trim().to_ascii_lowercase().as_str() {
            "unknown" => Ok(UncertaintyState::Unknown),
            "candidate" => Ok(UncertaintyState::Candidate),
            "needs_source" => Ok(UncertaintyState::NeedsSource),
            "conflicting" => Ok(UncertaintyState::Conflicting),
            "human_reviewed" => Ok(UncertaintyState::HumanReviewed),
            other => Err(BoundaryViolation::UnknownUncertaintyState {
                attempted_state: other.to_owned(),
            }),
        }
    }
}

/// Permitted typed relation predicates. Causation, identity auto-merge,
/// prophetic fulfillment, and normative rulings are structurally excluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationKind {
    ReferencesPassage,
    AppearsIn,
    WitnessedEvent,
    FramedByPerspective,
    GovernedByJurisdiction,
    ThematicallyRelated,
    CandidateVariantOf,
    UnresolvedAliasOf,
    DistinctFrom,
    CorrelatedWith,
    SupersedesDraft,
}

impl RelationKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            RelationKind::ReferencesPassage => "references_passage",
            RelationKind::AppearsIn => "appears_in",
            RelationKind::WitnessedEvent => "witnessed_event",
            RelationKind::FramedByPerspective => "framed_by_perspective",
            RelationKind::GovernedByJurisdiction => "governed_by_jurisdiction",
            RelationKind::ThematicallyRelated => "thematically_related",
            RelationKind::CandidateVariantOf => "candidate_variant_of",
            RelationKind::UnresolvedAliasOf => "unresolved_alias_of",
            RelationKind::DistinctFrom => "distinct_from",
            RelationKind::CorrelatedWith => "correlated_with",
            RelationKind::SupersedesDraft => "supersedes_draft",
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BoundaryViolation {
    #[error("unknown provenance domain '{attempted_domain}'")]
    UnknownDomain { attempted_domain: String },

    #[error("unknown review state '{attempted_state}'")]
    UnknownReviewState { attempted_state: String },

    #[error("unknown uncertainty state '{attempted_state}'")]
    UnknownUncertaintyState { attempted_state: String },

    #[error("boundary violation (correlation != causation): relation '{attempted_relation}' attempts causal assertion")]
    CorrelationNotCausation { attempted_relation: String },

    #[error("boundary violation (correlation != identity): relation '{attempted_relation}' attempts identity equivalence or alias auto-merge")]
    CorrelationNotIdentity { attempted_relation: String },

    #[error("boundary violation (correlation != fulfillment): relation '{attempted_relation}' attempts prophetic or metaphysical fulfillment claim")]
    CorrelationNotFulfillment { attempted_relation: String },

    #[error("boundary violation (non-normative ontology): relation '{attempted_relation}' attempts moral, divine, or legal verdict")]
    NormativeRelationProhibited { attempted_relation: String },

    #[error("cross-domain boundary violation between {source_domain} and {target_domain} for relation '{relation}': {rationale}")]
    CrossDomainOwnershipViolation {
        source_domain: ProvenanceDomain,
        target_domain: ProvenanceDomain,
        relation: String,
        rationale: String,
    },

    #[error("self-referential correlation edge rejected for node '{node_id}'")]
    SelfLoopProhibited { node_id: String },

    #[error("correlation edge requires a non-empty hexadecimal evidence hash (16..=128 hex chars)")]
    InvalidEvidenceHash { supplied: String },

    #[error("unregistered relation predicate '{attempted_relation}'; use canonical typed correlation vocabulary")]
    UnknownRelationKind { attempted_relation: String },
}

impl BoundaryViolation {
    pub const fn error_code(&self) -> &'static str {
        match self {
            BoundaryViolation::UnknownDomain { .. } => "UNKNOWN_DOMAIN",
            BoundaryViolation::UnknownReviewState { .. } => "UNKNOWN_REVIEW_STATE",
            BoundaryViolation::UnknownUncertaintyState { .. } => "UNKNOWN_UNCERTAINTY_STATE",
            BoundaryViolation::CorrelationNotCausation { .. } => "CORRELATION_NOT_CAUSATION",
            BoundaryViolation::CorrelationNotIdentity { .. } => "CORRELATION_NOT_IDENTITY",
            BoundaryViolation::CorrelationNotFulfillment { .. } => "CORRELATION_NOT_FULFILLMENT",
            BoundaryViolation::NormativeRelationProhibited { .. } => "NORMATIVE_RELATION_PROHIBITED",
            BoundaryViolation::CrossDomainOwnershipViolation { .. } => {
                "CROSS_DOMAIN_OWNERSHIP_VIOLATION"
            }
            BoundaryViolation::SelfLoopProhibited { .. } => "SELF_LOOP_PROHIBITED",
            BoundaryViolation::InvalidEvidenceHash { .. } => "INVALID_EVIDENCE_HASH",
            BoundaryViolation::UnknownRelationKind { .. } => "UNKNOWN_RELATION_KIND",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidatedBoundary {
    pub relation_kind: RelationKind,
    pub source_domain: ProvenanceDomain,
    pub target_domain: ProvenanceDomain,
    pub implies_causation: bool,
    pub implies_identity: bool,
    pub implies_fulfillment: bool,
    pub alias_auto_merge_prohibited: bool,
}

const CAUSATION_TERMS: &[&str] = &[
    "causes",
    "caused_by",
    "proves_causation",
    "causal_proof",
    "deterministically_causes",
    "directly_causes",
    "implies_causation",
    "causes_event",
];

const IDENTITY_TERMS: &[&str] = &[
    "same_as",
    "owl_same_as",
    "identical_to",
    "is_identical_to",
    "is_same_entity",
    "auto_merge",
    "auto_merge_alias",
    "merge_alias",
    "merges_identity_with",
    "proves_identity",
    "equals_entity",
];

const FULFILLMENT_TERMS: &[&str] = &[
    "fulfills",
    "fulfills_prophecy",
    "proves_prophecy",
    "supernatural_proof",
    "divine_confirmation",
    "divine_endorsement",
];

const NORMATIVE_RELATION_TERMS: &[&str] = &[
    "condemns",
    "absolves",
    "declares_guilty",
    "guilty_of",
    "salvation_verdict",
    "divine_verdict_on",
    "proves_moral_worth",
    "confirms_divine_judgment",
];

const OWNERSHIP_CONFLATION_TERMS: &[&str] = &[
    "owns_source",
    "rewrites_canonical_text",
    "proves_historical_fact",
    "establishes_story_canon",
    "establishes_interpretation",
    "overrides_corpus_text",
];

pub fn normalize_relation_token(raw_relation: &str) -> String {
    raw_relation
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' '], "_")
}

pub fn validate_evidence_hash(evidence_hash: &str) -> Result<String, BoundaryViolation> {
    let normalized_hash = evidence_hash.trim();
    let hex_part = normalized_hash
        .strip_prefix("sha256:")
        .unwrap_or(normalized_hash);
    let valid_len = (16..=128).contains(&hex_part.len());
    let valid_hex = hex_part.bytes().all(|byte| byte.is_ascii_hexdigit());
    if !valid_len || !valid_hex {
        return Err(BoundaryViolation::InvalidEvidenceHash {
            supplied: evidence_hash.to_owned(),
        });
    }
    Ok(normalized_hash.to_ascii_lowercase())
}

pub fn parse_relation_kind(raw_relation: &str) -> Result<RelationKind, BoundaryViolation> {
    let normalized = normalize_relation_token(raw_relation);

    if CAUSATION_TERMS.contains(&normalized.as_str()) {
        return Err(BoundaryViolation::CorrelationNotCausation {
            attempted_relation: raw_relation.to_owned(),
        });
    }
    if IDENTITY_TERMS.contains(&normalized.as_str()) {
        return Err(BoundaryViolation::CorrelationNotIdentity {
            attempted_relation: raw_relation.to_owned(),
        });
    }
    if FULFILLMENT_TERMS.contains(&normalized.as_str()) {
        return Err(BoundaryViolation::CorrelationNotFulfillment {
            attempted_relation: raw_relation.to_owned(),
        });
    }
    if NORMATIVE_RELATION_TERMS.contains(&normalized.as_str()) {
        return Err(BoundaryViolation::NormativeRelationProhibited {
            attempted_relation: raw_relation.to_owned(),
        });
    }

    match normalized.as_str() {
        "references_passage" => Ok(RelationKind::ReferencesPassage),
        "appears_in" => Ok(RelationKind::AppearsIn),
        "witnessed_event" => Ok(RelationKind::WitnessedEvent),
        "framed_by_perspective" => Ok(RelationKind::FramedByPerspective),
        "governed_by_jurisdiction" => Ok(RelationKind::GovernedByJurisdiction),
        "thematically_related" => Ok(RelationKind::ThematicallyRelated),
        "candidate_variant_of" => Ok(RelationKind::CandidateVariantOf),
        "unresolved_alias_of" => Ok(RelationKind::UnresolvedAliasOf),
        "distinct_from" => Ok(RelationKind::DistinctFrom),
        "correlated_with" => Ok(RelationKind::CorrelatedWith),
        "supersedes_draft" => Ok(RelationKind::SupersedesDraft),
        _ => Err(BoundaryViolation::UnknownRelationKind {
            attempted_relation: raw_relation.to_owned(),
        }),
    }
}

/// Strict cross-domain boundary validator enforcing:
/// 1. correlation != causation != identity != fulfillment
/// 2. non-normative relation vocabulary
/// 3. explicit domain ownership boundaries across the 7 canonical domains
/// 4. mandatory evidence hash and no self-loops
pub fn validate_cross_domain_boundary(
    source_node_id: &str,
    target_node_id: &str,
    source_domain: ProvenanceDomain,
    target_domain: ProvenanceDomain,
    raw_relation: &str,
    evidence_hash: &str,
) -> Result<ValidatedBoundary, BoundaryViolation> {
    if source_node_id.trim() == target_node_id.trim() {
        return Err(BoundaryViolation::SelfLoopProhibited {
            node_id: source_node_id.to_owned(),
        });
    }

    let _validated_hash = validate_evidence_hash(evidence_hash)?;
    let normalized = normalize_relation_token(raw_relation);

    if OWNERSHIP_CONFLATION_TERMS.contains(&normalized.as_str()) {
        return Err(BoundaryViolation::CrossDomainOwnershipViolation {
            source_domain,
            target_domain,
            relation: raw_relation.to_owned(),
            rationale: "No provenance domain may claim ownership, rewrite authority, or empirical proof across another domain boundary.".to_owned(),
        });
    }

    let relation_kind = parse_relation_kind(raw_relation)?;

    match relation_kind {
        RelationKind::ReferencesPassage => {
            if target_domain != ProvenanceDomain::TextRgbl {
                return Err(BoundaryViolation::CrossDomainOwnershipViolation {
                    source_domain,
                    target_domain,
                    relation: relation_kind.as_str().to_owned(),
                    rationale: "references_passage must target a TEXT_RGBL corpus node.".to_owned(),
                });
            }
        }
        RelationKind::WitnessedEvent => {
            if source_domain != ProvenanceDomain::PersonSuperhero
                || target_domain != ProvenanceDomain::EventLegend
            {
                return Err(BoundaryViolation::CrossDomainOwnershipViolation {
                    source_domain,
                    target_domain,
                    relation: relation_kind.as_str().to_owned(),
                    rationale: "witnessed_event requires PERSON_SUPERHERO -> EVENT_LEGEND."
                        .to_owned(),
                });
            }
        }
        RelationKind::FramedByPerspective => {
            if target_domain != ProvenanceDomain::PerspectiveJizz
                || source_domain == ProvenanceDomain::PerspectiveJizz
            {
                return Err(BoundaryViolation::CrossDomainOwnershipViolation {
                    source_domain,
                    target_domain,
                    relation: relation_kind.as_str().to_owned(),
                    rationale: "framed_by_perspective must link a non-perspective record to a PERSPECTIVE_JIZZ framing node.".to_owned(),
                });
            }
        }
        RelationKind::GovernedByJurisdiction => {
            if target_domain != ProvenanceDomain::LawAws {
                return Err(BoundaryViolation::CrossDomainOwnershipViolation {
                    source_domain,
                    target_domain,
                    relation: relation_kind.as_str().to_owned(),
                    rationale: "governed_by_jurisdiction must target a LAW_AWS domain node."
                        .to_owned(),
                });
            }
        }
        RelationKind::AppearsIn => {
            let valid_source = matches!(
                source_domain,
                ProvenanceDomain::PersonSuperhero | ProvenanceDomain::EventLegend
            );
            let valid_target = matches!(
                target_domain,
                ProvenanceDomain::StoryMftl
                    | ProvenanceDomain::TextRgbl
                    | ProvenanceDomain::EventLegend
            );
            if !valid_source || !valid_target {
                return Err(BoundaryViolation::CrossDomainOwnershipViolation {
                    source_domain,
                    target_domain,
                    relation: relation_kind.as_str().to_owned(),
                    rationale: "appears_in requires PERSON_SUPERHERO or EVENT_LEGEND targeting STORY_MFTL, TEXT_RGBL, or EVENT_LEGEND.".to_owned(),
                });
            }
        }
        RelationKind::CandidateVariantOf
        | RelationKind::UnresolvedAliasOf
        | RelationKind::SupersedesDraft => {
            if source_domain != target_domain {
                return Err(BoundaryViolation::CrossDomainOwnershipViolation {
                    source_domain,
                    target_domain,
                    relation: relation_kind.as_str().to_owned(),
                    rationale: "Variant, alias, and draft supersession links must remain within the same provenance domain without cross-domain conflation.".to_owned(),
                });
            }
        }
        RelationKind::ThematicallyRelated
        | RelationKind::DistinctFrom
        | RelationKind::CorrelatedWith => {}
    }

    Ok(ValidatedBoundary {
        relation_kind,
        source_domain,
        target_domain,
        implies_causation: false,
        implies_identity: false,
        implies_fulfillment: false,
        alias_auto_merge_prohibited: true,
    })
}

pub fn permitted_relation_kinds() -> Vec<&'static str> {
    vec![
        RelationKind::ReferencesPassage.as_str(),
        RelationKind::AppearsIn.as_str(),
        RelationKind::WitnessedEvent.as_str(),
        RelationKind::FramedByPerspective.as_str(),
        RelationKind::GovernedByJurisdiction.as_str(),
        RelationKind::ThematicallyRelated.as_str(),
        RelationKind::CandidateVariantOf.as_str(),
        RelationKind::UnresolvedAliasOf.as_str(),
        RelationKind::DistinctFrom.as_str(),
        RelationKind::CorrelatedWith.as_str(),
        RelationKind::SupersedesDraft.as_str(),
    ]
}

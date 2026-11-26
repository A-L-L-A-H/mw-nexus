use crate::domains::ProvenanceDomain;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const POLICY_VERSION: &str = "mizan-boundary-v1";
pub const PROFILE_ID: &str = "mw:profile:mizan-policy-boundary-v1";
pub const ADR_REFERENCE: &str = "ADR-0005";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SensitivityClass {
    PublicDraft,
    #[default]
    Internal,
    Confidential,
    Restricted,
    ChildSensitive,
}

impl SensitivityClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            SensitivityClass::PublicDraft => "PUBLIC_DRAFT",
            SensitivityClass::Internal => "INTERNAL",
            SensitivityClass::Confidential => "CONFIDENTIAL",
            SensitivityClass::Restricted => "RESTRICTED",
            SensitivityClass::ChildSensitive => "CHILD_SENSITIVE",
        }
    }

    pub fn from_db_str(raw_value: &str) -> Result<Self, String> {
        match raw_value.trim().to_ascii_uppercase().as_str() {
            "PUBLIC_DRAFT" => Ok(SensitivityClass::PublicDraft),
            "INTERNAL" => Ok(SensitivityClass::Internal),
            "CONFIDENTIAL" => Ok(SensitivityClass::Confidential),
            "RESTRICTED" => Ok(SensitivityClass::Restricted),
            "CHILD_SENSITIVE" => Ok(SensitivityClass::ChildSensitive),
            other => Err(format!("unknown sensitivity class '{other}'")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceReviewState {
    VerifiedHumanReviewed,
    Unreviewed,
    Missing,
    Conflicting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStance {
    Supporting,
    Counter,
    Neutral,
    Indeterminate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceFactorInput {
    pub evidence_id: String,
    #[serde(default)]
    pub corpus_ref: Option<String>,
    pub domain: ProvenanceDomain,
    pub review_state: EvidenceReviewState,
    pub stance: EvidenceStance,
    /// Bounded diagnostic signal in 0.0..=1.0. None is NEVER coerced to 0.0.
    #[serde(default)]
    pub signal_strength: Option<f64>,
    #[serde(default)]
    pub sensitivity_class: SensitivityClass,
    #[serde(default)]
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MizanEvaluationRequest {
    pub subject_id: String,
    pub subject_label: String,
    pub scope_description: String,
    #[serde(default)]
    pub sensitivity_class: SensitivityClass,
    #[serde(default)]
    pub involves_minor: bool,
    #[serde(default)]
    pub requested_claim: Option<String>,
    #[serde(default)]
    pub requested_action: Option<String>,
    #[serde(default)]
    pub factor_schema: Option<String>,
    #[serde(default)]
    pub evidence_inputs: Vec<EvidenceFactorInput>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MizanStatus {
    AdvisoryNonNormative,
    Unknown,
    InsufficientEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MizanDiagnostic {
    pub evaluation_id: String,
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
    pub conflict_preserved: bool,
    pub uncertainty_reasons: Vec<String>,
    pub evidence_ids: Vec<String>,
    pub conflicting_evidence_ids: Vec<String>,
    pub disclaimer: String,
    pub evaluated_at: DateTime<Utc>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum MizanRejection {
    #[error("fail-closed safeguarding block: CHILD_SENSITIVE or minor-scoped records are prohibited from Mizan evaluation and telemetry")]
    ChildSensitiveBlocked,

    #[error("ADR 0005 normative boundary violation in '{field}': prohibited normative, divine, or legal verdict wording '{matched_term}'")]
    NormativeBoundaryViolation { field: String, matched_term: String },

    #[error("ADR 0005 autonomous action boundary violation in '{field}': prohibited execution trigger '{matched_term}'")]
    AutonomousActionProhibited { field: String, matched_term: String },

    #[error("ADR 0005 legacy factor rejection: unverified legacy dev-mw-blueprint schema '{factor_schema}' is prohibited")]
    LegacyBlueprintFactorRejected { factor_schema: String },

    #[error("invalid Mizan evaluation input: {reason}")]
    InvalidInput { reason: String },
}

impl MizanRejection {
    pub const fn error_code(&self) -> &'static str {
        match self {
            MizanRejection::ChildSensitiveBlocked => "CHILD_SENSITIVE_BLOCKED",
            MizanRejection::NormativeBoundaryViolation { .. } => "NORMATIVE_BOUNDARY_VIOLATION",
            MizanRejection::AutonomousActionProhibited { .. } => "AUTONOMOUS_ACTION_PROHIBITED",
            MizanRejection::LegacyBlueprintFactorRejected { .. } => {
                "LEGACY_BLUEPRINT_FACTOR_REJECTED"
            }
            MizanRejection::InvalidInput { .. } => "INVALID_MIZAN_INPUT",
        }
    }
}

const PROHIBITED_NORMATIVE_PHRASES: &[&str] = &[
    "human worth",
    "human_worth",
    "moral worth",
    "moral_worth",
    "spiritual rank",
    "spiritual_rank",
    "salvation status",
    "salvation_status",
    "salvation verdict",
    "divine judgment",
    "divine_judgment",
    "divine verdict",
    "divine_verdict",
    "divine approval",
    "divine_approval",
    "divine condemnation",
    "afterlife level",
    "afterlife_level",
    "paradise level",
    "hellfire",
    "damned",
    "condemned",
    "takfir",
    "sin score",
    "sin_score",
    "legal verdict",
    "legal_verdict",
    "legal guilt",
    "legal_guilt",
    "legal liability",
    "legally liable",
    "criminal verdict",
    "criminal guilt",
    "convicted",
    "sentenced",
    "guilty",
];

const PROHIBITED_AUTONOMOUS_ACTIONS: &[&str] = &[
    "autonomous harm",
    "autonomous_harm",
    "autonomous publication",
    "autonomous_publication",
    "public_approved",
    "publish_public_approved",
    "promote_public_approved",
    "auto_publish",
    "auto_enforce",
    "issue_sanction",
    "suspend_account",
    "minor_disclosure",
    "minor_data_exposure",
    "expose_minor",
    "delete_record",
];

const PROHIBITED_LEGACY_SCHEMAS: &[&str] = &[
    "dev-mw-blueprint",
    "legacy_th",
    "legacy_ari",
    "th_ari_v1",
    "mizan-engine.fixtures.v1",
    "mizan-analytical.fixtures.v1.2",
];

fn contains_phrase_with_boundaries(haystack: &str, needle: &str) -> bool {
    let lower_haystack = haystack.to_ascii_lowercase();
    let lower_needle = needle.to_ascii_lowercase();
    let needle_bytes = lower_needle.as_bytes();
    let haystack_bytes = lower_haystack.as_bytes();

    if needle_bytes.is_empty() || needle_bytes.len() > haystack_bytes.len() {
        return false;
    }

    let mut search_offset = 0;
    while let Some(relative_index) = lower_haystack[search_offset..].find(&lower_needle) {
        let start_index = search_offset + relative_index;
        let end_index = start_index + needle_bytes.len();

        let left_boundary_ok = start_index == 0
            || !haystack_bytes[start_index - 1].is_ascii_alphanumeric();
        let right_boundary_ok = end_index == haystack_bytes.len()
            || !haystack_bytes[end_index].is_ascii_alphanumeric();

        if left_boundary_ok && right_boundary_ok {
            return true;
        }
        search_offset = start_index + 1;
    }

    false
}

fn scan_for_prohibited_text(field_name: &str, text: &str) -> Result<(), MizanRejection> {
    for phrase in PROHIBITED_NORMATIVE_PHRASES {
        if contains_phrase_with_boundaries(text, phrase) {
            return Err(MizanRejection::NormativeBoundaryViolation {
                field: field_name.to_owned(),
                matched_term: (*phrase).to_owned(),
            });
        }
    }

    for action_phrase in PROHIBITED_AUTONOMOUS_ACTIONS {
        if contains_phrase_with_boundaries(text, action_phrase) {
            return Err(MizanRejection::AutonomousActionProhibited {
                field: field_name.to_owned(),
                matched_term: (*action_phrase).to_owned(),
            });
        }
    }

    Ok(())
}

/// Evaluates bounded, non-normative Mizan/Hisab advisory diagnostics under ADR 0005.
/// - Fail-closed on CHILD_SENSITIVE or minor-scoped inputs (no metadata leak).
/// - Fail-closed on normative/divine/legal verdict wording or autonomous action requests.
/// - Rejects unverified legacy dev-mw-blueprint TH/ARI schemas.
/// - Preserves Unknown-Stays-Unknown: missing, unreviewed, or conflicting evidence emits
///   `unknown` or `insufficient_evidence` with `advisory_score: None` (NEVER coerced to 0.0 or safe).
pub fn evaluate_mizan(request: &MizanEvaluationRequest) -> Result<MizanDiagnostic, MizanRejection> {
    // Gate 1 (FM-04): Fail-closed block on CHILD_SENSITIVE or minor involvement.
    if request.sensitivity_class == SensitivityClass::ChildSensitive
        || request.involves_minor
        || request
            .evidence_inputs
            .iter()
            .any(|evidence_entry| evidence_entry.sensitivity_class == SensitivityClass::ChildSensitive)
    {
        return Err(MizanRejection::ChildSensitiveBlocked);
    }

    if request.subject_id.trim().is_empty() {
        return Err(MizanRejection::InvalidInput {
            reason: "subject_id must not be empty".to_owned(),
        });
    }
    if request.scope_description.trim().is_empty() {
        return Err(MizanRejection::InvalidInput {
            reason: "scope_description must not be empty".to_owned(),
        });
    }

    // Gate 2 (ADR 0005 Decision #1): Reject legacy unverified dev-mw-blueprint TH/ARI schemas.
    if let Some(schema_name) = &request.factor_schema {
        let lower_schema = schema_name.trim().to_ascii_lowercase();
        for forbidden_schema in PROHIBITED_LEGACY_SCHEMAS {
            if lower_schema.contains(forbidden_schema) {
                return Err(MizanRejection::LegacyBlueprintFactorRejected {
                    factor_schema: schema_name.clone(),
                });
            }
        }
    }

    // Gate 3 (FM-01 & FM-03): Reject normative/divine/legal wording and autonomous actions.
    scan_for_prohibited_text("subject_label", &request.subject_label)?;
    scan_for_prohibited_text("scope_description", &request.scope_description)?;
    if let Some(claim_text) = &request.requested_claim {
        scan_for_prohibited_text("requested_claim", claim_text)?;
    }
    if let Some(action_text) = &request.requested_action {
        scan_for_prohibited_text("requested_action", action_text)?;
    }

    for (position, evidence_entry) in request.evidence_inputs.iter().enumerate() {
        if evidence_entry.evidence_id.trim().is_empty() {
            return Err(MizanRejection::InvalidInput {
                reason: format!("evidence_inputs[{position}].evidence_id must not be empty"),
            });
        }
        scan_for_prohibited_text(
            &format!("evidence_inputs[{position}].summary"),
            &evidence_entry.summary,
        )?;
        if let Some(strength) = evidence_entry.signal_strength
            && (!strength.is_finite() || !(0.0..=1.0).contains(&strength))
        {
            return Err(MizanRejection::InvalidInput {
                reason: format!(
                    "evidence_inputs[{position}].signal_strength must be a finite number in 0.0..=1.0"
                ),
            });
        }
    }

    let evidence_ids: Vec<String> = request
        .evidence_inputs
        .iter()
        .map(|evidence_entry| evidence_entry.evidence_id.clone())
        .collect();

    let disclaimer = "Non-normative advisory diagnostic only (ADR 0005 / mizan-boundary-v1). Does not represent moral worth, spiritual rank, salvation status, divine judgment, or legal liability, and carries zero autonomous execution authority.".to_owned();

    // Gate 4 (FM-02 / F1): Missing evidence must emit insufficient_evidence (never coerced to 0 or safe).
    if request.evidence_inputs.is_empty() {
        return Ok(MizanDiagnostic {
            evaluation_id: Uuid::new_v4().to_string(),
            subject_id: request.subject_id.clone(),
            status: MizanStatus::InsufficientEvidence,
            advisory_only: true,
            normative_verdicts_allowed: false,
            autonomous_execution_authority: false,
            human_review_required: true,
            policy_version: POLICY_VERSION.to_owned(),
            profile_id: PROFILE_ID.to_owned(),
            adr_reference: ADR_REFERENCE.to_owned(),
            advisory_score: None,
            conflict_preserved: false,
            uncertainty_reasons: vec!["no_evidence_supplied".to_owned()],
            evidence_ids,
            conflicting_evidence_ids: Vec::new(),
            disclaimer,
            evaluated_at: Utc::now(),
        });
    }

    let mut uncertainty_reasons: Vec<String> = Vec::new();
    let mut conflicting_evidence_ids: Vec<String> = Vec::new();
    let mut has_missing_evidence = false;
    let mut has_unreviewed_evidence = false;
    let mut has_explicit_conflict = false;
    let mut supporting_ids: Vec<String> = Vec::new();
    let mut counter_ids: Vec<String> = Vec::new();

    for evidence_entry in &request.evidence_inputs {
        match evidence_entry.review_state {
            EvidenceReviewState::Missing => {
                has_missing_evidence = true;
            }
            EvidenceReviewState::Unreviewed => {
                has_unreviewed_evidence = true;
            }
            EvidenceReviewState::Conflicting => {
                has_explicit_conflict = true;
                conflicting_evidence_ids.push(evidence_entry.evidence_id.clone());
            }
            EvidenceReviewState::VerifiedHumanReviewed => {}
        }

        if evidence_entry.signal_strength.is_none() {
            has_missing_evidence = true;
        }

        match evidence_entry.stance {
            EvidenceStance::Supporting => supporting_ids.push(evidence_entry.evidence_id.clone()),
            EvidenceStance::Counter => counter_ids.push(evidence_entry.evidence_id.clone()),
            EvidenceStance::Indeterminate => {
                has_unreviewed_evidence = true;
            }
            EvidenceStance::Neutral => {}
        }
    }

    // Gate 5 (F2): Opposing supporting & counter evidence preserves conflict explicitly as unknown.
    let has_stance_conflict = !supporting_ids.is_empty() && !counter_ids.is_empty();
    if has_stance_conflict {
        has_explicit_conflict = true;
        for id in supporting_ids.iter().chain(counter_ids.iter()) {
            if !conflicting_evidence_ids.contains(id) {
                conflicting_evidence_ids.push(id.clone());
            }
        }
    }

    if has_explicit_conflict {
        uncertainty_reasons.push("conflicting_evidence_preserved_without_averaging".to_owned());
    }
    if has_unreviewed_evidence {
        uncertainty_reasons.push("unreviewed_or_indeterminate_evidence_present".to_owned());
    }
    if has_missing_evidence {
        uncertainty_reasons.push("missing_evidence_or_signal_not_coerced_to_zero".to_owned());
    }

    if has_explicit_conflict || has_unreviewed_evidence {
        return Ok(MizanDiagnostic {
            evaluation_id: Uuid::new_v4().to_string(),
            subject_id: request.subject_id.clone(),
            status: MizanStatus::Unknown,
            advisory_only: true,
            normative_verdicts_allowed: false,
            autonomous_execution_authority: false,
            human_review_required: true,
            policy_version: POLICY_VERSION.to_owned(),
            profile_id: PROFILE_ID.to_owned(),
            adr_reference: ADR_REFERENCE.to_owned(),
            advisory_score: None,
            conflict_preserved: has_explicit_conflict,
            uncertainty_reasons,
            evidence_ids,
            conflicting_evidence_ids,
            disclaimer,
            evaluated_at: Utc::now(),
        });
    }

    if has_missing_evidence {
        return Ok(MizanDiagnostic {
            evaluation_id: Uuid::new_v4().to_string(),
            subject_id: request.subject_id.clone(),
            status: MizanStatus::InsufficientEvidence,
            advisory_only: true,
            normative_verdicts_allowed: false,
            autonomous_execution_authority: false,
            human_review_required: true,
            policy_version: POLICY_VERSION.to_owned(),
            profile_id: PROFILE_ID.to_owned(),
            adr_reference: ADR_REFERENCE.to_owned(),
            advisory_score: None,
            conflict_preserved: false,
            uncertainty_reasons,
            evidence_ids,
            conflicting_evidence_ids: Vec::new(),
            disclaimer,
            evaluated_at: Utc::now(),
        });
    }

    // All inputs are human-reviewed, non-conflicting, and carry explicit bounded signals.
    let signal_sum: f64 = request
        .evidence_inputs
        .iter()
        .filter_map(|evidence_entry| evidence_entry.signal_strength)
        .sum();
    let count_f64 = request.evidence_inputs.len() as f64;
    let bounded_mean = ((signal_sum / count_f64).clamp(0.0, 1.0) * 10_000.0).round() / 10_000.0;

    Ok(MizanDiagnostic {
        evaluation_id: Uuid::new_v4().to_string(),
        subject_id: request.subject_id.clone(),
        status: MizanStatus::AdvisoryNonNormative,
        advisory_only: true,
        normative_verdicts_allowed: false,
        autonomous_execution_authority: false,
        human_review_required: true,
        policy_version: POLICY_VERSION.to_owned(),
        profile_id: PROFILE_ID.to_owned(),
        adr_reference: ADR_REFERENCE.to_owned(),
        advisory_score: Some(bounded_mean),
        conflict_preserved: false,
        uncertainty_reasons: Vec::new(),
        evidence_ids,
        conflicting_evidence_ids: Vec::new(),
        disclaimer,
        evaluated_at: Utc::now(),
    })
}

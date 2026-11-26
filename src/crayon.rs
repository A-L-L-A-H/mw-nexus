use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const CRAYON_PROFILE_ID: &str = "mw:profile:crayon-v1.3";
pub const CRAYON_VERSION: &str = "1.3";
pub const RGBL_PROFILE_ID: &str = "mw:profile:omniscience-rgbl-v1.0";
pub const RGBL_VERSION: &str = "1.0";

/// Cumulative level in CRAYON Inquiry Profile v1.3 (distinct from OMNISCIENCE RGBL).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrayonLevel {
    pub code: String,
    pub label: String,
    pub ordinal: u8,
    pub clock_position: String,
    pub cumulative_formula: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrayonGate {
    pub ordinal: u8,
    pub code: String,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrayonWay {
    pub ordinal: u8,
    pub code: String,
}

/// Versioned CRAYON Inquiry Profile v1.3.
/// Stored as a strictly separate type from `OmniscienceRgblProfile` to enforce
/// compile-time and runtime non-merge invariants.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrayonInquiryProfile {
    pub profile_id: String,
    pub framework_id: String,
    pub version: String,
    pub source_reference: String,
    pub classification: String,
    pub merge_permitted_with_rgbl: bool,
    pub distinct_from_profile_id: String,
    pub non_merge_rationale: String,
    pub cumulative_levels: Vec<CrayonLevel>,
    pub clock_metaphor_disclaimer: String,
    pub nine_gates: Vec<CrayonGate>,
    pub thirteen_ways: Vec<CrayonWay>,
    pub audience_conditions: Vec<String>,
    pub known_version_variation_note: String,
}

/// Epistemic stage in OMNISCIENCE RGBL v1.0 (distinct from CRAYON v1.3).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RgblEpistemicLayer {
    pub code: String,
    pub label: String,
    pub ordinal_stage: u8,
    pub guiding_question: String,
    pub structured_output: String,
}

/// Versioned OMNISCIENCE RGBL Epistemic Profile v1.0.
/// Strictly separate from `CrayonInquiryProfile` (G=Guidance, B=Balance vs CRAYON G2=GOVERNING ORDER, B3=BEING).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OmniscienceRgblProfile {
    pub profile_id: String,
    pub framework_id: String,
    pub version: String,
    pub source_reference: String,
    pub classification: String,
    pub merge_permitted_with_crayon: bool,
    pub distinct_from_profile_id: String,
    pub non_merge_rationale: String,
    pub ordinal_stage_rule: String,
    pub epistemic_layers: Vec<RgblEpistemicLayer>,
    pub minimum_provenance_chain: Vec<String>,
    pub operational_invariants: Vec<String>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ProfileSeparationViolation {
    #[error("non-merge invariant violated: CRAYON ({crayon_id}) and OMNISCIENCE RGBL ({rgbl_id}) cannot be merged because G/B semantics differ")]
    SilentMergeProhibited { crayon_id: String, rgbl_id: String },

    #[error("profile separation check failed: {reason}")]
    InvariantDrift { reason: String },
}

pub fn crayon_profile_v1_3() -> CrayonInquiryProfile {
    CrayonInquiryProfile {
        profile_id: CRAYON_PROFILE_ID.to_owned(),
        framework_id: "FRM-0015".to_owned(),
        version: CRAYON_VERSION.to_owned(),
        source_reference: "CRAYON_Framework_v1.3_Bilingual_FINAL.docx".to_owned(),
        classification: "proposed_conceptual_and_educational_framework".to_owned(),
        merge_permitted_with_rgbl: false,
        distinct_from_profile_id: RGBL_PROFILE_ID.to_owned(),
        non_merge_rationale: "CRAYON v1.3 defines G2=GOVERNING ORDER and B3=BEING, whereas OMNISCIENCE RGBL defines G=Guidance and B=Balance. Silent framework merger is strictly prohibited.".to_owned(),
        cumulative_levels: vec![
            CrayonLevel {
                code: "R1".to_owned(),
                label: "REALITY".to_owned(),
                ordinal: 1,
                clock_position: "12:00".to_owned(),
                cumulative_formula: "R1 = Reality".to_owned(),
                description: "What exists and can be observed.".to_owned(),
            },
            CrayonLevel {
                code: "G2".to_owned(),
                label: "GOVERNING ORDER".to_owned(),
                ordinal: 2,
                clock_position: "03:00".to_owned(),
                cumulative_formula: "G2 = R1 + Governing Order".to_owned(),
                description: "Patterns, relationships, regularities, and rules in reality.".to_owned(),
            },
            CrayonLevel {
                code: "B3".to_owned(),
                label: "BEING".to_owned(),
                ordinal: 3,
                clock_position: "06:00".to_owned(),
                cumulative_formula: "B3 = R1 + G2 + Being".to_owned(),
                description: "Existence, life, mind, consciousness, agency, and human experience.".to_owned(),
            },
            CrayonLevel {
                code: "L4".to_owned(),
                label: "LIGHT".to_owned(),
                ordinal: 4,
                clock_position: "09:00".to_owned(),
                cumulative_formula: "L4 = R1 + G2 + B3 + Integration/Insight".to_owned(),
                description: "Integrated understanding through reasoning and interpretation.".to_owned(),
            },
        ],
        clock_metaphor_disclaimer: "Clock positions (12:00 / 03:00 / 06:00 / 09:00) are a teaching metaphor only, not an evidence system, scientific measure, or religious symbol.".to_owned(),
        nine_gates: vec![
            CrayonGate { ordinal: 1, code: "SEE".to_owned(), category: "sensory_intake".to_owned() },
            CrayonGate { ordinal: 2, code: "HEAR".to_owned(), category: "sensory_intake".to_owned() },
            CrayonGate { ordinal: 3, code: "TOUCH".to_owned(), category: "sensory_intake".to_owned() },
            CrayonGate { ordinal: 4, code: "TASTE".to_owned(), category: "sensory_intake".to_owned() },
            CrayonGate { ordinal: 5, code: "SMELL".to_owned(), category: "sensory_intake".to_owned() },
            CrayonGate { ordinal: 6, code: "ATTEND".to_owned(), category: "cognitive_processing".to_owned() },
            CrayonGate { ordinal: 7, code: "REMEMBER".to_owned(), category: "cognitive_processing".to_owned() },
            CrayonGate { ordinal: 8, code: "REASON".to_owned(), category: "cognitive_processing".to_owned() },
            CrayonGate { ordinal: 9, code: "CHOOSE".to_owned(), category: "cognitive_processing".to_owned() },
        ],
        thirteen_ways: vec![
            CrayonWay { ordinal: 1, code: "OBSERVE".to_owned() },
            CrayonWay { ordinal: 2, code: "NOTICE".to_owned() },
            CrayonWay { ordinal: 3, code: "QUESTION".to_owned() },
            CrayonWay { ordinal: 4, code: "MEASURE".to_owned() },
            CrayonWay { ordinal: 5, code: "COMPARE".to_owned() },
            CrayonWay { ordinal: 6, code: "EXPLORE".to_owned() },
            CrayonWay { ordinal: 7, code: "UNDERSTAND".to_owned() },
            CrayonWay { ordinal: 8, code: "CONNECT".to_owned() },
            CrayonWay { ordinal: 9, code: "EVALUATE".to_owned() },
            CrayonWay { ordinal: 10, code: "REASON".to_owned() },
            CrayonWay { ordinal: 11, code: "CONCLUDE".to_owned() },
            CrayonWay { ordinal: 12, code: "ACT".to_owned() },
            CrayonWay { ordinal: 13, code: "LIVE".to_owned() },
        ],
        audience_conditions: vec![
            "KNOWLEDGE".to_owned(),
            "RELEVANCE".to_owned(),
            "CONFIDENCE".to_owned(),
            "TRUST_RESISTANCE".to_owned(),
        ],
        known_version_variation_note: "CRAYON v1.3 uses Gates 6-9 ATTEND/REMEMBER/REASON/CHOOSE and 13 Ways with REASON before CONCLUDE; companion v2 differs and must remain version-distinct without silent rewriting.".to_owned(),
    }
}

pub fn omniscience_rgbl_profile_v1_0() -> OmniscienceRgblProfile {
    OmniscienceRgblProfile {
        profile_id: RGBL_PROFILE_ID.to_owned(),
        framework_id: "FRM-0001".to_owned(),
        version: RGBL_VERSION.to_owned(),
        source_reference: "Omniscience_Method_RGBL_AI_Ready_Research_Paper.md".to_owned(),
        classification: "epistemic_research_framework".to_owned(),
        merge_permitted_with_crayon: false,
        distinct_from_profile_id: CRAYON_PROFILE_ID.to_owned(),
        non_merge_rationale: "OMNISCIENCE RGBL defines G=Guidance and B=Balance as epistemic method stages, distinct from CRAYON's G2=GOVERNING ORDER and B3=BEING and distinct from RGB color imagery.".to_owned(),
        ordinal_stage_rule: "Stage numbers 1-4 are ordinal stage identifiers only, never probabilities, confidence scores, or truth scores.".to_owned(),
        epistemic_layers: vec![
            RgblEpistemicLayer {
                code: "R".to_owned(),
                label: "Reality".to_owned(),
                ordinal_stage: 1,
                guiding_question: "What actually happened?".to_owned(),
                structured_output: "Reality Set: observations, records, claims, and source evidence".to_owned(),
            },
            RgblEpistemicLayer {
                code: "G".to_owned(),
                label: "Guidance".to_owned(),
                ordinal_stage: 2,
                guiding_question: "What should guide interpretation?".to_owned(),
                structured_output: "Guidance Set: sourced/versioned principles, rules, texts, and values".to_owned(),
            },
            RgblEpistemicLayer {
                code: "B".to_owned(),
                label: "Balance".to_owned(),
                ordinal_stage: 3,
                guiding_question: "How should facts and guidance be weighed in context?".to_owned(),
                structured_output: "Balanced Assessment: competing evidence, uncertainty, alternatives, and consequences".to_owned(),
            },
            RgblEpistemicLayer {
                code: "L".to_owned(),
                label: "Light".to_owned(),
                ordinal_stage: 4,
                guiding_question: "What becomes clear after the preceding steps?".to_owned(),
                structured_output: "Traceable synthesis / insight with explicit assumptions and provenance".to_owned(),
            },
        ],
        minimum_provenance_chain: vec![
            "Light claim".to_owned(),
            "Balance record".to_owned(),
            "Reality items + Guidance items".to_owned(),
        ],
        operational_invariants: vec![
            "Reported, uncertain, or disputed facts must never silently become verified.".to_owned(),
            "Guidance source, version, and scope remain explicit.".to_owned(),
            "Balance preserves conflict, counter-evidence, bias checks, and alternatives.".to_owned(),
            "Human review remains mandatory; AI output is not divine revelation or infallible authority.".to_owned(),
        ],
    }
}

/// Verifies at runtime that CRAYON v1.3 and OMNISCIENCE RGBL remain strictly separated.
pub fn assert_strict_profile_separation(
    crayon: &CrayonInquiryProfile,
    rgbl: &OmniscienceRgblProfile,
) -> Result<(), ProfileSeparationViolation> {
    if crayon.profile_id == rgbl.profile_id {
        return Err(ProfileSeparationViolation::InvariantDrift {
            reason: "CRAYON and RGBL share the same profile_id".to_owned(),
        });
    }
    if crayon.merge_permitted_with_rgbl || rgbl.merge_permitted_with_crayon {
        return Err(ProfileSeparationViolation::SilentMergeProhibited {
            crayon_id: crayon.profile_id.clone(),
            rgbl_id: rgbl.profile_id.clone(),
        });
    }

    let crayon_g = crayon.cumulative_levels.get(1).map(|lvl| lvl.label.as_str());
    let crayon_b = crayon.cumulative_levels.get(2).map(|lvl| lvl.label.as_str());
    let rgbl_g = rgbl.epistemic_layers.get(1).map(|layer| layer.label.as_str());
    let rgbl_b = rgbl.epistemic_layers.get(2).map(|layer| layer.label.as_str());

    if crayon_g != Some("GOVERNING ORDER")
        || crayon_b != Some("BEING")
        || rgbl_g != Some("Guidance")
        || rgbl_b != Some("Balance")
    {
        return Err(ProfileSeparationViolation::InvariantDrift {
            reason: "G/B semantic separation between CRAYON (GOVERNING ORDER / BEING) and RGBL (Guidance / Balance) was altered".to_owned(),
        });
    }

    Ok(())
}

/// Guardrail function rejecting any runtime attempt to merge CRAYON and OMNISCIENCE RGBL profiles.
pub fn reject_profile_merge(
    left_profile_id: &str,
    right_profile_id: &str,
) -> Result<(), ProfileSeparationViolation> {
    let left_lower = left_profile_id.trim().to_ascii_lowercase();
    let right_lower = right_profile_id.trim().to_ascii_lowercase();
    let involves_crayon = left_lower.contains("crayon") || right_lower.contains("crayon");
    let involves_rgbl = left_lower.contains("rgbl") || right_lower.contains("rgbl");

    if involves_crayon && involves_rgbl {
        return Err(ProfileSeparationViolation::SilentMergeProhibited {
            crayon_id: CRAYON_PROFILE_ID.to_owned(),
            rgbl_id: RGBL_PROFILE_ID.to_owned(),
        });
    }

    Ok(())
}

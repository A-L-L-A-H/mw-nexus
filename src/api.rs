use crate::crayon::{
    CrayonInquiryProfile, OmniscienceRgblProfile, assert_strict_profile_separation,
    crayon_profile_v1_3, omniscience_rgbl_profile_v1_0,
};
use crate::domains::{DomainMetadata, canonical_domains, permitted_relation_kinds};
use crate::graph::{
    CorrelationListFilter, CreateCorrelationRequest, CreateNodeRequest, GraphError,
    NexusGraphStore, NodeListFilter, OntologyEdge, OntologyNode, RadarIngestRequest,
    RadarIngestResponse, SubgraphQuery, SubgraphResponse,
};
use crate::mizan::{
    ADR_REFERENCE, MizanDiagnostic, MizanEvaluationRequest, MizanRejection, POLICY_VERSION,
    evaluate_mizan,
};
use crate::mtime::{MTIME_PROFILE, MTimeConversion, MTimeError, convert_ts_param};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct AppState {
    pub store: NexusGraphStore,
    pub crayon_profile: CrayonInquiryProfile,
    pub rgbl_profile: OmniscienceRgblProfile,
}

impl AppState {
    pub fn new(store: NexusGraphStore) -> Self {
        let crayon_profile = crayon_profile_v1_3();
        let rgbl_profile = omniscience_rgbl_profile_v1_0();
        assert_strict_profile_separation(&crayon_profile, &rgbl_profile)
            .expect("canonical CRAYON and RGBL profiles must remain strictly separated");
        Self {
            store,
            crayon_profile,
            rgbl_profile,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ApiErrorResponse {
    pub error_code: String,
    pub message: String,
    pub policy_reference: String,
}

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub error_code: String,
    pub message: String,
    pub policy_reference: String,
}

impl ApiError {
    fn new(
        status: StatusCode,
        error_code: impl Into<String>,
        message: impl Into<String>,
        policy_reference: impl Into<String>,
    ) -> Self {
        Self {
            status,
            error_code: error_code.into(),
            message: message.into(),
            policy_reference: policy_reference.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ApiErrorResponse {
            error_code: self.error_code,
            message: self.message,
            policy_reference: self.policy_reference,
        };
        (self.status, Json(body)).into_response()
    }
}

impl From<GraphError> for ApiError {
    fn from(graph_err: GraphError) -> Self {
        match graph_err {
            GraphError::Boundary(violation) => ApiError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                violation.error_code(),
                violation.to_string(),
                "7-domain-boundary-v1",
            ),
            GraphError::NodeNotFound { node_id } => ApiError::new(
                StatusCode::NOT_FOUND,
                "NODE_NOT_FOUND",
                format!("ontology node '{node_id}' not found"),
                "ontology-graph-v1",
            ),
            GraphError::DuplicateNodeId { node_id } => ApiError::new(
                StatusCode::CONFLICT,
                "DUPLICATE_NODE_ID",
                format!("ontology node '{node_id}' already exists"),
                "ontology-graph-v1",
            ),
            GraphError::ImmutableCorpusRefViolation { node_id } => ApiError::new(
                StatusCode::CONFLICT,
                "IMMUTABLE_CORPUS_REF",
                format!("corpus_ref is immutable on node '{node_id}'"),
                "provenance-immutability-v1",
            ),
            GraphError::ChildSensitiveBlocked => ApiError::new(
                StatusCode::FORBIDDEN,
                "CHILD_SENSITIVE_BLOCKED",
                "fail-closed safeguarding block: CHILD_SENSITIVE records cannot be exposed or correlated via general routes",
                ADR_REFERENCE,
            ),
            GraphError::InvalidRequest { reason } => ApiError::new(
                StatusCode::BAD_REQUEST,
                "INVALID_GRAPH_REQUEST",
                reason,
                "ontology-graph-v1",
            ),
            GraphError::Sqlite(sqlite_err) => ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "STORAGE_ERROR",
                sqlite_err.to_string(),
                "ontology-graph-v1",
            ),
            GraphError::Serialization(serde_err) => ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "SERIALIZATION_ERROR",
                serde_err.to_string(),
                "ontology-graph-v1",
            ),
            GraphError::LockPoisoned => ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "LOCK_POISONED",
                "internal storage lock unavailable",
                "ontology-graph-v1",
            ),
        }
    }
}

impl From<MizanRejection> for ApiError {
    fn from(rejection: MizanRejection) -> Self {
        let status = match rejection {
            MizanRejection::ChildSensitiveBlocked => StatusCode::FORBIDDEN,
            MizanRejection::InvalidInput { .. } => StatusCode::BAD_REQUEST,
            MizanRejection::NormativeBoundaryViolation { .. }
            | MizanRejection::AutonomousActionProhibited { .. }
            | MizanRejection::LegacyBlueprintFactorRejected { .. } => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
        };
        ApiError::new(status, rejection.error_code(), rejection.to_string(), POLICY_VERSION)
    }
}

impl From<MTimeError> for ApiError {
    fn from(err: MTimeError) -> Self {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "INVALID_TIMESTAMP",
            err.to_string(),
            MTIME_PROFILE,
        )
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct TimeConvertQuery {
    pub ts: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
    pub journal_mode: String,
    pub domains_count: usize,
    pub policy_version: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct DomainsCatalogResponse {
    pub domain_count: usize,
    pub domains: Vec<DomainMetadata>,
    pub permitted_relations: Vec<&'static str>,
    pub correlation_boundary_rule: String,
}

#[derive(Debug, Serialize)]
pub struct NodesListResponse {
    pub count: usize,
    pub alias_auto_merge_enabled: bool,
    pub nodes: Vec<OntologyNode>,
}

#[derive(Debug, Serialize)]
pub struct CorrelationsListResponse {
    pub count: usize,
    pub correlations: Vec<OntologyEdge>,
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/api/domains", get(domains_handler))
        .route("/api/nodes", get(list_nodes_handler).post(create_node_handler))
        .route("/api/nodes/{node_id}", get(get_node_handler))
        .route(
            "/api/correlations",
            get(list_correlations_handler).post(create_correlation_handler),
        )
        .route("/api/graph", get(query_graph_handler))
        .route("/api/mizan/evaluate", post(evaluate_mizan_handler))
        .route("/api/profiles/crayon", get(crayon_profile_handler))
        .route("/api/profiles/rgbl", get(rgbl_profile_handler))
        .route("/api/time/convert", get(time_convert_handler))
        .route("/api/radar/ingest", post(radar_ingest_handler))
        .with_state(state)
}

async fn health_handler(State(state): State<AppState>) -> Result<Json<HealthResponse>, ApiError> {
    let journal_mode = state.store.journal_mode()?;
    Ok(Json(HealthResponse {
        status: "ok".to_owned(),
        service: "mw-nexus".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        journal_mode,
        domains_count: 7,
        policy_version: POLICY_VERSION.to_owned(),
        timestamp: Utc::now().to_rfc3339(),
    }))
}

async fn domains_handler() -> Json<DomainsCatalogResponse> {
    let domains = canonical_domains();
    Json(DomainsCatalogResponse {
        domain_count: domains.len(),
        domains,
        permitted_relations: permitted_relation_kinds(),
        correlation_boundary_rule: "Correlation is strictly NOT causation, NOT identity equivalence, NOT prophetic fulfillment, and NOT a normative verdict.".to_owned(),
    })
}

async fn list_nodes_handler(
    State(state): State<AppState>,
    Query(filter): Query<NodeListFilter>,
) -> Result<Json<NodesListResponse>, ApiError> {
    let nodes = state.store.list_nodes(filter)?;
    Ok(Json(NodesListResponse {
        count: nodes.len(),
        alias_auto_merge_enabled: false,
        nodes,
    }))
}

async fn get_node_handler(
    State(state): State<AppState>,
    Path(node_id): Path<String>,
) -> Result<Json<OntologyNode>, ApiError> {
    let node = state.store.get_node(&node_id)?;
    Ok(Json(node))
}

async fn create_node_handler(
    State(state): State<AppState>,
    Json(payload): Json<CreateNodeRequest>,
) -> Result<(StatusCode, Json<OntologyNode>), ApiError> {
    let created_node = state.store.create_node(payload)?;
    Ok((StatusCode::CREATED, Json(created_node)))
}

async fn list_correlations_handler(
    State(state): State<AppState>,
    Query(filter): Query<CorrelationListFilter>,
) -> Result<Json<CorrelationsListResponse>, ApiError> {
    let correlations = state.store.list_correlations(filter)?;
    Ok(Json(CorrelationsListResponse {
        count: correlations.len(),
        correlations,
    }))
}

async fn create_correlation_handler(
    State(state): State<AppState>,
    Json(payload): Json<CreateCorrelationRequest>,
) -> Result<(StatusCode, Json<OntologyEdge>), ApiError> {
    let created_edge = state.store.create_correlation(payload)?;
    Ok((StatusCode::CREATED, Json(created_edge)))
}

async fn query_graph_handler(
    State(state): State<AppState>,
    Query(query): Query<SubgraphQuery>,
) -> Result<Json<SubgraphResponse>, ApiError> {
    let subgraph = state.store.query_subgraph(query)?;
    Ok(Json(subgraph))
}

async fn evaluate_mizan_handler(
    Json(payload): Json<MizanEvaluationRequest>,
) -> Result<Json<MizanDiagnostic>, ApiError> {
    let diagnostic = evaluate_mizan(&payload)?;
    Ok(Json(diagnostic))
}

async fn crayon_profile_handler(State(state): State<AppState>) -> Json<CrayonInquiryProfile> {
    Json(state.crayon_profile.clone())
}

async fn rgbl_profile_handler(State(state): State<AppState>) -> Json<OmniscienceRgblProfile> {
    Json(state.rgbl_profile.clone())
}

async fn time_convert_handler(
    Query(query): Query<TimeConvertQuery>,
) -> Result<Json<MTimeConversion>, ApiError> {
    let conversion = convert_ts_param(query.ts.as_deref())?;
    Ok(Json(conversion))
}

async fn radar_ingest_handler(
    State(state): State<AppState>,
    Json(payload): Json<RadarIngestRequest>,
) -> Result<(StatusCode, Json<RadarIngestResponse>), ApiError> {
    let response = state.store.ingest_radar_event(payload)?;
    let status_code = if response.status == "created" {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status_code, Json(response)))
}

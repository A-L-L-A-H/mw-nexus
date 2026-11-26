use crate::domains::{
    BoundaryViolation, ProvenanceDomain, RelationKind, ReviewState, UncertaintyState,
    parse_relation_kind, validate_cross_domain_boundary, validate_evidence_hash,
};
use crate::mizan::SensitivityClass;
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum GraphError {
    #[error("sqlite storage error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("json serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("boundary validation error: {0}")]
    Boundary(#[from] BoundaryViolation),

    #[error("ontology node '{node_id}' not found")]
    NodeNotFound { node_id: String },

    #[error("ontology node '{node_id}' already exists")]
    DuplicateNodeId { node_id: String },

    #[error("immutable corpus_ref violation on node '{node_id}': corpus_ref cannot be mutated after creation")]
    ImmutableCorpusRefViolation { node_id: String },

    #[error("fail-closed safeguarding block: CHILD_SENSITIVE nodes cannot be exposed or linked via general graph routes")]
    ChildSensitiveBlocked,

    #[error("invalid graph request: {reason}")]
    InvalidRequest { reason: String },

    #[error("internal store lock poisoned")]
    LockPoisoned,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OntologyNode {
    pub id: String,
    pub domain: ProvenanceDomain,
    pub canonical_label: String,
    pub corpus_ref: String,
    pub uncertainty_state: UncertaintyState,
    pub alias_set: Vec<String>,
    pub alias_set_json: String,
    pub sensitivity_class: SensitivityClass,
    pub metadata: serde_json::Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateNodeRequest {
    #[serde(default)]
    pub id: Option<String>,
    pub domain: ProvenanceDomain,
    pub canonical_label: String,
    pub corpus_ref: String,
    #[serde(default)]
    pub uncertainty_state: Option<UncertaintyState>,
    #[serde(default)]
    pub alias_set: Option<Vec<String>>,
    #[serde(default)]
    pub sensitivity_class: Option<SensitivityClass>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OntologyEdge {
    pub id: String,
    pub source_node_id: String,
    pub target_node_id: String,
    pub source_domain: ProvenanceDomain,
    pub target_domain: ProvenanceDomain,
    pub relation_kind: RelationKind,
    pub evidence_hash: String,
    pub evidence_locator: String,
    pub review_state: ReviewState,
    pub uncertainty_note: String,
    pub implies_causation: bool,
    pub implies_identity: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCorrelationRequest {
    pub source_node_id: String,
    pub target_node_id: String,
    pub relation_kind: String,
    pub evidence_hash: String,
    pub evidence_locator: String,
    #[serde(default)]
    pub review_state: Option<ReviewState>,
    #[serde(default)]
    pub uncertainty_note: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct NodeListFilter {
    pub domain: Option<ProvenanceDomain>,
    pub uncertainty_state: Option<UncertaintyState>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CorrelationListFilter {
    pub relation_kind: Option<String>,
    pub review_state: Option<ReviewState>,
    pub node_id: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SubgraphQuery {
    pub root_node_id: Option<String>,
    pub depth: Option<usize>,
    pub domain: Option<ProvenanceDomain>,
    pub review_state: Option<ReviewState>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubgraphResponse {
    pub root_node_id: Option<String>,
    pub traversal_depth: usize,
    pub node_count: usize,
    pub edge_count: usize,
    pub nodes: Vec<OntologyNode>,
    pub edges: Vec<OntologyEdge>,
    pub alias_auto_merge_enabled: bool,
    pub boundary_notice: String,
}

/// Computes a canonical SHA-256 hex digest for evidence payloads.
pub fn compute_evidence_hash(evidence_payload: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(evidence_payload.as_bytes());
    hex::encode(hasher.finalize())
}

#[derive(Clone)]
pub struct NexusGraphStore {
    connection: Arc<Mutex<Connection>>,
}

impl NexusGraphStore {
    pub fn open(db_path: &Path) -> Result<Self, GraphError> {
        if let Some(parent_dir) = db_path.parent()
            && !parent_dir.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent_dir).map_err(|io_err| GraphError::InvalidRequest {
                reason: format!("unable to create database directory: {io_err}"),
            })?;
        }
        let conn = Connection::open(db_path)?;
        Self::initialize_connection(conn)
    }

    pub fn open_in_memory() -> Result<Self, GraphError> {
        let conn = Connection::open_in_memory()?;
        Self::initialize_connection(conn)
    }

    fn initialize_connection(conn: Connection) -> Result<Self, GraphError> {
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA foreign_keys = ON;
            PRAGMA synchronous = NORMAL;
            PRAGMA busy_timeout = 5000;

            CREATE TABLE IF NOT EXISTS ontology_nodes (
                id TEXT PRIMARY KEY,
                domain TEXT NOT NULL,
                canonical_label TEXT NOT NULL,
                corpus_ref TEXT NOT NULL,
                uncertainty_state TEXT NOT NULL,
                alias_set_json TEXT NOT NULL,
                sensitivity_class TEXT NOT NULL DEFAULT 'INTERNAL',
                metadata_json TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL
            );

            CREATE TRIGGER IF NOT EXISTS trg_ontology_nodes_immutable_corpus_ref
            BEFORE UPDATE OF corpus_ref ON ontology_nodes
            WHEN NEW.corpus_ref != OLD.corpus_ref
            BEGIN
                SELECT RAISE(ABORT, 'IMMUTABLE_CORPUS_REF');
            END;

            CREATE TABLE IF NOT EXISTS ontology_edges (
                id TEXT PRIMARY KEY,
                source_node_id TEXT NOT NULL,
                target_node_id TEXT NOT NULL,
                source_domain TEXT NOT NULL,
                target_domain TEXT NOT NULL,
                relation_kind TEXT NOT NULL,
                evidence_hash TEXT NOT NULL,
                evidence_locator TEXT NOT NULL,
                review_state TEXT NOT NULL,
                uncertainty_note TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                FOREIGN KEY (source_node_id) REFERENCES ontology_nodes(id) ON DELETE RESTRICT,
                FOREIGN KEY (target_node_id) REFERENCES ontology_nodes(id) ON DELETE RESTRICT
            );

            CREATE INDEX IF NOT EXISTS idx_ontology_nodes_domain ON ontology_nodes(domain);
            CREATE INDEX IF NOT EXISTS idx_ontology_nodes_uncertainty ON ontology_nodes(uncertainty_state);
            CREATE INDEX IF NOT EXISTS idx_ontology_edges_source ON ontology_edges(source_node_id);
            CREATE INDEX IF NOT EXISTS idx_ontology_edges_target ON ontology_edges(target_node_id);
            CREATE INDEX IF NOT EXISTS idx_ontology_edges_relation ON ontology_edges(relation_kind);
            ",
        )?;

        Ok(Self {
            connection: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn journal_mode(&self) -> Result<String, GraphError> {
        let guard = self.connection.lock().map_err(|_| GraphError::LockPoisoned)?;
        let mode: String = guard.query_row("PRAGMA journal_mode;", [], |row| row.get(0))?;
        Ok(mode)
    }

    /// Inserts a new ontology node with immutable `corpus_ref` and preserved `alias_set_json`.
    /// Strictly NEVER auto-merges with existing nodes that share labels or aliases.
    pub fn create_node(&self, request: CreateNodeRequest) -> Result<OntologyNode, GraphError> {
        let canonical_label = request.canonical_label.trim().to_owned();
        if canonical_label.is_empty() {
            return Err(GraphError::InvalidRequest {
                reason: "canonical_label must not be empty".to_owned(),
            });
        }

        let corpus_ref = request.corpus_ref.trim().to_owned();
        if corpus_ref.is_empty() {
            return Err(GraphError::InvalidRequest {
                reason: "corpus_ref must not be empty and is immutable once recorded".to_owned(),
            });
        }

        let node_id = match request.id {
            Some(supplied_id) => {
                let trimmed_id = supplied_id.trim().to_owned();
                if trimmed_id.is_empty() {
                    return Err(GraphError::InvalidRequest {
                        reason: "supplied node id must not be blank".to_owned(),
                    });
                }
                trimmed_id
            }
            None => Uuid::new_v4().to_string(),
        };

        let uncertainty_state = request.uncertainty_state.unwrap_or(UncertaintyState::Unknown);
        let sensitivity_class = request.sensitivity_class.unwrap_or(SensitivityClass::Internal);
        let cleaned_aliases = sanitize_alias_list(request.alias_set.unwrap_or_default());
        let alias_set_json = serde_json::to_string(&cleaned_aliases)?;
        let metadata_value = request.metadata.unwrap_or_else(|| serde_json::json!({}));
        let metadata_json = serde_json::to_string(&metadata_value)?;
        let created_at = Utc::now().to_rfc3339();

        let guard = self.connection.lock().map_err(|_| GraphError::LockPoisoned)?;
        let existing_id: Option<String> = guard
            .query_row(
                "SELECT id FROM ontology_nodes WHERE id = ?1",
                params![&node_id],
                |row| row.get(0),
            )
            .optional()?;

        if existing_id.is_some() {
            return Err(GraphError::DuplicateNodeId { node_id });
        }

        guard.execute(
            "INSERT INTO ontology_nodes (
                id, domain, canonical_label, corpus_ref, uncertainty_state,
                alias_set_json, sensitivity_class, metadata_json, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                &node_id,
                request.domain.code(),
                &canonical_label,
                &corpus_ref,
                uncertainty_state.as_str(),
                &alias_set_json,
                sensitivity_class.as_str(),
                &metadata_json,
                &created_at
            ],
        )?;

        Ok(OntologyNode {
            id: node_id,
            domain: request.domain,
            canonical_label,
            corpus_ref,
            uncertainty_state,
            alias_set: cleaned_aliases,
            alias_set_json,
            sensitivity_class,
            metadata: metadata_value,
            created_at,
        })
    }

    pub fn get_node(&self, node_id: &str) -> Result<OntologyNode, GraphError> {
        let guard = self.connection.lock().map_err(|_| GraphError::LockPoisoned)?;
        let fetched = load_node_by_id(&guard, node_id)?;
        if fetched.sensitivity_class == SensitivityClass::ChildSensitive {
            return Err(GraphError::ChildSensitiveBlocked);
        }
        Ok(fetched)
    }

    /// Appends aliases to a single existing node without modifying `corpus_ref`
    /// and without auto-merging with any other node sharing those aliases.
    pub fn append_node_aliases(
        &self,
        node_id: &str,
        additional_aliases: Vec<String>,
    ) -> Result<OntologyNode, GraphError> {
        let guard = self.connection.lock().map_err(|_| GraphError::LockPoisoned)?;
        let mut node = load_node_by_id(&guard, node_id)?;
        if node.sensitivity_class == SensitivityClass::ChildSensitive {
            return Err(GraphError::ChildSensitiveBlocked);
        }

        let mut combined_aliases = node.alias_set.clone();
        combined_aliases.extend(additional_aliases);
        let deduped_aliases = sanitize_alias_list(combined_aliases);
        let alias_set_json = serde_json::to_string(&deduped_aliases)?;

        guard.execute(
            "UPDATE ontology_nodes SET alias_set_json = ?1 WHERE id = ?2",
            params![&alias_set_json, node_id],
        )?;

        node.alias_set = deduped_aliases;
        node.alias_set_json = alias_set_json;
        Ok(node)
    }

    /// Explicitly attempts to update `corpus_ref` to verify database trigger enforcement.
    /// Always fails with `ImmutableCorpusRefViolation` when the value differs.
    pub fn attempt_mutate_corpus_ref(
        &self,
        node_id: &str,
        new_corpus_ref: &str,
    ) -> Result<(), GraphError> {
        let guard = self.connection.lock().map_err(|_| GraphError::LockPoisoned)?;
        let _existing = load_node_by_id(&guard, node_id)?;
        let update_outcome = guard.execute(
            "UPDATE ontology_nodes SET corpus_ref = ?1 WHERE id = ?2",
            params![new_corpus_ref, node_id],
        );
        match update_outcome {
            Ok(_) => Ok(()),
            Err(sqlite_err) => {
                if sqlite_err.to_string().contains("IMMUTABLE_CORPUS_REF") {
                    Err(GraphError::ImmutableCorpusRefViolation {
                        node_id: node_id.to_owned(),
                    })
                } else {
                    Err(GraphError::Sqlite(sqlite_err))
                }
            }
        }
    }

    pub fn list_nodes(&self, filter: NodeListFilter) -> Result<Vec<OntologyNode>, GraphError> {
        let guard = self.connection.lock().map_err(|_| GraphError::LockPoisoned)?;
        let limit = filter.limit.unwrap_or(100).clamp(1, 500) as i64;

        let mut statement = guard.prepare(
            "SELECT id, domain, canonical_label, corpus_ref, uncertainty_state,
                    alias_set_json, sensitivity_class, metadata_json, created_at
             FROM ontology_nodes
             WHERE sensitivity_class != 'CHILD_SENSITIVE'
               AND (?1 IS NULL OR domain = ?1)
               AND (?2 IS NULL OR uncertainty_state = ?2)
             ORDER BY created_at ASC, id ASC
             LIMIT ?3",
        )?;

        let domain_param = filter.domain.map(|domain| domain.code().to_owned());
        let uncertainty_param = filter
            .uncertainty_state
            .map(|state| state.as_str().to_owned());

        let mapped_rows = statement.query_map(
            params![domain_param, uncertainty_param, limit],
            row_to_ontology_node,
        )?;

        let mut nodes = Vec::new();
        for row_result in mapped_rows {
            nodes.push(row_result?);
        }
        Ok(nodes)
    }

    /// Creates a typed cross-domain correlation edge after validating all 7-domain
    /// boundary invariants (correlation != causation != identity != fulfillment).
    pub fn create_correlation(
        &self,
        request: CreateCorrelationRequest,
    ) -> Result<OntologyEdge, GraphError> {
        let evidence_locator = request.evidence_locator.trim().to_owned();
        if evidence_locator.is_empty() {
            return Err(GraphError::InvalidRequest {
                reason: "evidence_locator must not be empty".to_owned(),
            });
        }

        let guard = self.connection.lock().map_err(|_| GraphError::LockPoisoned)?;
        let source_node = load_node_by_id(&guard, &request.source_node_id)?;
        let target_node = load_node_by_id(&guard, &request.target_node_id)?;

        if source_node.sensitivity_class == SensitivityClass::ChildSensitive
            || target_node.sensitivity_class == SensitivityClass::ChildSensitive
        {
            return Err(GraphError::ChildSensitiveBlocked);
        }

        let validated_boundary = validate_cross_domain_boundary(
            &source_node.id,
            &target_node.id,
            source_node.domain,
            target_node.domain,
            &request.relation_kind,
            &request.evidence_hash,
        )?;
        let normalized_hash = validate_evidence_hash(&request.evidence_hash)?;
        let review_state = request.review_state.unwrap_or(ReviewState::Candidate);
        let uncertainty_note = request.uncertainty_note.unwrap_or_default().trim().to_owned();
        let edge_id = Uuid::new_v4().to_string();
        let created_at = Utc::now().to_rfc3339();

        guard.execute(
            "INSERT INTO ontology_edges (
                id, source_node_id, target_node_id, source_domain, target_domain,
                relation_kind, evidence_hash, evidence_locator, review_state,
                uncertainty_note, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                &edge_id,
                &source_node.id,
                &target_node.id,
                source_node.domain.code(),
                target_node.domain.code(),
                validated_boundary.relation_kind.as_str(),
                &normalized_hash,
                &evidence_locator,
                review_state.as_str(),
                &uncertainty_note,
                &created_at
            ],
        )?;

        Ok(OntologyEdge {
            id: edge_id,
            source_node_id: source_node.id,
            target_node_id: target_node.id,
            source_domain: source_node.domain,
            target_domain: target_node.domain,
            relation_kind: validated_boundary.relation_kind,
            evidence_hash: normalized_hash,
            evidence_locator,
            review_state,
            uncertainty_note,
            implies_causation: validated_boundary.implies_causation,
            implies_identity: validated_boundary.implies_identity,
            created_at,
        })
    }

    pub fn list_correlations(
        &self,
        filter: CorrelationListFilter,
    ) -> Result<Vec<OntologyEdge>, GraphError> {
        let relation_filter = match &filter.relation_kind {
            Some(raw_rel) => Some(parse_relation_kind(raw_rel)?.as_str().to_owned()),
            None => None,
        };
        let review_filter = filter.review_state.map(|state| state.as_str().to_owned());
        let node_filter = filter.node_id.map(|id| id.trim().to_owned());
        let limit = filter.limit.unwrap_or(100).clamp(1, 500) as i64;

        let guard = self.connection.lock().map_err(|_| GraphError::LockPoisoned)?;
        let mut statement = guard.prepare(
            "SELECT e.id, e.source_node_id, e.target_node_id, e.source_domain, e.target_domain,
                    e.relation_kind, e.evidence_hash, e.evidence_locator, e.review_state,
                    e.uncertainty_note, e.created_at
             FROM ontology_edges e
             INNER JOIN ontology_nodes src ON src.id = e.source_node_id
             INNER JOIN ontology_nodes dst ON dst.id = e.target_node_id
             WHERE src.sensitivity_class != 'CHILD_SENSITIVE'
               AND dst.sensitivity_class != 'CHILD_SENSITIVE'
               AND (?1 IS NULL OR e.relation_kind = ?1)
               AND (?2 IS NULL OR e.review_state = ?2)
               AND (?3 IS NULL OR e.source_node_id = ?3 OR e.target_node_id = ?3)
             ORDER BY e.created_at ASC, e.id ASC
             LIMIT ?4",
        )?;

        let mapped_rows = statement.query_map(
            params![relation_filter, review_filter, node_filter, limit],
            row_to_ontology_edge,
        )?;

        let mut edges = Vec::new();
        for row_result in mapped_rows {
            edges.push(row_result?);
        }
        Ok(edges)
    }

    /// Queries a bounded subgraph rooted at an optional node ID (BFS traversal)
    /// or filtered by domain/review_state, excluding CHILD_SENSITIVE records.
    pub fn query_subgraph(&self, query: SubgraphQuery) -> Result<SubgraphResponse, GraphError> {
        let max_depth = query.depth.unwrap_or(2).min(6);
        let limit = query.limit.unwrap_or(100).clamp(1, 500);
        let guard = self.connection.lock().map_err(|_| GraphError::LockPoisoned)?;

        let boundary_notice = "Subgraph edges represent typed provenance correlations only; correlation does not imply causation, identity equivalence, prophetic fulfillment, or alias auto-merging.".to_owned();

        if let Some(root_id) = query.root_node_id.clone() {
            let root_node = load_node_by_id(&guard, &root_id)?;
            if root_node.sensitivity_class == SensitivityClass::ChildSensitive {
                return Err(GraphError::ChildSensitiveBlocked);
            }

            let mut visited_nodes: HashMap<String, OntologyNode> = HashMap::new();
            let mut visited_edges: HashMap<String, OntologyEdge> = HashMap::new();
            let mut frontier: VecDeque<(String, usize)> = VecDeque::new();

            visited_nodes.insert(root_node.id.clone(), root_node);
            frontier.push_back((root_id.clone(), 0));

            let review_filter = query.review_state.map(|state| state.as_str().to_owned());

            while let Some((current_id, current_depth)) = frontier.pop_front() {
                if current_depth >= max_depth || visited_nodes.len() >= limit {
                    continue;
                }

                let mut edge_stmt = guard.prepare(
                    "SELECT e.id, e.source_node_id, e.target_node_id, e.source_domain, e.target_domain,
                            e.relation_kind, e.evidence_hash, e.evidence_locator, e.review_state,
                            e.uncertainty_note, e.created_at
                     FROM ontology_edges e
                     INNER JOIN ontology_nodes src ON src.id = e.source_node_id
                     INNER JOIN ontology_nodes dst ON dst.id = e.target_node_id
                     WHERE src.sensitivity_class != 'CHILD_SENSITIVE'
                       AND dst.sensitivity_class != 'CHILD_SENSITIVE'
                       AND (e.source_node_id = ?1 OR e.target_node_id = ?1)
                       AND (?2 IS NULL OR e.review_state = ?2)
                     ORDER BY e.created_at ASC, e.id ASC",
                )?;

                let incident_edges = edge_stmt.query_map(
                    params![&current_id, &review_filter],
                    row_to_ontology_edge,
                )?;

                for edge_row in incident_edges {
                    let edge = edge_row?;
                    let neighbor_id = if edge.source_node_id == current_id {
                        edge.target_node_id.clone()
                    } else {
                        edge.source_node_id.clone()
                    };

                    if !visited_nodes.contains_key(&neighbor_id) {
                        if visited_nodes.len() >= limit {
                            continue;
                        }
                        let neighbor_node = load_node_by_id(&guard, &neighbor_id)?;
                        if neighbor_node.sensitivity_class == SensitivityClass::ChildSensitive {
                            continue;
                        }
                        if let Some(required_domain) = query.domain
                            && neighbor_node.domain != required_domain
                        {
                            continue;
                        }
                        visited_nodes.insert(neighbor_node.id.clone(), neighbor_node);
                        frontier.push_back((neighbor_id, current_depth + 1));
                    }

                    if visited_nodes.contains_key(&edge.source_node_id)
                        && visited_nodes.contains_key(&edge.target_node_id)
                    {
                        visited_edges.insert(edge.id.clone(), edge);
                    }
                }
            }

            let mut nodes: Vec<OntologyNode> = visited_nodes.into_values().collect();
            nodes.sort_by(|left, right| left.created_at.cmp(&right.created_at).then(left.id.cmp(&right.id)));

            let mut edges: Vec<OntologyEdge> = visited_edges.into_values().collect();
            edges.sort_by(|left, right| left.created_at.cmp(&right.created_at).then(left.id.cmp(&right.id)));

            return Ok(SubgraphResponse {
                root_node_id: Some(root_id),
                traversal_depth: max_depth,
                node_count: nodes.len(),
                edge_count: edges.len(),
                nodes,
                edges,
                alias_auto_merge_enabled: false,
                boundary_notice,
            });
        }

        // Unrooted subgraph: load filtered nodes and all connecting edges between them.
        let domain_param = query.domain.map(|domain| domain.code().to_owned());
        let mut node_stmt = guard.prepare(
            "SELECT id, domain, canonical_label, corpus_ref, uncertainty_state,
                    alias_set_json, sensitivity_class, metadata_json, created_at
             FROM ontology_nodes
             WHERE sensitivity_class != 'CHILD_SENSITIVE'
               AND (?1 IS NULL OR domain = ?1)
             ORDER BY created_at ASC, id ASC
             LIMIT ?2",
        )?;

        let node_rows = node_stmt.query_map(params![domain_param, limit as i64], row_to_ontology_node)?;
        let mut nodes = Vec::new();
        let mut node_id_set = HashSet::new();
        for row_result in node_rows {
            let node = row_result?;
            node_id_set.insert(node.id.clone());
            nodes.push(node);
        }

        let review_param = query.review_state.map(|state| state.as_str().to_owned());
        let mut edge_stmt = guard.prepare(
            "SELECT e.id, e.source_node_id, e.target_node_id, e.source_domain, e.target_domain,
                    e.relation_kind, e.evidence_hash, e.evidence_locator, e.review_state,
                    e.uncertainty_note, e.created_at
             FROM ontology_edges e
             INNER JOIN ontology_nodes src ON src.id = e.source_node_id
             INNER JOIN ontology_nodes dst ON dst.id = e.target_node_id
             WHERE src.sensitivity_class != 'CHILD_SENSITIVE'
               AND dst.sensitivity_class != 'CHILD_SENSITIVE'
               AND (?1 IS NULL OR e.review_state = ?1)
             ORDER BY e.created_at ASC, e.id ASC",
        )?;

        let edge_rows = edge_stmt.query_map(params![review_param], row_to_ontology_edge)?;
        let mut edges = Vec::new();
        for row_result in edge_rows {
            let edge = row_result?;
            if node_id_set.contains(&edge.source_node_id) && node_id_set.contains(&edge.target_node_id)
            {
                edges.push(edge);
            }
        }

        Ok(SubgraphResponse {
            root_node_id: None,
            traversal_depth: 0,
            node_count: nodes.len(),
            edge_count: edges.len(),
            nodes,
            edges,
            alias_auto_merge_enabled: false,
            boundary_notice,
        })
    }
}

fn sanitize_alias_list(raw_aliases: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut cleaned = Vec::new();
    for raw_alias in raw_aliases {
        let trimmed = raw_alias.trim();
        if !trimmed.is_empty() && seen.insert(trimmed.to_owned()) {
            cleaned.push(trimmed.to_owned());
        }
    }
    cleaned
}

fn load_node_by_id(conn: &Connection, node_id: &str) -> Result<OntologyNode, GraphError> {
    conn.query_row(
        "SELECT id, domain, canonical_label, corpus_ref, uncertainty_state,
                alias_set_json, sensitivity_class, metadata_json, created_at
         FROM ontology_nodes
         WHERE id = ?1",
        params![node_id],
        row_to_ontology_node,
    )
    .optional()?
    .ok_or_else(|| GraphError::NodeNotFound {
        node_id: node_id.to_owned(),
    })
}

fn row_to_ontology_node(row: &Row<'_>) -> rusqlite::Result<OntologyNode> {
    let domain_str: String = row.get(1)?;
    let uncertainty_str: String = row.get(4)?;
    let alias_set_json: String = row.get(5)?;
    let sensitivity_str: String = row.get(6)?;
    let metadata_json: String = row.get(7)?;

    let domain = ProvenanceDomain::from_str(&domain_str).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(err))
    })?;
    let uncertainty_state = UncertaintyState::from_str(&uncertainty_str).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(err))
    })?;
    let alias_set: Vec<String> = serde_json::from_str(&alias_set_json).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(err))
    })?;
    let sensitivity_class = SensitivityClass::from_db_str(&sensitivity_str).map_err(|err_msg| {
        rusqlite::Error::FromSqlConversionFailure(
            6,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, err_msg)),
        )
    })?;
    let metadata: serde_json::Value = serde_json::from_str(&metadata_json).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(err))
    })?;

    Ok(OntologyNode {
        id: row.get(0)?,
        domain,
        canonical_label: row.get(2)?,
        corpus_ref: row.get(3)?,
        uncertainty_state,
        alias_set,
        alias_set_json,
        sensitivity_class,
        metadata,
        created_at: row.get(8)?,
    })
}

fn row_to_ontology_edge(row: &Row<'_>) -> rusqlite::Result<OntologyEdge> {
    let source_domain_str: String = row.get(3)?;
    let target_domain_str: String = row.get(4)?;
    let relation_str: String = row.get(5)?;
    let review_str: String = row.get(8)?;

    let source_domain = ProvenanceDomain::from_str(&source_domain_str).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(err))
    })?;
    let target_domain = ProvenanceDomain::from_str(&target_domain_str).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(err))
    })?;
    let relation_kind = parse_relation_kind(&relation_str).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(err))
    })?;
    let review_state = ReviewState::from_str(&review_str).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, Box::new(err))
    })?;

    Ok(OntologyEdge {
        id: row.get(0)?,
        source_node_id: row.get(1)?,
        target_node_id: row.get(2)?,
        source_domain,
        target_domain,
        relation_kind,
        evidence_hash: row.get(6)?,
        evidence_locator: row.get(7)?,
        review_state,
        uncertainty_note: row.get(9)?,
        implies_causation: false,
        implies_identity: false,
        created_at: row.get(10)?,
    })
}

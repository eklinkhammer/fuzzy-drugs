//! Fuzzy-Drugs Core Library
//!
//! Local-first veterinary drug resolution system with Merkle tree audit trail.
//!
//! # Architecture
//!
//! ```text
//! Audio → Transcription → NER → Normalization → Disambiguation
//!                                                     │
//!                                     [STAGING: encounter_draft]
//!                                                     │
//!                                             Vet Review Queue
//!                                                     │
//!                                         Vet approves/edits
//!                                                     │
//!                                     ┌───────────────▼───────────────┐
//!                                     │      Merkle Tree Commit       │
//!                                     │  leaf = hash(encounter_json)  │
//!                                     │  update root                  │
//!                                     └───────────────┬───────────────┘
//!                                                     │
//!                             ┌───────────────────────┼───────────────────────┐
//!                             │                       │                       │
//!                             ▼                       ▼                       ▼
//!                         Billing               Compliance              PIMS Sync
//!                         Export                  Export              (when online)
//! ```
//!
//! # Core Principle
//!
//! **All drug resolutions require vet review.** No auto-acceptance regardless of confidence score.
//!
//! # Modules
//!
//! - [`db`]: SQLite database layer with FTS5 search
//! - [`models`]: Domain types (CatalogItem, Patient, Encounter, etc.)
//! - [`merkle`]: Merkle tree for tamper-evident audit log
//! - [`resolver`]: Semantic resolver (normalizer + disambiguator)
//! - [`export`]: Billing and compliance export

pub mod db;
pub mod export;
pub mod merkle;
pub mod models;
pub mod resolver;

// Re-export commonly used types
pub use db::Database;
pub use merkle::{LeafCommit, MerkleTree, TreeStats};
pub use models::{
    CatalogItem, DoseRange, DraftStatus, EncounterDraft, EncounterLineItem, Patient,
    ResolutionMethod, ResolutionStatus, ReviewedEncounter,
};
pub use resolver::{Normalizer, Resolver};

// UniFFI setup - using proc macros
uniffi::setup_scaffolding!();

use std::sync::{Arc, Mutex};

// =========================================================================
// FFI Error Type
// =========================================================================

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum FuzzyDrugsError {
    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Sync error: {0}")]
    SyncError(String),
}

impl From<db::DbError> for FuzzyDrugsError {
    fn from(e: db::DbError) -> Self {
        FuzzyDrugsError::DatabaseError(e.to_string())
    }
}

impl From<serde_json::Error> for FuzzyDrugsError {
    fn from(e: serde_json::Error) -> Self {
        FuzzyDrugsError::SerializationError(e.to_string())
    }
}

impl From<merkle::MerkleError> for FuzzyDrugsError {
    fn from(e: merkle::MerkleError) -> Self {
        FuzzyDrugsError::DatabaseError(e.to_string())
    }
}

impl From<resolver::ResolverError> for FuzzyDrugsError {
    fn from(e: resolver::ResolverError) -> Self {
        FuzzyDrugsError::DatabaseError(e.to_string())
    }
}

impl<T> From<std::sync::PoisonError<T>> for FuzzyDrugsError {
    fn from(e: std::sync::PoisonError<T>) -> Self {
        FuzzyDrugsError::DatabaseError(format!("Lock poisoned: {}", e))
    }
}

// =========================================================================
// Factory Functions (exported to FFI)
// =========================================================================

/// Open or create a database at the given path.
#[uniffi::export]
pub fn open_database(path: String) -> Result<Arc<FuzzyDrugsCore>, FuzzyDrugsError> {
    let db = Database::open(&path)?;
    Ok(Arc::new(FuzzyDrugsCore {
        db: Arc::new(Mutex::new(db)),
    }))
}

/// Create an in-memory database (for testing).
#[uniffi::export]
pub fn open_database_in_memory() -> Result<Arc<FuzzyDrugsCore>, FuzzyDrugsError> {
    let db = Database::open_in_memory()?;
    Ok(Arc::new(FuzzyDrugsCore {
        db: Arc::new(Mutex::new(db)),
    }))
}

// =========================================================================
// Main API Object
// =========================================================================

/// Thread-safe database wrapper for FFI.
#[derive(uniffi::Object)]
pub struct FuzzyDrugsCore {
    db: Arc<Mutex<Database>>,
}

#[uniffi::export]
impl FuzzyDrugsCore {
    // =========================================================================
    // Catalog Operations
    // =========================================================================

    /// Add or update a catalog item.
    pub fn upsert_catalog_item(&self, item: FfiCatalogItem) -> Result<(), FuzzyDrugsError> {
        let db = self.db.lock()?;
        let catalog_item = item.into();
        db.upsert_catalog_item(&catalog_item)?;
        Ok(())
    }

    /// Get a catalog item by SKU.
    pub fn get_catalog_item(&self, sku: String) -> Result<Option<FfiCatalogItem>, FuzzyDrugsError> {
        let db = self.db.lock()?;
        let item = db.get_catalog_item(&sku)?;
        Ok(item.map(|i| i.into()))
    }

    /// Search catalog by name/alias.
    pub fn search_catalog(
        &self,
        query: String,
        limit: u32,
    ) -> Result<Vec<FfiCatalogItem>, FuzzyDrugsError> {
        let db = self.db.lock()?;
        let items = db.search_catalog(&query, limit as usize)?;
        Ok(items.into_iter().map(|i| i.into()).collect())
    }

    // =========================================================================
    // Patient Operations
    // =========================================================================

    /// Create a new patient.
    pub fn create_patient(
        &self,
        name: String,
        species: String,
    ) -> Result<FfiPatient, FuzzyDrugsError> {
        let db = self.db.lock()?;
        let patient = Patient::new(name, species);
        db.insert_patient(&patient)?;
        Ok(patient.into())
    }

    /// Get a patient by local ID.
    pub fn get_patient(&self, local_id: String) -> Result<Option<FfiPatient>, FuzzyDrugsError> {
        let db = self.db.lock()?;
        let patient = db.get_patient(&local_id)?;
        Ok(patient.map(|p| p.into()))
    }

    /// Search patients by name.
    pub fn search_patients(
        &self,
        query: String,
        limit: u32,
    ) -> Result<Vec<FfiPatient>, FuzzyDrugsError> {
        let db = self.db.lock()?;
        let patients = db.search_patients(&query, limit as usize)?;
        Ok(patients.into_iter().map(|p| p.into()).collect())
    }

    // =========================================================================
    // Draft Operations
    // =========================================================================

    /// Create a new encounter draft.
    pub fn create_draft(&self, patient_id: String) -> Result<FfiEncounterDraft, FuzzyDrugsError> {
        let db = self.db.lock()?;
        let draft = EncounterDraft::new(patient_id);
        db.insert_draft(&draft)?;
        Ok(draft.into())
    }

    /// Get a draft by ID.
    pub fn get_draft(&self, draft_id: String) -> Result<Option<FfiEncounterDraft>, FuzzyDrugsError> {
        let db = self.db.lock()?;
        let draft = db.get_draft(&draft_id)?;
        Ok(draft.map(|d| d.into()))
    }

    /// Get drafts pending review (sorted by lowest confidence first).
    pub fn get_pending_review_drafts(&self) -> Result<Vec<FfiEncounterDraft>, FuzzyDrugsError> {
        let db = self.db.lock()?;
        let drafts = db.list_pending_review_drafts()?;
        Ok(drafts.into_iter().map(|d| d.into()).collect())
    }

    // =========================================================================
    // Resolver Operations
    // =========================================================================

    /// Resolve a drug mention to SKU candidates.
    pub fn resolve_mention(
        &self,
        drug_name: String,
        dose: Option<f64>,
        unit: Option<String>,
        route: Option<String>,
        patient_species: Option<String>,
        patient_weight_kg: Option<f64>,
    ) -> Result<FfiResolvedItem, FuzzyDrugsError> {
        let db = self.db.lock()?;
        let resolver = Resolver::new(&db);

        let mention = models::DrugMention {
            raw_text: format!(
                "{} {} {}",
                dose.map(|d| d.to_string()).unwrap_or_default(),
                drug_name,
                route.clone().unwrap_or_default()
            ),
            drug_name,
            dose,
            unit,
            route,
            species: patient_species.clone(),
            start_offset: 0,
            end_offset: 0,
        };

        let resolved = resolver.resolve(&mention, patient_species.as_deref(), patient_weight_kg)?;

        Ok(resolved.into())
    }

    // =========================================================================
    // Merkle Tree Operations
    // =========================================================================

    /// Commit a reviewed encounter to the Merkle tree.
    pub fn commit_encounter(
        &self,
        encounter: FfiReviewedEncounter,
    ) -> Result<FfiLeafCommit, FuzzyDrugsError> {
        let db = self.db.lock()?;
        let tree = MerkleTree::new(&db);
        let reviewed: ReviewedEncounter = encounter.into();
        let commit = tree.commit_encounter(&reviewed)?;
        Ok(commit.into())
    }

    /// Get current tree statistics.
    pub fn get_tree_stats(&self) -> Result<FfiTreeStats, FuzzyDrugsError> {
        let db = self.db.lock()?;
        let tree = MerkleTree::new(&db);
        let stats = tree.get_stats()?;
        Ok(stats.into())
    }

    /// Check if there are unsynced changes.
    pub fn has_unsynced_changes(&self) -> Result<bool, FuzzyDrugsError> {
        let db = self.db.lock()?;
        let sync_manager = merkle::SyncManager::new(&db);
        Ok(sync_manager.has_unsynced_changes()?)
    }

    // =========================================================================
    // Export Operations
    // =========================================================================

    /// Export billing data as JSON.
    pub fn export_billing_json(&self) -> Result<String, FuzzyDrugsError> {
        let db = self.db.lock()?;
        let exporter = export::BillingExporter::new(&db);
        let batch = exporter.export_all()?;
        Ok(batch.to_json()?)
    }

    /// Export billing data as CSV.
    pub fn export_billing_csv(&self) -> Result<String, FuzzyDrugsError> {
        let db = self.db.lock()?;
        let exporter = export::BillingExporter::new(&db);
        let batch = exporter.export_all()?;
        Ok(batch.to_csv())
    }

    /// Export compliance data as JSON.
    pub fn export_compliance_json(&self) -> Result<String, FuzzyDrugsError> {
        let db = self.db.lock()?;
        let exporter = export::ComplianceExporter::new(&db);
        let batch = exporter.export_all()?;
        Ok(batch.to_json()?)
    }
}

// =========================================================================
// FFI Types
// =========================================================================

/// FFI-safe catalog item.
#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiCatalogItem {
    pub sku: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub concentration: Option<String>,
    pub package_size: Option<String>,
    pub species: Vec<String>,
    pub routes: Vec<String>,
    pub active: bool,
}

impl From<CatalogItem> for FfiCatalogItem {
    fn from(item: CatalogItem) -> Self {
        Self {
            sku: item.sku,
            name: item.name,
            aliases: item.aliases,
            concentration: item.concentration,
            package_size: item.package_size,
            species: item.species,
            routes: item.routes,
            active: item.active,
        }
    }
}

impl From<FfiCatalogItem> for CatalogItem {
    fn from(item: FfiCatalogItem) -> Self {
        CatalogItem {
            sku: item.sku,
            name: item.name,
            aliases: item.aliases,
            concentration: item.concentration,
            package_size: item.package_size,
            species: item.species,
            routes: item.routes,
            dose_range: None,
            active: item.active,
            server_id: None,
            last_synced: None,
        }
    }
}

/// FFI-safe patient.
#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiPatient {
    pub local_id: String,
    pub server_id: Option<String>,
    pub name: String,
    pub species: String,
    pub breed: Option<String>,
    pub weight_kg: Option<f64>,
}

impl From<Patient> for FfiPatient {
    fn from(patient: Patient) -> Self {
        Self {
            local_id: patient.local_id,
            server_id: patient.server_id,
            name: patient.name,
            species: patient.species,
            breed: patient.breed,
            weight_kg: patient.weight_kg,
        }
    }
}

/// FFI-safe encounter draft.
#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiEncounterDraft {
    pub draft_id: String,
    pub patient_id: String,
    pub transcript: String,
    pub status: String,
    pub pending_review_count: u32,
    pub lowest_confidence: Option<f64>,
}

impl From<EncounterDraft> for FfiEncounterDraft {
    fn from(draft: EncounterDraft) -> Self {
        Self {
            draft_id: draft.draft_id.clone(),
            patient_id: draft.patient_id.clone(),
            transcript: draft.transcript.clone(),
            status: format!("{:?}", draft.status),
            pending_review_count: draft.pending_review_count() as u32,
            lowest_confidence: draft.lowest_confidence(),
        }
    }
}

/// FFI-safe resolved item.
#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiResolvedItem {
    pub normalized_name: String,
    pub normalized_dose: Option<f64>,
    pub normalized_unit: Option<String>,
    pub normalized_route: Option<String>,
    pub top_sku: String,
    pub top_name: String,
    pub top_confidence: f64,
    pub alternatives: Vec<FfiScoredCandidate>,
}

impl From<models::ResolvedItem> for FfiResolvedItem {
    fn from(item: models::ResolvedItem) -> Self {
        Self {
            normalized_name: item.mention.normalized_name,
            normalized_dose: item.mention.normalized_dose,
            normalized_unit: item.mention.normalized_unit,
            normalized_route: item.mention.normalized_route,
            top_sku: item.top_candidate.sku,
            top_name: item.top_candidate.name,
            top_confidence: item.top_candidate.confidence,
            alternatives: item.alternatives.into_iter().map(|c| c.into()).collect(),
        }
    }
}

/// FFI-safe scored candidate.
#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiScoredCandidate {
    pub sku: String,
    pub name: String,
    pub confidence: f64,
}

impl From<models::ScoredCandidate> for FfiScoredCandidate {
    fn from(candidate: models::ScoredCandidate) -> Self {
        Self {
            sku: candidate.sku,
            name: candidate.name,
            confidence: candidate.confidence,
        }
    }
}

/// FFI-safe reviewed encounter.
#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiReviewedEncounter {
    pub draft_id: String,
    pub patient_id: String,
    pub patient_server_id: Option<String>,
    pub transcript: String,
    pub line_items: Vec<FfiLineItem>,
    pub reviewed_by: String,
    pub notes: Option<String>,
}

impl From<FfiReviewedEncounter> for ReviewedEncounter {
    fn from(enc: FfiReviewedEncounter) -> Self {
        ReviewedEncounter {
            draft_id: enc.draft_id,
            patient_id: enc.patient_id,
            patient_server_id: enc.patient_server_id,
            transcript: enc.transcript,
            line_items: enc.line_items.into_iter().map(|i| i.into()).collect(),
            reviewed_by: enc.reviewed_by,
            reviewed_at: chrono::Utc::now().to_rfc3339(),
            notes: enc.notes,
        }
    }
}

/// FFI-safe line item.
#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiLineItem {
    pub sku: String,
    pub name: String,
    pub quantity: f64,
    pub unit: String,
    pub route: Option<String>,
    pub original_mention: String,
}

impl From<FfiLineItem> for EncounterLineItem {
    fn from(item: FfiLineItem) -> Self {
        EncounterLineItem {
            sku: item.sku,
            name: item.name,
            quantity: item.quantity,
            unit: item.unit,
            route: item.route,
            original_mention: item.original_mention,
            resolution_method: ResolutionMethod::SystemApproved { confidence: 1.0 },
        }
    }
}

/// FFI-safe leaf commit result.
#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiLeafCommit {
    pub leaf_hash: String,
    pub root_hash: String,
    pub tree_height: u32,
    pub leaf_count: u32,
}

impl From<LeafCommit> for FfiLeafCommit {
    fn from(commit: LeafCommit) -> Self {
        Self {
            leaf_hash: commit.leaf_hash,
            root_hash: commit.root_hash,
            tree_height: commit.tree_height,
            leaf_count: commit.leaf_count,
        }
    }
}

/// FFI-safe tree statistics.
#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiTreeStats {
    pub root_hash: Option<String>,
    pub height: u32,
    pub leaf_count: u32,
}

impl From<TreeStats> for FfiTreeStats {
    fn from(stats: TreeStats) -> Self {
        Self {
            root_hash: stats.root_hash,
            height: stats.height,
            leaf_count: stats.leaf_count,
        }
    }
}

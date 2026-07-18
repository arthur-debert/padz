use crate::error::Result;
use crate::model::Scope;
use crate::store::DataStore;

/// Semantic result of reconciling a store's index and content files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoctorOutcome {
    Clean {
        missing_files: usize,
        recovered_files: usize,
    },
    Repaired {
        missing_files: usize,
        recovered_files: usize,
    },
}

pub fn run<S: DataStore>(store: &mut S, scope: Scope) -> Result<DoctorOutcome> {
    let report = store.doctor(scope)?;
    if report.fixed_missing_files == 0 && report.recovered_files == 0 {
        Ok(DoctorOutcome::Clean {
            missing_files: 0,
            recovered_files: 0,
        })
    } else {
        Ok(DoctorOutcome::Repaired {
            missing_files: report.fixed_missing_files,
            recovered_files: report.recovered_files,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Metadata;
    use crate::store::backend::StorageBackend;
    use crate::store::bucketed::BucketedStore;
    use crate::store::mem_backend::MemBackend;
    use chrono::Utc;
    use std::collections::HashMap;
    use uuid::Uuid;

    /// Helper: create a BucketedStore where the active backend has been pre-populated.
    fn bucketed_with_active(active_backend: MemBackend) -> BucketedStore<MemBackend> {
        BucketedStore::new(
            active_backend,
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        )
    }

    #[test]
    fn doctor_no_inconsistencies() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        let result = run(&mut store, Scope::Project).unwrap();

        assert_eq!(
            result,
            DoctorOutcome::Clean {
                missing_files: 0,
                recovered_files: 0
            }
        );
    }

    #[test]
    fn doctor_recovers_orphan_files() {
        let backend = MemBackend::new();
        let orphan_id = Uuid::new_v4();

        // Create orphan: content exists but no index entry
        backend
            .write_content(&orphan_id, Scope::Project, "Orphan Title\n\nBody")
            .unwrap();

        let mut store = bucketed_with_active(backend);
        let result = run(&mut store, Scope::Project).unwrap();

        assert_eq!(
            result,
            DoctorOutcome::Repaired {
                missing_files: 0,
                recovered_files: 1
            }
        );
    }

    #[test]
    fn doctor_removes_zombie_entries() {
        let backend = MemBackend::new();
        let zombie_id = Uuid::new_v4();

        // Create zombie: index entry exists but no content
        let mut index = HashMap::new();
        index.insert(
            zombie_id,
            Metadata {
                id: zombie_id,
                created_at: Utc::now(),
                updated_at: Utc::now(),
                is_pinned: false,
                pinned_at: None,
                delete_protected: false,
                parent_id: None,
                title: "Zombie".to_string(),
                status: crate::model::TodoStatus::Planned,
                tags: Vec::new(),
            },
        );
        backend.save_index(Scope::Project, &index).unwrap();

        let mut store = bucketed_with_active(backend);
        let result = run(&mut store, Scope::Project).unwrap();

        assert_eq!(
            result,
            DoctorOutcome::Repaired {
                missing_files: 1,
                recovered_files: 0
            }
        );
    }

    #[test]
    fn doctor_reports_both_issues() {
        let backend = MemBackend::new();

        // Create orphan content
        let orphan_id = Uuid::new_v4();
        backend
            .write_content(&orphan_id, Scope::Project, "Orphan\n\nContent")
            .unwrap();

        // Create zombie entry
        let zombie_id = Uuid::new_v4();
        let mut index = HashMap::new();
        index.insert(
            zombie_id,
            Metadata {
                id: zombie_id,
                created_at: Utc::now(),
                updated_at: Utc::now(),
                is_pinned: false,
                pinned_at: None,
                delete_protected: false,
                parent_id: None,
                title: "Zombie".to_string(),
                status: crate::model::TodoStatus::Planned,
                tags: Vec::new(),
            },
        );
        backend.save_index(Scope::Project, &index).unwrap();

        let mut store = bucketed_with_active(backend);
        let result = run(&mut store, Scope::Project).unwrap();

        assert_eq!(
            result,
            DoctorOutcome::Repaired {
                missing_files: 1,
                recovered_files: 1
            }
        );
    }
}

use super::mem_backend::MemBackend;
use super::pad_store::PadStore;

pub type InMemoryStore = PadStore<MemBackend>;

impl Default for InMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryStore {
    pub fn new() -> Self {
        PadStore::with_backend(MemBackend::new())
    }
}

// --- Test Fixtures ---

#[cfg(any(test, feature = "test_utils"))]
pub mod fixtures {
    use super::*;
    use crate::model::{Pad, Scope};
    use crate::store::DataStore;

    pub struct StoreFixture {
        pub store: InMemoryStore,
    }

    impl Default for StoreFixture {
        fn default() -> Self {
            Self::new()
        }
    }

    impl StoreFixture {
        pub fn new() -> Self {
            Self {
                store: InMemoryStore::new(),
            }
        }

        pub fn with_pads(mut self, count: usize, scope: Scope) -> Self {
            for i in 0..count {
                let title = format!("Test Pad {}", i + 1);
                let content = format!("Content for pad {}", i + 1);
                let pad = Pad::new(title, content);
                self.store.save_pad(&pad, scope).unwrap();
            }
            self
        }

        pub fn with_active_pad(mut self, title: &str, scope: Scope) -> Self {
            let pad = Pad::new(title.to_string(), "Some content".to_string());
            self.store.save_pad(&pad, scope).unwrap();
            self
        }

        pub fn with_pinned_pad(mut self, title: &str, scope: Scope) -> Self {
            let mut pad = Pad::new(title.to_string(), "Pinned content".to_string());
            pad.metadata.is_pinned = true;
            pad.metadata.pinned_at = Some(chrono::Utc::now());
            self.store.save_pad(&pad, scope).unwrap();
            self
        }

        pub fn with_deleted_pad(mut self, title: &str, scope: Scope) -> Self {
            let mut pad = Pad::new(title.to_string(), "Deleted content".to_string());
            pad.metadata.is_deleted = true;
            pad.metadata.deleted_at = Some(chrono::Utc::now());
            self.store.save_pad(&pad, scope).unwrap();
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::StoreFixture;
    use super::*;
    use crate::error::PadzError;
    use crate::model::Scope;
    use crate::store::backend::StorageBackend;
    use crate::store::DataStore;
    use crate::tags::TagEntry;
    use uuid::Uuid;

    #[test]
    fn test_delete_not_found() {
        let mut store = InMemoryStore::new();
        let id = Uuid::new_v4();
        match store.delete_pad(&id, Scope::Project) {
            Err(PadzError::PadNotFound(err_id)) => assert_eq!(err_id, id),
            _ => panic!("Expected PadNotFound"),
        }
    }

    #[test]
    fn test_doctor_noop() {
        let mut store = InMemoryStore::new();
        let report = store.doctor(Scope::Project).unwrap();
        // InMemoryStore doctor does nothing, so strict default check
        assert_eq!(report.fixed_missing_files, 0);
        assert_eq!(report.recovered_files, 0);
        assert_eq!(report.fixed_content_files, 0);
    }

    #[test]
    fn test_fixtures_coverage() {
        // Exercise fixture methods to cover lines 71-112
        let fixture = StoreFixture::default() // covers Default trait (71-72)
            .with_pads(2, Scope::Project) // covers with_pads (83-91)
            .with_active_pad("Active", Scope::Project) // covers with_active_pad (93-97)
            .with_pinned_pad("Pinned", Scope::Project) // covers with_pinned_pad (99-105)
            .with_deleted_pad("Deleted", Scope::Project); // covers with_deleted_pad (107-113)

        let pads = fixture.store.list_pads(Scope::Project).unwrap();
        assert_eq!(pads.len(), 5);

        let active = pads.iter().find(|p| p.metadata.title == "Active").unwrap();
        assert!(!active.metadata.is_pinned);
        assert!(!active.metadata.is_deleted);

        let pinned = pads.iter().find(|p| p.metadata.title == "Pinned").unwrap();
        assert!(pinned.metadata.is_pinned);

        let deleted = pads.iter().find(|p| p.metadata.title == "Deleted").unwrap();
        assert!(deleted.metadata.is_deleted);

        let generic = pads
            .iter()
            .filter(|p| p.metadata.title.starts_with("Test Pad"))
            .count();
        assert_eq!(generic, 2);
    }

    #[test]
    fn test_mem_backend_tags_empty() {
        use crate::store::mem_backend::MemBackend;

        let backend = MemBackend::new();
        let tags = backend.load_tags(Scope::Project).unwrap();
        assert!(tags.is_empty());
    }

    #[test]
    fn test_mem_backend_tags_save_and_load() {
        use crate::store::mem_backend::MemBackend;

        let backend = MemBackend::new();
        let tags = vec![
            TagEntry::new("work".to_string()),
            TagEntry::new("rust".to_string()),
        ];

        backend.save_tags(Scope::Project, &tags).unwrap();

        let loaded = backend.load_tags(Scope::Project).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].name, "work");
        assert_eq!(loaded[1].name, "rust");
    }

    #[test]
    fn test_mem_backend_tags_scope_isolation() {
        use crate::store::mem_backend::MemBackend;

        let backend = MemBackend::new();
        let project_tags = vec![TagEntry::new("project-tag".to_string())];
        let global_tags = vec![TagEntry::new("global-tag".to_string())];

        backend.save_tags(Scope::Project, &project_tags).unwrap();
        backend.save_tags(Scope::Global, &global_tags).unwrap();

        let loaded_project = backend.load_tags(Scope::Project).unwrap();
        let loaded_global = backend.load_tags(Scope::Global).unwrap();

        assert_eq!(loaded_project.len(), 1);
        assert_eq!(loaded_project[0].name, "project-tag");

        assert_eq!(loaded_global.len(), 1);
        assert_eq!(loaded_global[0].name, "global-tag");
    }
}

use crate::commands::{CmdMessage, CmdResult};
use crate::error::Result;
use crate::model::Scope;
use crate::store::DataStore;

pub fn run<S: DataStore>(store: &mut S, scope: Scope) -> Result<CmdResult> {
    let report = store.doctor(scope)?;
    let mut result = CmdResult::default();

    if report.fixed_missing_files == 0 && report.recovered_files == 0 {
        result.add_message(CmdMessage::success("No inconsistencies found."));
    } else {
        result.add_message(CmdMessage::warning("Inconsistencies found and fixed:"));
        if report.fixed_missing_files > 0 {
            result.add_message(CmdMessage::info(format!(
                "  - Removed {} pad(s) listed in DB but missing from disk.",
                report.fixed_missing_files
            )));
        }
        if report.recovered_files > 0 {
            result.add_message(CmdMessage::success(format!(
                "  - Recovered {} pad(s) found on disk but missing from DB.",
                report.recovered_files
            )));
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Metadata;
    use crate::store::backend::StorageBackend;
    use crate::store::mem_backend::MemBackend;
    use crate::store::memory::InMemoryStore;
    use crate::store::pad_store::PadStore;
    use chrono::Utc;
    use std::collections::HashMap;
    use uuid::Uuid;

    #[test]
    fn doctor_no_inconsistencies() {
        let mut store = InMemoryStore::new();

        let result = run(&mut store, Scope::Project).unwrap();

        assert_eq!(result.messages.len(), 1);
        assert!(result.messages[0].content.contains("No inconsistencies"));
    }

    #[test]
    fn doctor_recovers_orphan_files() {
        let backend = MemBackend::new();
        let orphan_id = Uuid::new_v4();

        // Create orphan: content exists but no index entry
        backend
            .write_content(&orphan_id, Scope::Project, "Orphan Title\n\nBody")
            .unwrap();

        let mut store = PadStore::with_backend(backend);
        let result = run(&mut store, Scope::Project).unwrap();

        // Should report inconsistencies found
        assert!(result.messages.len() >= 2);
        assert!(result.messages[0].content.contains("Inconsistencies found"));
        // Should report recovered files
        assert!(result
            .messages
            .iter()
            .any(|m| m.content.contains("Recovered") && m.content.contains("1")));
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
                is_deleted: false,
                deleted_at: None,
                delete_protected: false,
                parent_id: None,
                title: "Zombie".to_string(),
            },
        );
        backend.save_index(Scope::Project, &index).unwrap();

        let mut store = PadStore::with_backend(backend);
        let result = run(&mut store, Scope::Project).unwrap();

        // Should report inconsistencies found
        assert!(result.messages.len() >= 2);
        assert!(result.messages[0].content.contains("Inconsistencies found"));
        // Should report removed entries
        assert!(result
            .messages
            .iter()
            .any(|m| m.content.contains("Removed") && m.content.contains("1")));
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
                is_deleted: false,
                deleted_at: None,
                delete_protected: false,
                parent_id: None,
                title: "Zombie".to_string(),
            },
        );
        backend.save_index(Scope::Project, &index).unwrap();

        let mut store = PadStore::with_backend(backend);
        let result = run(&mut store, Scope::Project).unwrap();

        // Should have warning + both issue types
        assert!(result.messages.len() >= 3);
        assert!(result.messages[0].content.contains("Inconsistencies found"));
        assert!(result
            .messages
            .iter()
            .any(|m| m.content.contains("Removed")));
        assert!(result
            .messages
            .iter()
            .any(|m| m.content.contains("Recovered")));
    }
}

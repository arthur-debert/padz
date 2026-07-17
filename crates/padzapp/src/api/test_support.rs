//! Test-only helpers shared across `api/` submodule tests.

use super::{PadzApi, PadzPaths};
use crate::store::bucketed::BucketedStore;
use crate::store::mem_backend::MemBackend;
use std::path::PathBuf;

pub(crate) type TestStore = BucketedStore<MemBackend>;

pub(crate) fn make_store() -> TestStore {
    BucketedStore::new(
        MemBackend::new(),
        MemBackend::new(),
        MemBackend::new(),
        MemBackend::new(),
    )
}

pub(crate) fn make_api() -> PadzApi<TestStore> {
    let store = make_store();
    let paths = PadzPaths {
        project: Some(PathBuf::from("/tmp/test")),
        global: PathBuf::from("/tmp/global"),
        home: None,
    };
    PadzApi::new(store, paths)
}

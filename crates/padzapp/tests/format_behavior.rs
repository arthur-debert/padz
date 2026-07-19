//! File-format behavior at the core/store seam.
//!
//! These tests use the public API with a real [`FileStore`]: format selection is
//! a core argument here, while the resulting filename and mixed-extension lookup
//! are filesystem-store behavior. Neither requires spawning the CLI binary.

use padzapp::api::{PadFilter, PadzApi, PadzPaths};
use padzapp::commands::NestingMode;
use padzapp::model::Scope;
use padzapp::store::fs::FileStore;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

struct Fixture {
    _temp: TempDir,
    api: PadzApi<FileStore>,
}

impl Fixture {
    fn new() -> Self {
        Self::with_default_format("txt")
    }

    fn with_default_format(format: &str) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("project").join(".padz");
        let global = temp.path().join("global");
        let store = FileStore::new_fs(Some(project.clone()), global.clone()).with_format(format);
        let paths = PadzPaths {
            project: Some(project),
            global,
            home: None,
        };

        Self {
            _temp: temp,
            api: PadzApi::new(store, paths),
        }
    }

    fn create(&mut self, title: &str) -> PathBuf {
        self.api
            .create_pad(Scope::Project, title.to_string(), "body".to_string(), None)
            .unwrap()
            .pad_paths
            .remove(0)
    }

    fn create_with_format(&mut self, title: &str, format: &str) -> PathBuf {
        self.api
            .create_pad_with_format(
                Scope::Project,
                title.to_string(),
                "body".to_string(),
                None,
                format,
            )
            .unwrap()
            .pad_paths
            .remove(0)
    }
}

fn assert_extension(path: &Path, expected: &str) {
    assert_eq!(
        path.extension().and_then(|ext| ext.to_str()),
        Some(expected)
    );
    assert!(
        path.exists(),
        "created pad should exist at {}",
        path.display()
    );
}

#[test]
fn default_create_stores_a_txt_file() {
    let mut fx = Fixture::new();

    let path = fx.create("plain note");

    assert_extension(&path, "txt");
}

#[test]
fn explicit_md_format_stores_a_markdown_file() {
    let mut fx = Fixture::new();

    let path = fx.create_with_format("markdown note", "md");

    assert_extension(&path, "md");
}

#[test]
fn an_explicit_format_override_does_not_persist() {
    let mut fx = Fixture::new();

    let overridden = fx.create_with_format("markdown note", "md");
    let following = fx.create("plain note");

    assert_extension(&overridden, "md");
    assert_extension(&following, "txt");
}

#[test]
fn markdown_and_text_aliases_select_the_canonical_extensions() {
    let mut markdown_fx = Fixture::new();
    let mut text_fx = Fixture::with_default_format("md");

    let markdown = markdown_fx.create_with_format("markdown alias", "markdown");
    let text = text_fx.create_with_format("text alias", "text");

    assert_extension(&markdown, "md");
    assert_extension(&text, "txt");
}

#[test]
fn mixed_format_pads_are_listable_and_viewable() {
    let mut fx = Fixture::new();
    let markdown = fx.create_with_format("markdown note", "md");
    let text = fx.create("plain text note");

    assert_extension(&markdown, "md");
    assert_extension(&text, "txt");

    let listed = fx
        .api
        .get_pads(Scope::Project, PadFilter::default(), &[] as &[String])
        .unwrap();
    let mut titles: Vec<_> = listed
        .listed_pads
        .iter()
        .map(|pad| pad.pad.metadata.title.as_str())
        .collect();
    titles.sort_unstable();
    assert_eq!(titles, ["markdown note", "plain text note"]);

    for selector in ["1", "2"] {
        let viewed = fx
            .api
            .view_pads(Scope::Project, &[selector], NestingMode::Flat)
            .unwrap();
        assert_eq!(viewed.listed_pads.len(), 1);
    }
}

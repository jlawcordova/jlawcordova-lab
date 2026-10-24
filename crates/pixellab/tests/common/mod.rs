//! Test helpers shared by the program's test suites (ports the parts of
//! cli/test/helpers.mjs the document tests use). Nothing here reads or
//! copies the real site checkout.
#![allow(dead_code)]

pub mod server;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// The repository root.
pub fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("repo root")
}

/// The sample document the suites run on (format 2).
pub fn fixture() -> PathBuf {
    repo().join("fixtures").join("sample.pixellab")
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A fresh, empty folder under the system temp folder, never the repo.
pub fn temp_dir(prefix: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("{prefix}{}-{nanos}-{n}", std::process::id()));
    fs::create_dir_all(&dir).expect("temp dir");
    dir.canonicalize().expect("temp dir path")
}

/// Copies a folder, recursively.
pub fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("create dir");
    for entry in fs::read_dir(from).expect("read dir").flatten() {
        let name = entry.file_name();
        if name == ".DS_Store" {
            continue;
        }
        let (src, dst) = (entry.path(), to.join(&name));
        if src.is_dir() {
            copy_dir(&src, &dst);
        } else {
            fs::copy(&src, &dst).expect("copy file");
        }
    }
}

/// Writes `text` to `root/file`, making the folders.
pub fn write(root: &Path, file: &str, text: &str) {
    let path = root.join(file);
    fs::create_dir_all(path.parent().expect("parent")).expect("create dir");
    fs::write(path, text).expect("write file");
}

/// A temp copy of the sample document with an empty HOME next to it.
pub struct TempDocument {
    pub base: PathBuf,
    pub root: PathBuf,
    pub home: PathBuf,
}

impl TempDocument {
    /// An environment with HOME set to the temp home.
    pub fn env(&self) -> HashMap<String, String> {
        HashMap::from([("HOME".to_string(), self.home.to_string_lossy().into_owned())])
    }
    pub fn write(&self, file: &str, text: &str) {
        write(&self.root, file, text);
    }
}

impl Drop for TempDocument {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

/// Copies the fixture into a temp folder as `sample.pixellab`, with a temp HOME.
pub fn make_document() -> TempDocument {
    let base = temp_dir("pixellab-test-");
    let root = base.join("sample.pixellab");
    let home = base.join("home");
    fs::create_dir_all(&home).expect("home");
    copy_dir(&fixture(), &root);
    TempDocument { base, root, home }
}

/// A temp folder laid out as a site checkout: the sample document at
/// `src/assets/pixel-art/sample.pixellab`, `src/styles/variables.css`, and a temp HOME.
pub struct TempSite {
    pub base: PathBuf,
    pub root: PathBuf,
    pub home: PathBuf,
    pub document: PathBuf,
}

impl TempSite {
    pub fn env(&self) -> HashMap<String, String> {
        HashMap::from([("HOME".to_string(), self.home.to_string_lossy().into_owned())])
    }
    pub fn write(&self, file: &str, text: &str) {
        write(&self.root, file, text);
    }
}

impl Drop for TempSite {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

pub const VARIABLES_CSS: &str = ":root {\n  --color-card: #E9DCC6;\n  --color-grid: rgba(90, 62, 43, 0.06);\n  --color-page: #F4EBDD;\n}\n\n[data-theme=\"dark\"] {\n  --color-page: #1c1712;\n}\n";

pub fn make_site() -> TempSite {
    let base = temp_dir("pixellab-test-");
    let root = base.join("site");
    let home = base.join("home");
    fs::create_dir_all(&home).expect("home");
    let document = root.join("src/assets/pixel-art/sample.pixellab");
    copy_dir(&fixture(), &document);
    write(&root, "src/styles/variables.css", VARIABLES_CSS);
    TempSite { base, root, home, document }
}

/// A `.pixellib` made in `dir` (as `<name>.pixellib`) by the program's own writer: the colors its
/// objects recorded at export as (name, hex, tier), and the objects by name.
pub fn make_library(dir: &Path, name: &str, colors: &[(&str, &str, &str)], objects: Vec<(&str, serde_json::Value)>) -> PathBuf {
    let tier = |t: &str| colors.iter().filter(|(_, _, c)| *c == t).map(|(n, h, _)| (n.to_string(), h.to_string())).collect();
    let lib = pixellab::library::Library {
        name: name.to_string(),
        exported_from: "test".to_string(),
        world: tier("world"),
        outfit: tier("outfit"),
        objects: objects.into_iter().map(|(n, o)| (n.to_string(), o)).collect(),
        load_errors: Vec::new(),
    };
    let path = dir.join(format!("{name}.pixellib"));
    pixellab::library::write_library(&path, &lib, false).expect("write the library");
    path
}

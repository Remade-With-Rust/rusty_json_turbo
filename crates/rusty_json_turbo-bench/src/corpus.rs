//! JSONCORP on disk: the three benchmark files plus the conformance sets.

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum File {
    Canada,
    CitmCatalog,
    Twitter,
}

impl File {
    pub const ALL: [File; 3] = [File::Twitter, File::CitmCatalog, File::Canada];

    pub fn parse(s: &str) -> Option<File> {
        match s {
            "canada" => Some(File::Canada),
            "citm" | "citm_catalog" | "citm-catalog" => Some(File::CitmCatalog),
            "twitter" => Some(File::Twitter),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            File::Canada => "canada",
            File::CitmCatalog => "citm_catalog",
            File::Twitter => "twitter",
        }
    }

    pub fn path(self) -> PathBuf {
        corpus_dir().join(format!("{}.json", self.name()))
    }

    pub fn load(self) -> Vec<u8> {
        let path = self.path();
        fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
    }
}

/// `<repo>/corpus`, located relative to this crate so tests and the binary
/// agree wherever they are run from.
pub fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("corpus")
}

/// Every `.json` file under the corpus: the three benchmark files, then
/// `jsonchecker/*` and `roundtrip/*`, as (display name, bytes).
pub fn all_documents() -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for f in File::ALL {
        out.push((format!("{}.json", f.name()), f.load()));
    }
    for sub in ["jsonchecker", "roundtrip"] {
        out.extend(json_files_in(&corpus_dir().join(sub), sub));
    }
    out
}

/// All `.json` files directly under `dir`, sorted by name, as (`prefix/name`, bytes).
pub fn json_files_in(dir: &Path, prefix: &str) -> Vec<(String, Vec<u8>)> {
    let mut names: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    names.sort();
    names
        .into_iter()
        .map(|p| {
            let name = format!("{prefix}/{}", p.file_name().unwrap().to_string_lossy());
            let bytes = fs::read(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
            (name, bytes)
        })
        .collect()
}

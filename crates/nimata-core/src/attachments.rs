//! Files attached to posts: their metadata, what kind of file each is, and
//! the local blob store that holds their bytes.
//!
//! Bytes live outside the database in a content-addressed store
//! (`blobs/sha256/ab/<hash>`), so a file attached twice is stored once and
//! a stored file can always be checked against its name.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::time::UnixMillis;

/// The largest file that can be attached.
pub const MAX_ATTACHMENT_BYTES: u64 = 50 * 1024 * 1024;

/// Content hash recorded for every attachment, as `sha256:<lowercase hex>`.
/// Identical files share a hash, which allows deduplication.
pub fn content_hash(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("sha256:{hex}")
}

/// The hex part of a content hash, if it is a well-formed SHA-256 hash.
pub fn hash_hex(hash: &str) -> Option<&str> {
    let hex = hash.strip_prefix("sha256:")?;
    (hex.len() == 64
        && hex
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
    .then_some(hex)
}

/// What Nimata can do with a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttachmentKind {
    /// UTF-8 text: plain text, Markdown, CSV, JSON, source code. Every model
    /// can read these; they are sent inline.
    Text,
    /// PNG, JPEG, GIF, or WebP.
    Image,
    Pdf,
    /// Anything else: kept and exported, but not sent to models.
    Other,
}

/// A file attached to a post.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: Uuid,
    pub post_id: Uuid,
    pub filename: String,
    pub media_type: String,
    pub size: u64,
    pub content_hash: String,
    pub kind: AttachmentKind,
    pub created_at: UnixMillis,
}

/// A file stored and waiting to be posted, kept with the draft.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StagedAttachment {
    pub filename: String,
    pub media_type: String,
    pub size: u64,
    pub content_hash: String,
    pub kind: AttachmentKind,
}

const IMAGE_TYPES: &[&str] = &["image/png", "image/jpeg", "image/gif", "image/webp"];

/// Extensions of text files that systems often report without a type, or
/// as `application/octet-stream`.
const TEXT_EXTENSIONS: &[(&str, &str)] = &[
    ("md", "text/markdown"),
    ("markdown", "text/markdown"),
    ("txt", "text/plain"),
    ("text", "text/plain"),
    ("log", "text/plain"),
    ("csv", "text/csv"),
    ("tsv", "text/tab-separated-values"),
    ("json", "application/json"),
    ("jsonl", "application/jsonl"),
    ("yaml", "application/yaml"),
    ("yml", "application/yaml"),
    ("toml", "application/toml"),
    ("xml", "application/xml"),
    ("html", "text/html"),
    ("htm", "text/html"),
    ("css", "text/css"),
    ("svg", "image/svg+xml"),
    ("sql", "application/sql"),
    ("rs", "text/x-rust"),
    ("py", "text/x-python"),
    ("js", "text/javascript"),
    ("mjs", "text/javascript"),
    ("ts", "text/x-typescript"),
    ("tsx", "text/x-typescript"),
    ("svelte", "text/x-svelte"),
    ("java", "text/x-java"),
    ("kt", "text/x-kotlin"),
    ("go", "text/x-go"),
    ("c", "text/x-c"),
    ("h", "text/x-c"),
    ("cpp", "text/x-c++"),
    ("swift", "text/x-swift"),
    ("rb", "text/x-ruby"),
    ("sh", "text/x-shellscript"),
    ("ex", "text/x-elixir"),
    ("exs", "text/x-elixir"),
    ("tex", "text/x-tex"),
    ("ini", "text/plain"),
    ("cfg", "text/plain"),
];

fn extension(filename: &str) -> Option<String> {
    let (_, ext) = filename.rsplit_once('.')?;
    Some(ext.to_ascii_lowercase())
}

/// Recognises images and PDFs by their first bytes.
fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else if bytes.starts_with(b"%PDF-") {
        Some("application/pdf")
    } else {
        None
    }
}

/// Decides a file's media type and kind from its contents, its name, and
/// the type the system reported. Contents win: a file named `.png` that is
/// not a PNG is not treated as an image.
pub fn classify(filename: &str, reported: &str, bytes: &[u8]) -> (String, AttachmentKind) {
    if let Some(sniffed) = sniff(bytes) {
        let kind = if sniffed == "application/pdf" {
            AttachmentKind::Pdf
        } else {
            AttachmentKind::Image
        };
        return (sniffed.to_string(), kind);
    }
    let reported = reported
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let by_extension = extension(filename)
        .and_then(|ext| TEXT_EXTENSIONS.iter().find(|(e, _)| *e == ext))
        .map(|(_, t)| t.to_string());
    let looks_textual = |t: &str| {
        t.starts_with("text/")
            || matches!(
                t,
                "application/json"
                    | "application/xml"
                    | "application/yaml"
                    | "application/toml"
                    | "application/javascript"
                    | "application/sql"
                    | "image/svg+xml"
            )
    };
    let media_type = if !reported.is_empty() && reported != "application/octet-stream" {
        reported.clone()
    } else {
        by_extension
            .clone()
            .unwrap_or_else(|| "application/octet-stream".into())
    };
    let textual = looks_textual(&media_type) || by_extension.is_some();
    // Images claimed by name or type but not recognised are just files.
    let kind = if textual && std::str::from_utf8(bytes).is_ok() {
        AttachmentKind::Text
    } else {
        AttachmentKind::Other
    };
    let media_type =
        if IMAGE_TYPES.contains(&media_type.as_str()) || media_type == "application/pdf" {
            "application/octet-stream".to_string()
        } else {
            media_type
        };
    (media_type, kind)
}

/// The kind recorded for a stored media type.
pub fn kind_of(media_type: &str, is_text: bool) -> AttachmentKind {
    if IMAGE_TYPES.contains(&media_type) {
        AttachmentKind::Image
    } else if media_type == "application/pdf" {
        AttachmentKind::Pdf
    } else if is_text {
        AttachmentKind::Text
    } else {
        AttachmentKind::Other
    }
}

/// A file name safe to show and to save under: no folders, no control
/// characters, at most 200 characters.
pub fn clean_filename(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let cleaned: String = base
        .chars()
        .filter(|c| !c.is_control())
        .take(200)
        .collect::<String>()
        .trim()
        .trim_start_matches('.')
        .to_string();
    if cleaned.is_empty() {
        "file".into()
    } else {
        cleaned
    }
}

/// The local store of attachment bytes, by content hash.
#[derive(Debug, Clone)]
pub struct BlobStore {
    root: PathBuf,
}

impl BlobStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Where the bytes with `hash` are (or would be) stored.
    pub fn path(&self, hash: &str) -> Result<PathBuf> {
        let hex =
            hash_hex(hash).ok_or_else(|| Error::Invalid(format!("not a content hash: {hash}")))?;
        Ok(self.root.join("sha256").join(&hex[..2]).join(hex))
    }

    pub fn contains(&self, hash: &str) -> bool {
        self.path(hash).is_ok_and(|p| p.is_file())
    }

    /// Stores `bytes` and returns their hash. Storing the same bytes again
    /// does nothing. The file is written beside its final place and renamed
    /// into it, so a crash never leaves a partial file under a hash.
    pub fn put(&self, bytes: &[u8]) -> Result<String> {
        let hash = content_hash(bytes);
        let path = self.path(&hash)?;
        if path.is_file() {
            return Ok(hash);
        }
        let dir = path.parent().expect("blob paths have a folder");
        std::fs::create_dir_all(dir)?;
        let partial = dir.join(format!(".{}.partial", Uuid::now_v7()));
        let written = (|| {
            let mut file = std::fs::File::create(&partial)?;
            file.write_all(bytes)?;
            file.sync_all()?;
            std::fs::rename(&partial, &path)
        })();
        if let Err(e) = written {
            let _ = std::fs::remove_file(&partial);
            return Err(e.into());
        }
        Ok(hash)
    }

    /// The bytes with `hash`, checked against it, so a damaged file is
    /// reported rather than shown or sent.
    pub fn read(&self, hash: &str) -> Result<Vec<u8>> {
        let path = self.path(hash)?;
        let bytes = std::fs::read(&path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => Error::NotFound("attached file"),
            _ => e.into(),
        })?;
        if content_hash(&bytes) != hash {
            return Err(Error::Corrupt(format!(
                "the stored file {hash} has changed"
            )));
        }
        Ok(bytes)
    }

    /// Removes the bytes with `hash`. Callers check first that nothing
    /// refers to them any more.
    pub fn remove(&self, hash: &str) -> Result<()> {
        match std::fs::remove_file(self.path(hash)?) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_standard_sha256_test_vector() {
        assert_eq!(
            content_hash(b"abc"),
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(hash_hex(&content_hash(b"abc")).is_some());
        assert!(hash_hex("sha256:../../etc/passwd").is_none());
        assert!(hash_hex("md5:abc").is_none());
    }

    #[test]
    fn identical_content_has_identical_hashes() {
        assert_eq!(content_hash(b"# Notes"), content_hash(b"# Notes"));
        assert_ne!(content_hash(b"# Notes"), content_hash(b"# notes"));
    }

    #[test]
    fn files_are_recognised_by_content_then_name() {
        let png = b"\x89PNG\r\n\x1a\n rest";
        assert_eq!(
            classify("x.bin", "", png),
            ("image/png".into(), AttachmentKind::Image)
        );
        assert_eq!(
            classify("paper.pdf", "application/pdf", b"%PDF-1.7 ..."),
            ("application/pdf".into(), AttachmentKind::Pdf)
        );
        assert_eq!(
            classify("notes.md", "", b"# Notes"),
            ("text/markdown".into(), AttachmentKind::Text)
        );
        assert_eq!(
            classify("notes.md", "application/octet-stream", b"# Notes"),
            ("text/markdown".into(), AttachmentKind::Text)
        );
        assert_eq!(
            classify("data.csv", "text/csv; charset=utf-8", b"a,b"),
            ("text/csv".into(), AttachmentKind::Text)
        );
        assert_eq!(
            classify("main.rs", "", b"fn main() {}"),
            ("text/x-rust".into(), AttachmentKind::Text)
        );
        // Names and reported types do not make something an image or text.
        assert_eq!(
            classify("fake.png", "image/png", b"not a png"),
            ("application/octet-stream".into(), AttachmentKind::Other)
        );
        assert_eq!(
            classify("binary.txt", "text/plain", &[0xff, 0xfe, 0x00]),
            ("text/plain".into(), AttachmentKind::Other)
        );
        assert_eq!(
            classify(
                "report.docx",
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                b"PK.."
            ),
            (
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document".into(),
                AttachmentKind::Other
            )
        );
    }

    #[test]
    fn file_names_cannot_escape_or_hide() {
        assert_eq!(clean_filename("../../secret.txt"), "secret.txt");
        assert_eq!(clean_filename("C:\\Users\\me\\notes.md"), "notes.md");
        assert_eq!(clean_filename(".hidden"), "hidden");
        assert_eq!(clean_filename("a\u{0}b\nc.md"), "abc.md");
        assert_eq!(clean_filename(""), "file");
    }

    #[test]
    fn the_store_deduplicates_and_checks_what_it_reads() {
        let dir = tempfile::TempDir::new().unwrap();
        let store = BlobStore::new(dir.path());
        let hash = store.put(b"# Notes").unwrap();
        assert_eq!(store.put(b"# Notes").unwrap(), hash);
        let path = store.path(&hash).unwrap();
        assert!(path.starts_with(dir.path().join("sha256")));
        assert_eq!(store.read(&hash).unwrap(), b"# Notes");
        let files: Vec<_> = std::fs::read_dir(path.parent().unwrap()).unwrap().collect();
        assert_eq!(files.len(), 1, "no partial files are left behind");

        std::fs::write(&path, b"tampered").unwrap();
        assert!(matches!(store.read(&hash), Err(Error::Corrupt(_))));
        store.remove(&hash).unwrap();
        store.remove(&hash).unwrap();
        assert!(matches!(store.read(&hash), Err(Error::NotFound(_))));
        assert!(store.path("sha256:../../x").is_err());
    }
}

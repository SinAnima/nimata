//! The full archive (`nimata-archive/1`, a zip of every discussion) and
//! reading files to import: a Nimata discussion, a Nimata archive, or a
//! ChatGPT export. See docs/archive-format.md.

use std::io::{Read, Seek, Write};

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::archive::{DiscussionArchive, FORMAT, format_utc};
use crate::chatgpt;
use crate::error::{Error, Result};
use crate::time::UnixMillis;

pub const ARCHIVE_FORMAT: &str = "nimata-archive/1";

/// Largest file read from inside a zip, so a damaged or hostile zip cannot
/// exhaust memory.
const MAX_ENTRY_BYTES: u64 = 1 << 30;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub format: String,
    pub exported_at: String,
    pub discussions: Vec<ManifestEntry>,
    /// Content hashes of files under `attachments/` (from Stage 8).
    #[serde(default)]
    pub attachments: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestEntry {
    pub id: Uuid,
    pub title: String,
    /// Path of its `nimata/1` file inside the zip.
    pub path: String,
    pub posts: usize,
}

/// Every discussion, active and archived, as archives. Deleted
/// discussions are left out.
pub fn export_all<R: crate::Repository + ?Sized>(
    repo: &mut R,
    exported_at: UnixMillis,
) -> Result<Vec<DiscussionArchive>> {
    use crate::domain::DiscussionFilter;
    let me = repo.local_user()?.id;
    let mut ids = Vec::new();
    for filter in [DiscussionFilter::Active, DiscussionFilter::Archived] {
        ids.extend(repo.list_discussions(filter)?.into_iter().map(|d| d.id));
    }
    ids.into_iter()
        .map(|id| {
            let view = repo.get_discussion(id)?;
            let revisions = repo.discussion_revisions(id)?;
            Ok(DiscussionArchive::new(&view, &revisions, me, exported_at))
        })
        .collect()
}

/// Writes every discussion into a zip: `manifest.json` and
/// `discussions/<id>.json`.
pub fn write_archive<W: Write + Seek>(
    out: W,
    discussions: &[DiscussionArchive],
    exported_at: UnixMillis,
) -> Result<()> {
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut zip = ZipWriter::new(out);
    let mut entries = Vec::new();
    for archive in discussions {
        let path = format!("discussions/{}.json", archive.discussion.id);
        zip.start_file(&path, options).map_err(zip_error)?;
        zip.write_all(archive.to_json().as_bytes())?;
        entries.push(ManifestEntry {
            id: archive.discussion.id,
            title: archive.discussion.title.clone(),
            path,
            posts: archive.posts.len(),
        });
    }
    let manifest = Manifest {
        format: ARCHIVE_FORMAT.into(),
        exported_at: format_utc(exported_at),
        discussions: entries,
        attachments: vec![],
    };
    zip.start_file("manifest.json", options)
        .map_err(zip_error)?;
    zip.write_all(
        serde_json::to_string_pretty(&manifest)
            .expect("the manifest always serializes")
            .as_bytes(),
    )?;
    zip.finish().map_err(zip_error)?;
    Ok(())
}

fn zip_error(e: zip::result::ZipError) -> Error {
    Error::Invalid(format!("the zip file could not be read or written: {e}"))
}

fn read_entry<R: Read + Seek>(zip: &mut ZipArchive<R>, name: &str) -> Result<Option<String>> {
    let file = match zip.by_name(name) {
        Ok(file) => file,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(e) => return Err(zip_error(e)),
    };
    let mut text = String::new();
    file.take(MAX_ENTRY_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(|e| Error::Invalid(format!("{name} in the zip is not readable text: {e}")))?;
    if text.len() as u64 > MAX_ENTRY_BYTES {
        return Err(Error::Invalid(format!(
            "{name} in the zip is too large to import"
        )));
    }
    Ok(Some(text))
}

fn read_archive<R: Read + Seek>(
    zip: &mut ZipArchive<R>,
    manifest: &str,
) -> Result<Vec<DiscussionArchive>> {
    let manifest: Manifest = serde_json::from_str(manifest)
        .map_err(|e| Error::Invalid(format!("the archive's manifest.json is not valid: {e}")))?;
    if manifest.format != ARCHIVE_FORMAT {
        return Err(Error::Invalid(format!(
            "unsupported archive format {:?}; this version reads {ARCHIVE_FORMAT:?}",
            manifest.format
        )));
    }
    manifest
        .discussions
        .iter()
        .map(|entry| {
            let json = read_entry(zip, &entry.path)?.ok_or_else(|| {
                Error::Invalid(format!(
                    "the archive lists {} but does not contain it",
                    entry.path
                ))
            })?;
            let archive = DiscussionArchive::from_json(&json)
                .map_err(|e| Error::Invalid(format!("{}: {e}", entry.path)))?;
            if archive.discussion.id != entry.id {
                return Err(Error::Invalid(format!(
                    "{} holds a different discussion than the manifest says",
                    entry.path
                )));
            }
            Ok(archive)
        })
        .collect()
}

/// What kind of file was imported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ImportSource {
    /// One discussion exported as `nimata/1` JSON.
    NimataDiscussion,
    /// A `nimata-archive/1` zip.
    NimataArchive,
    /// ChatGPT's data export: `conversations.json`, or the zip holding it.
    ChatGpt,
}

/// Reads a file chosen for import, recognising it by its contents.
pub fn read_import(bytes: &[u8]) -> Result<(ImportSource, Vec<DiscussionArchive>)> {
    if bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06") {
        let mut zip = ZipArchive::new(std::io::Cursor::new(bytes)).map_err(zip_error)?;
        if let Some(manifest) = read_entry(&mut zip, "manifest.json")? {
            return Ok((
                ImportSource::NimataArchive,
                read_archive(&mut zip, &manifest)?,
            ));
        }
        if let Some(json) = read_entry(&mut zip, "conversations.json")? {
            return Ok((ImportSource::ChatGpt, chatgpt::conversations(&json)?));
        }
        return Err(Error::Invalid(
            "this zip is neither a Nimata archive nor a ChatGPT export \
             (it has no manifest.json or conversations.json)"
                .into(),
        ));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| {
        Error::Invalid("this file is not a Nimata export or a ChatGPT export".into())
    })?;
    let value: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|_| {
            Error::Invalid("this file is not a Nimata export or a ChatGPT export".into())
        })?;
    match &value {
        serde_json::Value::Array(_) => Ok((ImportSource::ChatGpt, chatgpt::conversations(text)?)),
        serde_json::Value::Object(map)
            if map.get("format").and_then(|f| f.as_str()) == Some(FORMAT) =>
        {
            Ok((
                ImportSource::NimataDiscussion,
                vec![DiscussionArchive::from_json(text)?],
            ))
        }
        serde_json::Value::Object(map) if map.contains_key("format") => {
            Err(Error::Invalid(format!(
                "unsupported format {}; this version reads {FORMAT:?}",
                map["format"]
            )))
        }
        _ => Err(Error::Invalid(
            "this file is not a Nimata export or a ChatGPT export".into(),
        )),
    }
}

/// What importing one discussion did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ImportResult {
    /// It was new here.
    Added,
    /// It was here already; posts it did not have were added.
    Updated,
    /// Everything in it was here already.
    Unchanged,
    /// It was deleted on this device, so it was left deleted.
    SkippedDeleted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportOutcome {
    pub discussion_id: Uuid,
    pub title: String,
    pub result: ImportResult,
    pub posts_added: usize,
    /// Posts that were already here, by ID, and left unchanged.
    pub posts_present: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::{ArchivedDiscussion, ArchivedParticipant, ArchivedPost};
    use crate::domain::{ParticipantKind, PostStatus};

    fn discussion(title: &str) -> DiscussionArchive {
        let me = Uuid::now_v7();
        DiscussionArchive {
            format: FORMAT.into(),
            exported_at: "2026-10-07T10:00:00.000Z".into(),
            discussion: ArchivedDiscussion {
                id: Uuid::now_v7(),
                title: title.into(),
                created_at: "2026-10-04T16:41:07.123Z".into(),
                archived_at: None,
            },
            participants: vec![ArchivedParticipant {
                id: me,
                kind: ParticipantKind::Human,
                display_name: "Thanos".into(),
                provider: None,
                model: None,
                local_user: true,
            }],
            posts: vec![ArchivedPost {
                id: Uuid::now_v7(),
                parent_id: None,
                author_id: me,
                created_at: "2026-10-04T12:41:07.123-04:00".into(),
                status: PostStatus::Complete,
                body: title.into(),
                edited_at: None,
                deleted_at: None,
                revisions: vec![],
                provider_metadata: None,
                context_ids: vec![],
            }],
        }
    }

    fn zip_of(files: &[(&str, &str)]) -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        let mut zip = ZipWriter::new(&mut out);
        for (name, body) in files {
            zip.start_file(*name, SimpleFileOptions::default()).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
        out.into_inner()
    }

    #[test]
    fn an_archive_round_trips_with_a_readable_manifest() {
        let discussions = vec![discussion("One"), discussion("Two")];
        let mut out = std::io::Cursor::new(Vec::new());
        write_archive(&mut out, &discussions, UnixMillis(0)).unwrap();
        let bytes = out.into_inner();

        let (source, back) = read_import(&bytes).unwrap();
        assert_eq!(source, ImportSource::NimataArchive);
        assert_eq!(back, discussions);

        let mut zip = ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
        let manifest: serde_json::Value =
            serde_json::from_str(&read_entry(&mut zip, "manifest.json").unwrap().unwrap()).unwrap();
        assert_eq!(manifest["format"], "nimata-archive/1");
        assert_eq!(manifest["discussions"][1]["title"], "Two");
        assert_eq!(
            manifest["discussions"][0]["path"],
            format!("discussions/{}.json", discussions[0].discussion.id)
        );
    }

    #[test]
    fn single_discussions_and_unknown_files_are_recognised() {
        let one = discussion("One");
        let (source, back) = read_import(one.to_json().as_bytes()).unwrap();
        assert_eq!(source, ImportSource::NimataDiscussion);
        assert_eq!(back, vec![one]);

        for (bytes, message) in [
            (&b"\x00\x01binary"[..], "not a Nimata export"),
            (b"{\"hello\": 1}", "not a Nimata export"),
            (b"{\"format\": \"nimata/9\"}", "unsupported format"),
        ] {
            let error = read_import(bytes).unwrap_err().to_string();
            assert!(error.contains(message), "{error}");
        }
        let error = read_import(&zip_of(&[("notes.txt", "hi")]))
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("neither a Nimata archive nor a ChatGPT export"),
            "{error}"
        );
    }

    #[test]
    fn broken_archives_are_explained() {
        let one = discussion("One");
        let manifest = |path: &str, id: Uuid| {
            format!(
                r#"{{"format":"nimata-archive/1","exportedAt":"x","discussions":[{{"id":"{id}","title":"One","path":"{path}","posts":1}}]}}"#
            )
        };
        let missing = zip_of(&[(
            "manifest.json",
            &manifest("discussions/a.json", one.discussion.id),
        )]);
        assert!(
            read_import(&missing)
                .unwrap_err()
                .to_string()
                .contains("does not contain")
        );

        let wrong_id = zip_of(&[
            (
                "manifest.json",
                &manifest("discussions/a.json", Uuid::now_v7()),
            ),
            ("discussions/a.json", &one.to_json()),
        ]);
        assert!(
            read_import(&wrong_id)
                .unwrap_err()
                .to_string()
                .contains("different discussion")
        );

        let newer = zip_of(&[(
            "manifest.json",
            r#"{"format":"nimata-archive/2","exportedAt":"x","discussions":[]}"#,
        )]);
        assert!(
            read_import(&newer)
                .unwrap_err()
                .to_string()
                .contains("unsupported archive format")
        );
    }
}

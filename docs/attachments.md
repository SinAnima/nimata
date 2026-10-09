# Attachments

Files can be attached to any post you write: notes, data, source code,
papers, screenshots. They stay on this device as part of the discussion,
travel with archives and backups, and are sent to a model only when you ask
it about a post they belong to (or a post further down that thread).

## Attaching

- **Attach…** next to **Post** (and on the new discussion form) opens the
  system's file picker; on phones this is the platform picker.
- On desktop, files can also be dropped onto the composer.
- A post may be just files, without text. A discussion started with only a
  file is named after it.
- Up to 20 files per post, each up to 50 MB.
- Files waiting to be posted are kept with the draft, so they survive
  switching discussions and restarting. Removing one (×) before posting
  deletes it.

Each file shows its name, type, and size. Images appear as thumbnails;
text and Markdown files can be previewed (the first 512 KB, as text); any
file can be saved elsewhere with **Save…**.

## What models receive

Nimata does not assume every provider accepts the same files. Each file is
sorted by its contents (not just its name) into one of four kinds:

| Kind  | Recognised by                                                  | OpenAI | Anthropic | OpenAI-compatible |
| ----- | -------------------------------------------------------------- | ------ | --------- | ----------------- |
| Text  | UTF-8 text: Markdown, plain text, CSV, JSON, YAML, source code | inline | inline    | inline            |
| Image | PNG, JPEG, GIF, or WebP signature                              | yes    | yes       | no                |
| PDF   | `%PDF-` signature                                              | yes    | yes       | no                |
| Other | everything else (Word, zip, audio, ...)                        | no     | no        | no                |

- **Text files** go to every model, inside the message of the post they are
  attached to:
  `<attachment name="notes.md" type="text/markdown"> … </attachment>`.
  Files over 200,000 characters are not sent.
- **Images and PDFs** are sent in each provider's own format: OpenAI
  `input_image` and `input_file`, Anthropic `image` and `document` blocks.
  Limits: images up to 20 MB (OpenAI) or 5 MB (Anthropic); PDFs up to
  32 MB (OpenAI) or 30 MB (Anthropic).
- **OpenAI-compatible servers** receive text files only: servers differ, and
  most local models read text only.
- A file a model cannot take is **named** in its message ("Attached file not
  included: report.docx … You cannot read this kind of file"), so the model
  knows it exists, and is listed under **Files not sent** in **What will be
  sent?** and in a reply's record of what was sent.

While writing, the composer warns when the model about to answer cannot
read one of the files. Files count towards the size estimate (about 1,600
tokens per image; PDFs by size) and are trimmed with their post when a
long thread is shortened.

## Storage

Bytes are kept outside the database, in `blobs/sha256/ab/<hash>` in the data
folder, named by their SHA-256 hash:

- the same file attached several times is stored once;
- each file is written beside its final place and renamed into it, so a
  crash never leaves a partial file under a hash;
- every read is checked against the hash, so a damaged file is reported, not
  shown or sent.

The database holds the metadata (see
[data-model.md](data-model.md#attachments-schema-version-7)).

**Deleting** a post deletes its attachments' records, and their bytes once
no other post or draft uses them. Deleting a discussion does the same for
all of its posts.

## Export, import, backup

- **Archives** (`nimata-archive/1`) carry every attached file under
  `attachments/<hex>`; each post lists its files. Importing checks every
  file against its hash, stores it, and links it to its post. Files whose
  bytes are not in the archive are left out and counted in the import
  summary. See [archive-format.md](archive-format.md).
- **One-discussion JSON** lists attachments but cannot carry their bytes;
  use an archive to move files.
- **Markdown** exports list each post's files by name, type, and size.
- **Backups** hold the files too: the backup file includes a copy of every
  attached file, so one file still restores everything.

## Not yet

- Attachments are not searched.
- No file picker for photos from the camera roll beyond the system picker
  (Stage 9 reviews the mobile flow).
- OpenAI-compatible connections cannot yet be marked as able to read
  images.

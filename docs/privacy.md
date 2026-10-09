# Privacy

Nimata handles personal material, so its defaults favour keeping it on the
device.

- **Local first.** Discussions live in a SQLite database on this device.
  There is no Nimata account, server, or hosted database.
- **No telemetry.** Nimata sends nothing about how it is used.
- **Models see only what they are asked about.** When you ask a model to
  reply, only the thread down to that post is sent to its provider (see
  [providers.md](providers.md#what-a-model-is-sent)). OpenAI requests ask
  OpenAI not to store the conversation. With an OpenAI-compatible
  connection, the thread goes to the server you configured.
- **Search is local.** The search index lives inside the same database, and
  searches never leave the device. Recently used searches are remembered
  there too, and can be cleared from the search field.
- **Attached files stay here.** They are stored in the data folder and
  sent to a provider only as part of a request to a model, with the posts
  they belong to. Deleting a post deletes its files once nothing else uses
  them.
- **Exports and backups are yours.** They are written only where you
  choose.

## API keys

API keys are kept in the operating system's credential store, never in
Nimata's files:

| Platform   | Store                                                   |
| ---------- | ------------------------------------------------------- |
| macOS, iOS | Keychain                                                |
| Windows    | Credential Manager                                      |
| Android    | Not yet supported (Stage 9); keys cannot be saved there |

- Each provider's key is stored under the service `org.nimata.app`.
- Only Rust reads a key, and only to send a request to that provider. The
  interface is told whether a key is saved, never the key itself.
- Keys are never written to the database, backups, exports, or logs.
  Restoring a backup on another device means entering keys again.
- **Release builds** show only "Saved in the Keychain".
- **Development builds** also show the key's last four characters, and,
  when no key is saved, use `OPENAI_API_KEY` or `ANTHROPIC_API_KEY` from
  the environment (labelled in Settings). A saved key always takes
  precedence. OpenAI-compatible connections never read the environment.
- OpenAI-compatible connections may have no key at all; a local server
  keeps the whole exchange on your machine.

Unsigned development builds may see repeated macOS Keychain prompts after
each rebuild, because macOS identifies the app by its code signature.

# Developer Bridge

Voice Flow exposes local automation through the same backend command service as
the desktop UI. The desktop application must be running before a CLI request is
sent.

## Bundled CLI

The main executable includes CLI mode, so the command remains available in the
installed bundle without a separate sidecar:

```bash
# macOS application bundle
"/Applications/Voice Flow.app/Contents/MacOS/voiceflow" --cli status

# Windows installation directory
voiceflow.exe --cli status
```

For development, the standalone Cargo binary offers the same protocol:

```bash
cargo run --manifest-path apps/desktop/src-tauri/Cargo.toml \
  --bin voiceflow-cli -- status
```

Both forms print one JSON response and return a non-zero exit status for a
rejected or failed command.

## Commands

```text
start [profile-id]
stop
cancel
status
submit
insert TEXT
transcribe-file PATH [profile-id]
copy-last [raw|final]
reinsert-last [raw|final]
open voiceflow://COMMAND?ARGUMENTS
clear-code-context
```

Text for code formatting and structured editor context is read from standard
input. This avoids shell-escaping source paths, symbols, and multiline text:

```bash
printf '%s' '{"language":"rust","file_path":"src/main.rs","symbol":"HTTPServer","editor_id":"dev.zed.Zed"}' \
  | voiceflow.exe --cli code-context

printf '%s' 'cargo test dash dash workspace' \
  | voiceflow.exe --cli format-code rust
```

The editor context fields are optional. Supported fields are `language`,
`file_path`, `symbol`, and `editor_id`.

## URL scheme

The native bundles register `voiceflow://`. These examples route through the
same dispatcher as CLI and UI requests:

```text
voiceflow://start?profile=code
voiceflow://status
voiceflow://transcribe-file?path=%2Ftmp%2Fsample.wav
voiceflow://copy-last?version=final
voiceflow://code-context?language=rust&file=src%2Fmain.rs&symbol=HTTPServer&editor=dev.zed.Zed
```

Unknown commands, missing arguments, and malformed percent encoding are
rejected before dispatch.

## Security and privacy

- The bridge binds to an ephemeral `127.0.0.1` port only.
- Each application launch creates a new random token.
- The endpoint file is user-private (`0600` on Unix) and contains the loopback
  address, process ID, protocol version, and token.
- Every request is size-bounded and token-authenticated.
- Logs contain command names, result status, and errors, but never inserted or
  transcribed content.
- The bridge is unavailable when Voice Flow is not running.

Do not copy the endpoint token into editor settings or source control. Adapters
should invoke the bundled CLI and let it discover the current endpoint.

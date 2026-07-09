---
name: verify
description: Build, launch, and drive sinclairdev to verify rendering/behavior changes end-to-end on macOS.
---

# Verifying Sinclair changes on the live app

## Build + launch

```sh
cargo build -p app                       # debug is fine, ~5s incremental
target/debug/sinclairdev >/tmp/sinclairdev.log 2>&1 &
```

The dev binary runs side by side with an installed `sinclair` (own socket,
own window). Kill with `pkill -x sinclairdev` when done.

## Drive it: the MCP bridge

`sinclairdev mcp` is a stdio JSON-RPC bridge into the *running* GUI over the
single-instance socket. Pipe newline-delimited JSON-RPC; keep stdin open long
enough to collect responses:

```sh
{ cat calls.jsonl; sleep 2; } | target/debug/sinclairdev mcp
```

`calls.jsonl` — initialize first, then tool calls:

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"verify","version":"0"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"run_command","arguments":{"text":"echo hi"}}}
```

Useful tools: `run_command` (type a shell command into the focused pane),
`send_input` (raw bytes, no newline), `read_screen` (scrollback + grid text),
`split` (`direction`: right|down).

## Rendering tricks

- Paint test cells and park the cursor on them with printf + a sleep so the
  prompt doesn't repaint over the scenario:
  `printf '\n\033[48;2;255;255;255m          \033[0m\033[5D'; sleep 180`
- OSC 12 sets the app cursor color: `\033]12;rgb:ff/ff/ff\007`.
- `split` moves focus to the new pane — the old pane shows the unfocused
  (hollow) cursor and the unfocused-split dimming.

## Observe

`screencapture -x out.png` works on this host (screen recording permission is
granted). `-R x,y,w,h` takes screen points (half the retina pixel size) for
region crops. `read_screen` gives text but no colors — cursor/color work needs
the screenshots.

# prompt roadmap

Prompt is a terminal emulator written in Rust.

- Feature target: parity with Ghostty (https://ghostty.org/docs)
- Architecture: Zed-style (https://github.com/zed-industries/zed) — a cargo
  workspace of small, focused crates under `crates/`, GPUI for the UI layer.

Conventions (non-negotiable):

- File names: all lowercase, never use spaces, `-`, or `_`. Multi-word concepts
  become nested module directories (`src/grid/scrollback.rs`, never
  `src/grid_scrollback.rs`).
- Small, hyper-focused files.
- Functional style: free functions and plain structs/enums; avoid trait-object
  heavy designs unless genuinely needed.
- Each crate declares its own dependencies in its own `Cargo.toml`.

## Phases

- [x] **1. Core terminal model** — `vt` crate: vte-based parser handling,
  cell/color/sgr, grid + scrollback, primary/alt screens, scroll regions, tab
  stops, cursor save/restore, charsets, modes (DECAWM, DECTCEM, DECOM, 1049,
  2004), OSC title, DSR responses. `pty` crate: openpty, shell spawn, resize,
  read/write. `config` crate: Ghostty-style `key = value` config file. `theme`
  crate: ANSI palette, truecolor, builtin schemes. All unit-tested,
  `cargo test --workspace` green.
- [x] **2. Event loop integration** — wire pty output through vt in a
  background thread; terminal state behind a lock; damage tracking; headless
  integration tests (spawn shell, run commands, assert grid contents).
- [x] **3. GPUI app shell** — depend on gpui (zed repo git dep); window,
  monospace grid text rendering, cursor, keyboard input encoding (incl.
  modifiers) written to pty, resize handling.
- [x] **4. Selection & clipboard** — mouse selection (normal/word/line),
  copy/paste, bracketed paste, scrollback scrolling, scroll-to-bottom on
  input. Pulled forward from phase 7: SGR/X10 mouse reporting (1000/1002/
  1003/1006) since app-side mouse plumbing lands here, with shift-bypass
  for selection.
- [x] **5. Tabs & splits** — Zed-style workspace/pane tree model, keybindings,
  pane resize, tab bar UI.
- [x] **6. Config & themes deep pass** — live reload, all Ghostty-style core
  options (fonts, padding, cursor style, shell, keybind = trigger=action,
  window opts), theme option + builtin scheme library. (Deferred to later
  phases: font-feature application → phase 8; GUI runtime verification of
  keybind dispatch.)
- [x] **7. Advanced VT** — mouse reporting modes (phase 4), OSC 52
  clipboard, OSC 4/10/11/12 color queries, focus reporting ?1004,
  synchronized output ?2026 (with a 150ms stuck-sync safety timeout in the
  app), DA2, OSC 8 hyperlinks (vt model + cmd-click-to-open + underline),
  kitty keyboard protocol (vt negotiation + input disambiguation encoder),
  XTGETTCAP. Kitty is press-only (no release/repeat events from the host,
  so event-type/alternate-key/text flags are tracked but not encoded).
- [x] **8. Fonts** — font fallback chain (`font-family` repeats), emoji
  (via the fallback chain + system fallback), ligatures (contiguous-run
  shaping + `calt`), font-feature config (`+liga`/`-calt`/`ss01`/`cv01=2`
  → gpui FontFeatures), box-drawing glyph rendering (custom geometry for
  light lines/junctions, block elements, shades, eighth blocks).
  Remaining for a later polish pass: heavy/double/dashed/rounded box lines
  (currently fall back to font glyphs).
- [x] **9. Shell integration** — OSC 133 semantic prompt marking (A marks
  prompt rows, tracked into scrollback), OSC 7 cwd reporting (phase 6),
  jump-to-prompt (`jump_to_prompt:N` action, default cmd+up/down).
  Deferred: automatic shell-integration *script* injection (the bash/zsh/
  fish hooks that emit these sequences) is shell-side packaging, not
  terminal logic.
- [x] **10. Images & extras** — URL detection (`vt::url`, cmd-click opens a
  detected URL when there is no OSC 8 link), search in scrollback
  (`vt::search` engine + an in-app overlay: cmd+f, live highlight, n/N
  navigation, jump-to-match). Image protocols: kitty graphics (APC) and
  sixel (DCS) are consumed without corrupting the screen (vte swallows
  them); actual image *rendering* — pixel decode + GPU compositing — is a
  large deferred follow-up, not yet drawn.
- [x] **11. Performance & polish** — parser throughput benchmark
  (`vt/tests/throughput.rs`, ~103 MiB/s baseline on this machine), snapshot
  buffer pre-sizing, idle repaints already avoided (notify-driven render).
  Identified next optimization: per-row damage-clipped shaping (vt damage
  tracking exists from phase 2; the renderer currently drains it and
  re-shapes the full viewport per frame).
- [x] **12. Ghostty parity audit** — feature-by-feature coverage map in
  `docs/parity.md`, with implemented areas, partial areas, and the
  remaining gaps prioritized.
- [x] **13. Plugin foundation** — Zed-style manifest plugins under
  `~/.config/prompt/plugins/*/plugin.toml` (plus explicit `plugin = path`
  config entries). Plugins contribute command actions with optional default
  keybindings, resolved through the same keymap pipeline as built-ins. Command
  targets: focused pane, new tab, right split, down split. Deferred: richer
  extension hosts for themes, languages, and UI surfaces.

## Status log

- 2026-06-12: workspace scaffolded; phase 1 implementation started.
- 2026-06-12: phase 1 complete — vt/pty/config/theme implemented, 196 tests
  green. Phase 2 started: `terminal` crate (pty↔vt runtime) + damage tracking
  in vt.
- 2026-06-12: phase 2 complete — `terminal` runtime + vt damage/bell/title
  signals, 229 tests green. Toolchain updated to rustc 1.96. gpui build recipe
  proven (see gpui.md). Phase 3 started: gpui app shell + `input` crate.
- 2026-06-12: phase 3 complete — windowed terminal works end to end, 283
  tests green, launch smoke test passed, rendering visually verified
  (colors, title, cursor, real zsh session). Phase 4 started: selection,
  clipboard, scrollback scrolling, mouse reporting.
- 2026-06-12: phase 4 complete — selection (cell/word/line), cmd-c +
  copy-on-select, scrollback viewing with stable offset, SGR/X10 mouse
  reporting with shift-bypass, alternate scroll. 351 tests green across 15
  suites, smoke test clean. Phase 5 started: `workspace` crate (pane tree)
  + tabs/splits in app.
- 2026-06-12: phase 5 complete — tabs + splits live (pane tree crate,
  tab bar, divider drag, directional focus, cmd-t/w/d bindings, per-pane
  sessions). 395 tests green across 17 suites. Phase 6 started: config
  deep pass (full option set, keybind parsing, live reload) + builtin
  scheme library + app application of all of it.
- 2026-06-15: phase 6 complete — live config reload wired into the app
  (`app/reload.rs` bridges the config watcher to the gpui foreground;
  `WorkspaceView::reload` rebuilds colors/font/size/padding/cursor/
  copy-on-select and pushes an `Appearance` to every pane). Config
  keybindings now drive the app: hardcoded gpui bindings removed,
  `config::resolve` feeds a `RunBind(index)` data action through one
  dispatcher (`app/keys.rs` maps triggers → gpui keystrokes, unit-tested;
  `clear_key_bindings` + rebind on reload). New actions wired: font
  size inc/dec/reset (live re-measure), scroll page/top/bottom, clear
  screen (form feed), new_split left/up, goto_split prev/next, goto_tab
  (incl. negative), move_tab, toggle_fullscreen, copy/paste via focused
  pane. 460 tests green. Pending: live GUI verification of dispatch;
  font-feature application (phase 8).
- 2026-06-15: phase 7 batch 1 — advanced VT replies/modes landed in `vt`
  (pure, unit-tested): focus reporting (?1004) with `report_focus`/
  `focus_reporting`; synchronized output (?2026) with `synchronized_output`
  accessor; OSC 52 clipboard set (base64 decode in `term/report.rs`,
  surfaced as `terminal::Event::Clipboard`, written to the system
  clipboard by the pane); OSC 4/10/11/12 color queries answered from
  host-installed `ReportColors` (`set_report_colors`, fed from the theme on
  spawn + reload); DA2. App wiring: focus in/out listeners drive ?1004;
  pane installs report colors; clipboard event writes to gpui clipboard.
  475 tests green. Remaining phase 7: OSC 8 hyperlinks, kitty keyboard
  protocol, XTGETTCAP; app frame-gating for ?2026 (needs a stuck-sync
  timeout).
- 2026-06-15: phase 7 batch 2 — OSC 8 hyperlinks and kitty keyboard.
  Hyperlinks: `vt/hyperlink.rs` interns links; `Cell` gains a 2-byte
  `Option<HyperlinkId>` (NonZeroU16 niche); OSC 8 open/close sets the pen
  link; `Terminal::cell_hyperlink`; RIS clears the registry. App: cmd-click
  on a linked cell calls `cx.open_url` (visual underline still TODO).
  Kitty: `vt/kitty.rs` per-screen flag stack with `CSI >/=/< u` push/set/
  pop + `CSI ? u` query + `kitty_keyboard_flags()` accessor (per-screen,
  RIS-reset, all tested); `input/kitty.rs` encodes the disambiguated keys
  (Escape always; ctrl/alt or all-keys text → `CSI codepoint;mods u`),
  deferring everything else to legacy. Limits: press-only (no release/
  repeat events from gpui, so event-type/alternate-key/text flags are
  tracked but not encoded); nav/function keys keep legacy spellings.
  `TermState` gains `kitty_flags`, fed from the terminal. 496 tests green.
- 2026-06-15: phase 7 complete — XTGETTCAP via DCS (`vt/term/dcs.rs`,
  hook/put/unhook; answers Co/colors, TN, RGB, and common booleans; hex
  helpers in report.rs); ?2026 frame-gating in the pane (withhold repaint
  while synchronized, 150ms stuck-sync safety timer via gpui timer+spawn);
  OSC 8 link underline in the element snapshot (linked cells get UNDERLINE,
  spaces included). 503 tests green. Phase 8 (fonts) next.
- 2026-06-15: phase 8 complete — fonts. `font-family` now accumulates into
  a fallback chain (`config::Options::primary_font`/`font_fallbacks`);
  `app/font.rs` builds the gpui Font with primary + `FontFallbacks` +
  `FontFeatures` (parsing `+liga`/`-calt`/`ss01`/`cv01=2`) + base style,
  used by startup and live reload (closes the phase-6 font-feature TODO).
  Emoji ride the fallback chain + system fallback; ligatures work via
  same-style run shaping plus `calt`. `app/boxdraw.rs` renders box-drawing
  and block glyphs with custom quad geometry (light lines ─│┌┐└┘├┤┬┴┼,
  full/half blocks, shades ░▒▓, lower/left eighth blocks); the element
  snapshot diverts these cells to `box_quads`. 517 tests green. Phases
  1–8 done; remaining: 9 (shell integration), 10 (images/extras),
  11 (perf), 12 (parity audit).
- 2026-06-15: phases 9–12. Phase 9: OSC 133;A prompt marking on `Row`
  (travels into scrollback), `Terminal::prompt_lines`, `jump_to_prompt:N`
  action (cmd+up/down). Phase 10: `vt::url` detection (cmd-click opens a
  bare URL), `vt::search` engine + cmd+f overlay with live highlight and
  n/N navigation; kitty/sixel image sequences swallowed without corruption
  (rendering deferred). Phase 11: `vt/tests/throughput.rs` benchmark
  (~103 MiB/s), snapshot buffer pre-sizing. Phase 12: `docs/parity.md`
  full coverage map. All phases 1–12 complete; remaining work is the
  prioritized gap list in parity.md (chief: image rendering, resize
  reflow, damage-clipped paint). Test count 529 green.
- 2026-06-15: configuration UX. Native macOS menu bar (`set_menus`,
  items reuse config actions). Settings panel (cmd+,) writes changes back
  to the config file via `config::upsert` and live-reloads. Added a
  reusable text-input model (`app/textedit.rs`, tested) and wired editable
  fields into settings (font family, shell, foreground, background) and the
  search query (caret + cursor keys). Keymap now emits gpui's `secondary`
  modifier for `cmd`, so every ⌘ binding is ⌘ on macOS / Ctrl on
  Linux+Windows. 539 tests green.
- 2026-06-18: command macros + MCP server. New `macros` crate: a keystroke
  `Recorder` (captures typed command lines, segmented on Enter) and plain
  per-macro text storage under `~/.config/prompt/macros`. Actions
  `macro_record` (toggle capture; names/saves via the rename modal) and
  `macro:<name>` (replay into the focused pane, paced off OSC 133 prompt
  marks with a fixed-delay fallback). New `mcp` crate: a dependency-light
  Model Context Protocol server over stdio (`initialize`/`tools/list`/
  `tools/call`). `prompt mcp` bridges tool calls to the running instance over
  the existing single-instance socket (`ipc.rs` grew a JSON request/response
  protocol; `mcpbridge.rs` maps ops onto the active `WorkspaceView`). Tools:
  run_command, read_screen, list_macros, run_macro, list_tabs, focus_tab.
  575 tests green.
- 2026-06-23: UI, working-directory, and packaging pass (v0.3.0). About panel
  (`app/about.rs`): app menu → About Prompt shows the icon, version, and the
  release date (stamped from the HEAD commit by `app/build.rs`), plus a repo
  link. Zed-style window chrome: the window opens with a transparent native
  title bar and `app/titlebar.rs` draws the strip — tabs folded in, drag-to-move
  (`start_window_move`), macOS traffic-light inset, and on Linux app-drawn
  minimize/maximize/close controls + resize edges (client-side decorations).
  Working directory: shells default to `$HOME` (was inheriting the launcher's
  cwd, e.g. `/` from Finder), and new windows now inherit the focused pane's cwd
  like new tabs/splits already did. Tests relocated: every crate's unit tests
  moved from inline `#[cfg(test)]` modules into a sibling `tests/` directory,
  compiled in-crate via `#[path]` includes with `autotests = false`; genuine
  integration tests (`vt`, `terminal`) declared as explicit `[[test]]` targets.
  Linux releases: `scripts/linux.sh` builds `.tar.gz`/`.deb`/`.AppImage` for
  x86_64 and aarch64, wired into the release workflow plus a `linux.yml`
  validation workflow. 583 tests green.

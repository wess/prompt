# Ghostty parity audit

A feature-by-feature map of Prompt against Ghostty (https://ghostty.org/docs),
as of phases 1–12. Status key: **✓** implemented, **◑** partial (works for
the common case, with documented limits), **✗** not yet.

## Terminal emulation (VT)

| Area | Status | Notes |
|------|--------|-------|
| C0/C1 controls, ESC dispatch | ✓ | BEL/BS/HT/LF/VT/FF/CR/SO/SI, ESC 7/8/D/E/H/M/c/=/>, SCS, DECALN |
| CSI cursor/erase/scroll/insert | ✓ | CUU…CUP, ED/EL/ECH, IL/DL/ICH/DCH, SU/SD, REP, DECSTBM |
| SGR (colors + attributes) | ✓ | 16/256/truecolor (semicolon + colon forms), underline styles, all attrs |
| Modes (DEC private + ANSI) | ✓ | DECAWM/DECTCEM/DECOM/IRM, 47/1047/1048/1049, bracketed paste |
| Charsets (G0/G1, DEC special) | ✓ | line-drawing via SCS + SO/SI |
| Scrollback + alt screen | ✓ | ring buffer, content-anchored offset, no scrollback on alt |
| Wide characters | ✓ | width 2 + spacer cells |
| Combining characters | ✗ | width-0 chars dropped (needs per-cell grapheme storage) |
| Reflow on resize | ✗ | truncate/pad only; `Row::wrapped` flag is tracked but unused |
| Damage tracking | ✓ | per-row + full-escalation (renderer does not yet clip to it) |

## Input

| Area | Status | Notes |
|------|--------|-------|
| Legacy xterm key encoding | ✓ | modifiers, cursor/tilde/function keys, app cursor/keypad |
| Mouse reporting | ✓ | X10/normal/button/any + SGR (1000/1002/1003/1006), alt-scroll |
| Bracketed paste | ✓ | |
| Kitty keyboard protocol | ◑ | negotiation + disambiguation encoding; **press-only** (no release/repeat events from the host, so event-type/alternate-key/associated-text flags are tracked but not encoded) |

## OSC / clipboard / links

| Area | Status | Notes |
|------|--------|-------|
| Title (OSC 0/2) + title stack | ✓ | |
| Palette OSC 4 / 104, cursor OSC 12 / 112 | ✓ | |
| Dynamic color *queries* (OSC 4/10/11/12 `?`) | ✓ | answered from theme via `set_report_colors` |
| OSC 7 cwd reporting | ✓ | inherited by new splits, tabs, and windows; defaults to `$HOME` when unknown |
| OSC 52 clipboard | ✓ | base64 decode → system clipboard |
| OSC 8 hyperlinks | ✓ | interned per-cell, underlined, cmd-click opens |
| URL detection (no OSC 8) | ✓ | cmd-click opens detected URLs |
| Focus reporting (?1004) | ✓ | |
| Synchronized output (?2026) | ✓ | frame-gated with a 150 ms stuck-sync timeout |
| XTGETTCAP, DA1/DA2, DSR | ✓ | |

## Shell integration

| Area | Status | Notes |
|------|--------|-------|
| Semantic prompts (OSC 133) | ✓ | A marks prompt rows (into scrollback) |
| Jump-to-prompt | ✓ | `jump_to_prompt:N`, default cmd+up/down |
| Auto-injected shell scripts | ✗ | bash/zsh/fish hooks that *emit* 133/7 are shell-side packaging |
| sudo / title helpers | ✗ | part of the shell scripts above |

## Fonts & rendering

| Area | Status | Notes |
|------|--------|-------|
| Font family + size | ✓ | live-reloadable |
| Fallback chain | ✓ | repeated `font-family` |
| Emoji | ✓ | via fallback chain + system fallback |
| Ligatures | ✓ | run shaping + `calt` |
| Font features | ✓ | `+liga`/`-calt`/`ss01`/`cv01=2` |
| Box-drawing / blocks | ◑ | light lines/junctions, blocks, shades, eighths drawn custom; heavy/double/dashed/rounded fall back to font |
| Cursor styles (DECSCUSR) | ✓ | block/bar/underline, config default |
| Images (kitty graphics / sixel) | ✗ | sequences consumed without corruption; pixel decode + GPU compositing not yet done |

## UI / workspace

| Area | Status | Notes |
|------|--------|-------|
| Tabs | ✓ | bar, activate, close, move, goto N |
| Splits | ✓ | binary tree, directional focus, divider drag |
| Selection (cell/word/line) | ✓ | copy, copy-on-select, bracketed paste |
| Scrollback view + indicator | ✓ | |
| Search in scrollback | ✓ | cmd+f overlay with editable query (caret, cursor keys), live highlight, n/N nav |
| Config (`key = value`) | ✓ | full option set, diagnostics |
| Live config reload | ✓ | theme/font/padding/cursor/keybinds |
| Settings panel (GUI) | ✓ | cmd+, modal: click controls (theme/font size+style/cursor/padding/scrollback/copy-on-select) plus editable text fields (font family, shell, foreground, background) via a built-in text-input widget; all written back to the config file |
| Text-input widget | ✓ | `textedit` model (insert/delete/cursor, unicode) + in-panel field with caret; also backs the search query |
| Keybindings (`trigger = action`) | ✓ | config-driven, defaults + user overrides + unbind |
| Themes | ✓ | 22 builtin schemes + overrides |
| Native macOS menu bar | ✓ | Prompt/Shell/Edit/View/Window menus, items reuse config actions (shortcuts shown); includes an About panel (icon, version, release date) |
| Custom window titlebar | ✓ | transparent native bar; app-drawn strip with tabs folded in and drag-to-move. macOS keeps the traffic lights; Linux draws its own minimize/maximize/close + resize edges (client-side decorations) |
| macOS status-bar (tray) item | ✗ | NSStatusBar is not exposed by the UI layer; needs custom native code |

## Prioritized remaining gaps

1. **Image rendering** (kitty graphics + sixel) — the largest missing feature;
   needs pixel decode (PNG/zlib for kitty, custom for sixel) and GPU image
   compositing in the element.
2. **Reflow on resize** — rewrap soft-wrapped lines using the `wrapped` flag.
3. **Combining characters** — per-cell grapheme storage so width-0 marks attach.
4. **Damage-clipped rendering** — shape only dirty rows for big-throughput wins.
5. **Kitty release/repeat events** — needs key-up delivery from the host layer.
6. **Heavy/double/dashed/rounded box-drawing** — extend `boxdraw` geometry.
7. **Shell-integration auto-injection** — ship + source the shell hook scripts.

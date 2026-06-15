# Prompt

A fast, modern terminal that gets out of your way.

Prompt is a GPU-accelerated terminal emulator built for people who live in the
command line. It pairs a meticulous, standards-complete terminal core with a
clean tabbed-and-split workspace, live-reloading config, and a library of
beautiful themes — so your terminal feels instant, looks great, and bends to
exactly how you work.

## Why Prompt

- **Quick.** GPU rendering and a tight event loop keep scrolling and heavy
  output buttery, even under a firehose of logs.
- **Comfortable.** Tabs and recursive splits, true-color and ligature-aware
  text, emoji, crisp box-drawing, and 22 hand-tuned themes out of the box.
- **Yours.** A single readable config file, reloaded the instant you save —
  no restart. Rebind any key, set fonts, pick a theme, tune behavior.
- **Capable.** Deep terminal support: hyperlinks, the clipboard protocol,
  bracketed paste, mouse reporting, the kitty keyboard protocol, focus and
  synchronized-output handling, and shell-integration prompt marking with
  jump-to-prompt.
- **Searchable.** Find anything in your scrollback with a live, highlighted
  in-place search.

## Highlights

- **Tabs & splits** — open tabs, split panes any direction, drag the dividers,
  and move focus by direction. Each pane is its own shell.
- **Selection & clipboard** — mouse selection by cell, word, or line;
  copy-on-select; paste with bracketing; OSC 52 clipboard support.
- **Hyperlinks & URLs** — OSC 8 links are underlined and open on ⌘-click, and
  plain URLs in output are clickable too.
- **Shell integration** — prompts are marked, the working directory follows
  into new splits and tabs, and you can jump between prompts.
- **Fonts** — primary font plus a fallback chain, emoji, programming
  ligatures, and OpenType feature controls.
- **Search** — ⌘F opens an incremental search across scrollback with live
  match highlighting and next/previous navigation.
- **Themes** — 22 built-in schemes with full per-color overrides.

## Install

On macOS, install with Homebrew:

```sh
brew install --cask wess/packages/prompt
```

Or grab the latest `Prompt.dmg` from the
[releases page](https://github.com/wess/prompt/releases) and drag it to
Applications.

## Get started

Build and launch from source:

```sh
# Launch Prompt
cargo run -p app --release
```

That's it — Prompt opens with sensible defaults. On first run it looks for a
config file (see below); if there isn't one, it uses built-in defaults.

To build a distributable macOS app yourself:

```sh
scripts/bundle.sh   # cargo build --release + assemble dist/Prompt.app
scripts/dmg.sh      # package dist/Prompt.dmg
```

See [`docs/release.md`](docs/release.md) for signing, notarization, and how
tagged releases are cut.

## Configure

Prefer a UI? Press **⌘,** for an in-app settings panel — flip themes, font
size and style, cursor, padding, scrollback, and copy-on-select with a click,
and type directly into fields for your font family, shell, and foreground /
background colors. Changes are written straight back to your config file, so
the file stays the single source of truth.

Under the hood it's a simple `key = value` file at `~/.config/prompt/config`
(or `$XDG_CONFIG_HOME/prompt/config`) that **reloads the moment you save** —
fonts, theme, padding, cursor, and keybindings all update live.

```ini
# Fonts — repeat font-family to add fallbacks (the first is primary)
font-family = JetBrains Mono
font-family = Apple Color Emoji
font-size = 14
font-feature = +liga
font-feature = +ss01

# Look
theme = catppuccin-mocha
background = #1e1e2e
cursor-style = bar
window-padding-x = 8
window-padding-y = 8

# Behavior
shell = /bin/zsh
scrollback-limit = 10000
copy-on-select = true

# Keybindings — trigger = action[:param]; use `unbind` to remove a default
keybind = cmd+shift+c=copy_to_clipboard
keybind = ctrl+shift+page_up=scroll_page_up
```

Mistakes are reported as friendly diagnostics on launch — a bad line never
stops the rest of your config from loading.

## Default keys

| Keys | Action |
|------|--------|
| ⌘T / ⌘W | New tab / close pane |
| ⌘1…⌘9 | Go to tab |
| ⌘⇧[ / ⌘⇧] | Previous / next tab |
| ⌘D / ⌘⇧D | Split right / down |
| ⌘⌥ arrows | Move focus between splits |
| ⌘C / ⌘V | Copy / paste |
| ⌘F | Search scrollback |
| ⌘↑ / ⌘↓ | Jump to previous / next prompt |
| ⌘+ / ⌘− / ⌘0 | Font size up / down / reset |
| ⌘K | Clear screen |
| ⌘, | Open settings |
| ⌘⇧, | Reload config |
| ⌘Q | Quit |

⌘ is **Command on macOS** and **Ctrl on Linux & Windows** — the same config
binding works everywhere. Every binding is a config default; override or
unbind any of them.

## Themes

22 built-in schemes, matched loosely (`Tokyo Night`, `tokyo-night`, and
`tokyonight` all work):

`dark`, `light`, `dracula`, `nord`, `gruvbox dark`, `gruvbox light`,
`solarized dark`, `solarized light`, `catppuccin latte`, `catppuccin mocha`,
`tokyo night`, `one dark`, `monokai`, `ayu dark`, `rose pine`, `kanagawa`,
`everforest`, `github dark`, `github light`, `material dark`, `palenight`,
`zenburn`.

Override any color in config (`background`, `foreground`,
`palette = N=#rrggbb`, …).

## Documentation

- [`docs/roadmap.md`](docs/roadmap.md) — what's built and what's planned.
- [`docs/parity.md`](docs/parity.md) — feature coverage and known gaps.
- [`docs/release.md`](docs/release.md) — how releases are built and shipped.

## License

Licensed under the [Apache License, Version 2.0](LICENSE).

# tileport — Detailed Project Plan

> A Rust-based tiling window manager for macOS that makes your Mac work like your brain expects.
>
> GitHub: `aitechnerd/tileport`
> License: MIT

---

## 1. Vision

tileport is a tiling window manager for macOS built in Rust. It has one guiding principle: **eliminate every spatial friction between you and your work.**

macOS fights you when you want real tiling, instant workspace switching, or sensible multi-monitor behavior. Linux WMs like Hyprland and i3 solve these problems beautifully — but Linux on a MacBook means losing battery life, trackpad quality, AirDrop, and app compatibility. tileport brings the best of the Linux tiling experience to macOS without asking you to give up what makes the Mac worth using.

The project's differentiator: **first-class dock/undock monitor profile switching.** No existing macOS WM handles the transition between an external ultrawide and laptop-only mode without manual intervention. tileport treats this as a core feature, not an afterthought.

---

## 2. Problems We Solve

These are real complaints from real users of Aerospace, yabai, and Amethyst, validated through extensive research.

### 2.1 What Every macOS WM Gets Wrong

| Problem | Who has it | How tileport fixes it |
|---------|-----------|----------------------|
| Monitor disconnect destroys window layout | Amethyst, yabai, Aerospace | Monitor profiles with per-workspace layout memory and automatic switching |
| macOS Space animations are slow and uncontrollable | yabai, Amethyst | Virtual workspaces with instant switching (Aerospace approach) |
| Need to disable SIP for basic features | yabai | Zero SIP dependency — public APIs only (one private API for window ID) |
| Need a separate hotkey tool (skhd) | yabai | Built-in keybinding engine with binding modes |
| Window moves are sluggish and unreliable | Amethyst | Instant, reliable window positioning — never dropped, never misplaced |
| No visual feedback for state changes | Amethyst | Focus borders, workspace indicator, mode indicator in menu bar |
| Ricing/aesthetics deliberately ignored | Aerospace | Ship one polished look with subtle animations by default |
| No focus-follows-mouse without SIP | Aerospace, Amethyst | Focus-follows-mouse using public Accessibility API observation |
| No scratchpad/quick-access overlays | All macOS WMs | Special workspaces (toggle-able overlay workspaces) |
| Complex nested tree layouts are hard to build | Aerospace | Pre-split direction command (i3-style) AND after-the-fact adjustment |
| Hidden windows peek from screen edge | Aerospace | Minimize cosmetic leakage with smallest possible offscreen sliver |
| Config requires restart or is a GUI-only | Mixed | TOML config with instant hot-reload on save |
| No "find my lost window" capability | All macOS WMs | Window finder command — fuzzy search all managed windows |
| No tabbed/grouped windows | All macOS WMs | Window groups — multiple windows sharing one tile slot |

### 2.2 What Linux WMs Do That macOS WMs Don't

Features beloved by Hyprland/i3/Sway users that don't exist on macOS:

| Linux Feature | tileport Equivalent |
|--------------|-------------------|
| Smooth window animations (Hyprland) | Optional subtle animations (100-200ms) for window open/close/move |
| Scratchpads / special workspaces (Hyprland) | Toggle-able overlay workspaces for terminal, notes, music |
| Window groups / tabbed containers (Hyprland) | Multiple windows in one tile slot with tab indicators |
| Per-monitor independent workspaces (i3) | Each monitor owns its workspace set |
| Pre-declare split direction before opening window (i3) | `tileport split horizontal` before launching next app |
| Socket-based IPC with event broadcasting (Hyprland) | Unix socket for commands + event stream for external tools |
| Config reload on save (Hyprland) | File watcher triggers instant reload, no restart needed |
| Gradient borders on focused window (Hyprland) | Configurable focus indicator (border color/width) |
| Dynamic workspaces (Hyprland) | Workspaces created on demand, destroyed when empty (optional mode) |
| Auto-tiling based on aspect ratio (Hyprland dwindle) | Zone fill order with center-first priority, tuned for ultrawide |

---

## 3. Design Principles

1. **One keybinding, zero thought.** Every common action should be muscle memory. No multi-step operations for daily tasks.

2. **Instant and reliable.** Windows move in under 16ms. No dropped moves. No windows landing in wrong positions. If Aerospace users describe it as "the move never fails" — tileport must be even more reliable.

3. **Beautiful by default.** Ship one polished look — subtle animations, clean focus borders, tasteful gaps. Not ricing for Reddit; intentional design that makes you happier. Disable animations with one config flag if you want raw speed.

4. **Zero SIP, zero hacks.** Only public macOS APIs plus the single `_AXUIElementGetWindow` private API that every macOS WM uses. No Dock injection, no scripting additions, no code signing headaches.

5. **Monitor transitions are a lifestyle feature.** Plugging in or unplugging a display should feel like your workspace is breathing — expanding or contracting to fit, with zero manual intervention.

6. **Unix philosophy inside.** tileport does tiling, workspaces, and keybindings. Clipboard management, screenshots, app launching — those are other tools. tileport exposes IPC so they can integrate.

7. **Config is code, code is shareable.** One TOML file defines everything. Put it in a dotfiles repo or a nix-darwin flake. New machine, same workflow.

---

## 4. Feature Specification

### 4.1 Layout Engine

tileport uses a **zone-based layout system** rather than a generic BSP tree. Zones are predefined regions of the screen that windows fill in a specific order. This is more opinionated than BSP but produces predictable, beautiful layouts without needing to manually arrange a tree.

Each layout is assigned to a workspace or monitor profile. Different screens get different layouts automatically.

#### 4.1.1 Layout: Ultrawide Three-Column (default for 38" ultrawide)

The primary layout for wide monitors. Three vertical columns with a dominant center.

```toml
[layouts.ultrawide-3col]
columns = [0.30, 0.40, 0.30]  # left 30%, center 40%, right 30%
```

**Fill order** — windows fill zones in a natural priority:

```
1 window:                    2 windows:
┌────────────────────────┐   ┌───────┬────────────────┐
│                        │   │       │                │
│       A (center)       │   │   B   │    A (center)  │
│       100% width       │   │  left │                │
│                        │   │       │                │
└────────────────────────┘   └───────┴────────────────┘

3 windows:                   4 windows:
┌───────┬──────────┬───────┐ ┌───────┬──────────┬───────┐
│       │          │       │ │   B   │          │       │
│   B   │    A     │   C   │ ├───────┤    A     │   C   │
│  left │  center  │ right │ │   D   │  center  │ right │
│       │          │       │ │  b-l  │          │       │
└───────┴──────────┴───────┘ └───────┴──────────┴───────┘

5 windows:                   6 windows:
┌───────┬──────────┬───────┐ ┌───────┬──────────┬───────┐
│   B   │          │   C   │ │   B   │    A     │   C   │
├───────┤    A     ├───────┤ ├───────┤  center  ├───────┤
│   D   │  center  │   E   │ │   D   │          │   E   │
│  b-l  │          │  b-r  │ ├───────┤          ├───────┤
└───────┴──────────┴───────┘ │   F   │          │       │
                              └───────┴──────────┴───────┘
```

**Fill order logic:**
1. First window → center column (full height)
2. Second window → left column (full height)
3. Third window → right column (full height)
4. Fourth window → left column splits horizontally: top-left and bottom-left
5. Fifth window → right column splits horizontally: top-right and bottom-right
6. Sixth+ → continue subdividing side columns

The center column NEVER subdivides — it's always your primary focus window (editor, browser). Side columns stack vertically as windows are added.

#### 4.1.2 Layout: Laptop Two-Column (default for 15" MacBook)

```toml
[layouts.laptop-2col]
columns = [0.40, 0.60]  # left 40%, right 60%
```

```
1 window:                    2 windows:
┌────────────────────────┐   ┌──────────┬─────────────┐
│                        │   │          │             │
│     A (full screen)    │   │  B (40%) │  A (60%)    │
│                        │   │          │             │
│                        │   │          │             │
└────────────────────────┘   └──────────┴─────────────┘

3 windows:                   4 windows:
┌──────────┬─────────────┐   ┌──────────┬─────────────┐
│    B     │             │   │    B     │      A      │
├──────────┤      A      │   ├──────────┤    (60%)    │
│    C     │    (60%)    │   │    C     ├─────────────┤
│          │             │   │          │      D      │
└──────────┴─────────────┘   └──────────┴─────────────┘
```

Fill order: first window full screen, second goes right (60%), third stacks on left, etc.

#### 4.1.3 Layout: Monocle (default for 13" or small screens)

One window at a time, full screen. Navigation moves between windows like a carousel.

```toml
[layouts.monocle]
type = "monocle"
```

```
┌────────────────────────┐
│                        │
│     A (focused)        │    B, C, D exist but are
│     full screen        │    hidden offscreen
│                        │
└────────────────────────┘

Focus right → A slides out, B slides in (with animation)
```

#### 4.1.4 Layout: Zen Focus (the navigation model)

This isn't a separate layout — it's a **navigation behavior** that applies to any zone layout. The idea: the focused zone expands, non-focused zones compress.

On ultrawide with Zen enabled:

```
Normal state (A focused in center):
┌───────┬──────────┬───────┐
│   B   │    A     │   C   │
│  30%  │   40%    │  30%  │
└───────┴──────────┴───────┘

Focus moves right (C becomes focused):
┌──┬──────────┬────────────┐
│B │    A     │     C      │
│  │   (dim)  │  (focused) │
│  │          │   expanded │
└──┴──────────┴────────────┘

Focus right again → C moves to center, next app fills right
```

On monocle/laptop with Zen enabled, it becomes the exact behavior you described: focused app is full screen (or near-full), navigating left/right brings that side app into the focus position. It's like a carousel where the center is always the main stage.

```toml
[layouts.ultrawide-3col]
columns = [0.30, 0.40, 0.30]
zen-focus = true         # focused zone expands
zen-focused-ratio = 0.50 # focused column grows to 50%
zen-dim-inactive = true  # slightly dim non-focused windows (opacity)
```

#### 4.1.5 Custom Layouts

Users can define any zone layout:

```toml
# Equal four-quadrant layout
[layouts.quadrant]
columns = [0.50, 0.50]
max-per-column = 2

# 70/30 main-side for coding
[layouts.code-focus]
columns = [0.70, 0.30]

# Equal three columns (ultrawide)
[layouts.equal-3]
columns = [0.33, 0.34, 0.33]
```

#### 4.1.6 Layout Assignment

Layouts are assigned to workspaces and/or monitor profiles:

```toml
[workspaces]
1 = { layout = "ultrawide-3col" }  # coding workspace
2 = { layout = "monocle" }         # focused reading
3 = { layout = "laptop-2col" }     # browser + notes
4 = { layout = "monocle" }         # comms (Slack fullscreen)

# Override per profile
[profiles.docked.workspaces]
1 = { monitor = "main", layout = "ultrawide-3col" }
3 = { monitor = "main", layout = "ultrawide-3col" }

[profiles.undocked.workspaces]
1 = { monitor = "only", layout = "laptop-2col" }
3 = { monitor = "only", layout = "monocle" }
```

#### 4.1.7 Window Operations Within Layouts

| Operation | Description | Default keybinding |
|-----------|-------------|-------------------|
| Focus directional | Move focus to adjacent zone | `alt+h/j/k/l` |
| Swap | Swap focused window with window in direction | `alt+shift+h/j/k/l` |
| Promote to center | Move focused window to center/primary zone | `alt+enter` |
| Cycle layout | Switch workspace to next configured layout | `alt+slash` |
| Resize columns | Adjust column widths | `alt+[` / `alt+]` |
| Balance | Reset column widths to configured defaults | `alt+shift+b` |
| Float toggle | Remove from layout, float freely | `alt+shift+f` |
| Fullscreen toggle | Window fills entire workspace | `alt+f` |

**Floating** windows remember their zone position — toggling float off returns them to where they were.

### 4.2 Virtual Workspace System

tileport completely bypasses macOS Spaces (they're slow, animated, limited to 16, no public API). Instead, it implements its own workspace system by moving windows offscreen.

#### 4.2.1 How It Works

- Each workspace has an independent layout tree
- Inactive workspace windows are moved to the bottom-right corner of the screen (offscreen, with a few pixels visible — macOS limitation)
- Switching workspaces: hide current windows offscreen, restore target workspace windows to their calculated positions
- On crash or quit: all windows restored to visible positions

#### 4.2.2 Workspace Configuration

```toml
[workspaces]
# Workspace display names — shown in indicator, switch overlay, IPC events
# Accessed via alt+<key> by default
names = ["code", "browser", "claude-1", "claude-2", "claude-3", "6", "7", "8", "9"]

# Dynamic workspaces: create on demand, destroy when empty
dynamic = false

# Attention system (§4.11) — signals which workspace needs you
[attention]
auto-clear-on-focus = true       # switching to a workspace clears its attention flag
title-triggers = [               # regex patterns matched against window titles
    "waiting for input",         # Claude Code waiting state
    "\\[done\\]",               # agent completed
    "\\[error\\]",              # agent hit an error
]
debounce-ms = 2000               # ignore rapid title changes during agent streaming
show-peek = true                 # flash overlay when attention fires on another workspace
peek-duration-ms = 1500
peek-position = "top-right"      # top-right, top-center, bottom-right

# Workspace-to-monitor default assignments
[workspaces.monitor-assignment]
# When docked, these workspaces go to the ultrawide
1 = "main"
2 = "main"
3 = "main"
# These go to the laptop screen
4 = "secondary"
5 = "secondary"
```

#### 4.2.3 Special Workspaces (Scratchpads)

Toggle-able overlay workspaces that slide in/out. Perfect for a quick terminal, notes app, or music player.

```toml
[[special-workspaces]]
name = "terminal"
key = "grave"  # alt+` to toggle
width = 0.8    # 80% of screen width
height = 0.6   # 60% of screen height
position = "center"  # center, top, bottom
animation = "slide-down"  # slide-down, slide-up, fade, none

[[special-workspaces]]
name = "music"
key = "m"  # alt+m to toggle
width = 0.4
height = 0.5
position = "bottom-right"
```

When toggled, the scratchpad window floats above the tiling layout. Toggle again to hide. The window persists — it's not closed, just hidden.

### 4.3 Monitor Profile System

**This is tileport's defining feature.** No other macOS WM does this.

#### 4.3.1 Profile Detection

tileport listens for `CGDisplayRegisterReconfigurationCallback` events. When monitors change, it:

1. Enumerates connected displays (resolution, position, scale factor)
2. Matches against configured profiles using resolution patterns
3. Saves current layout state (split ratios, window positions, focused window per workspace)
4. Applies the new profile's workspace assignments and layout modes
5. Restores split ratios where applicable
6. Fires `exec-on-profile-change` callback

```toml
[profiles.docked]
# Match when an ultrawide + built-in display are connected
monitors = [
    { pattern = "3840x*", role = "main" },     # ultrawide (any height)
    { pattern = "2880x*", role = "secondary" }, # MacBook Retina
]

[profiles.docked.workspaces]
# Workspace 1: coding — tiles on ultrawide, editor-heavy
1 = { monitor = "main", layout = "h_tiles", split-ratio = 0.55 }
# Workspace 2: terminals — accordion on ultrawide
2 = { monitor = "main", layout = "h_accordion" }
# Workspace 3: browser/research — tiles on ultrawide
3 = { monitor = "main", layout = "h_tiles" }
# Workspace 4: comms — tiles on laptop
4 = { monitor = "secondary", layout = "h_tiles" }
# Workspace 5: music/misc — accordion on laptop
5 = { monitor = "secondary", layout = "h_accordion" }

[profiles.undocked]
# Match when only the built-in display is connected
monitors = [
    { pattern = "2880x*", role = "only" },
]

[profiles.undocked.workspaces]
# Everything goes accordion on laptop — one window at a time
1 = { monitor = "only", layout = "h_accordion" }
2 = { monitor = "only", layout = "h_accordion" }
3 = { monitor = "only", layout = "h_accordion" }
4 = { monitor = "only", layout = "h_tiles" }  # comms stays tiled
5 = { monitor = "only", layout = "h_accordion" }
```

#### 4.3.2 Profile Switch Behavior

```
Display change detected
    │
    ├── Save current state
    │   ├── Per-workspace: split ratios, focused window, window order
    │   ├── Per-window: last known position in tree
    │   └── Write to ~/.local/state/tileport/profiles/<profile>.json
    │
    ├── Match new monitor set to profile
    │   ├── Exact match → apply profile
    │   ├── No match → use "fallback" profile (all accordion on primary)
    │   └── Manual override available via hotkey
    │
    ├── Redistribute workspaces
    │   ├── Move workspaces to new monitor assignments
    │   ├── Switch layout modes per workspace rules
    │   └── Workspaces not in profile stay on primary monitor
    │
    └── Restore layout state
        ├── Tiles→tiles: restore split ratios from saved state
        ├── Tiles→accordion: just track focused window
        ├── Accordion→tiles: restore split ratios if previously saved
        └── Recalculate all window positions
```

#### 4.3.3 Manual Override

For USB-C docks that send flaky hotplug events:

```toml
[keybindings.main]
alt-shift-d = "profile docked"    # manually switch to docked profile
alt-shift-u = "profile undocked"  # manually switch to undocked profile
```

### 4.4 Window Management

#### 4.4.1 Core Window Operations

| Feature | Description | Default keybinding |
|---------|-------------|-------------------|
| Focus directional | Focus window in direction (left/right/up/down) | `alt+h/j/k/l` |
| Move directional | Move focused window in direction within tree | `alt+shift+h/j/k/l` |
| Move to workspace | Send window to another workspace | `alt+shift+1..9` |
| Move with follow | Send window and switch to that workspace | `alt+ctrl+1..9` |
| Float toggle | Toggle floating for focused window | `alt+shift+f` |
| Fullscreen toggle | Window fills workspace, others hidden | `alt+f` |
| Close window | Close focused window (pass-through to app's cmd+w) | Managed by app |
| Focus previous | Focus previously focused window | `alt+tab` |
| Window finder | Fuzzy search all managed windows by title/app | `alt+space` |

#### 4.4.2 Focus-Follows-Mouse

Optional, off by default. When enabled, moving the mouse to a window focuses it without clicking. Uses AXObserver to watch cursor position against known window rects.

```toml
[behavior]
focus-follows-mouse = false
# Delay before focus changes (ms) — prevents accidental focus on mouse pass-through
focus-follows-mouse-delay = 150
```

#### 4.4.3 Window Rules

Per-application rules for automatic behavior on window creation:

```toml
[[window-rules]]
app-id = "com.apple.systempreferences"
command = "float"

[[window-rules]]
app-id = "com.tinyspeck.slackmacgap"
command = "move-to-workspace 4"

[[window-rules]]
app-id = "com.spotify.client"
command = "move-to-workspace 5"

[[window-rules]]
# Float all windows without a fullscreen button (dialog heuristic)
match = "no-fullscreen-button"
command = "float"
exclude-apps = ["com.mitchellh.ghostty", "com.googlecode.iterm2", "net.kovidgoyal.kitty"]
```

#### 4.4.4 Dialog Detection

Following Aerospace's proven heuristic:
- Query Accessibility API: is window a dialog/sheet?
- Windows without a fullscreen button → float by default
- Terminal emulators explicitly excluded from above rule
- User-defined overrides in window-rules

#### 4.4.5 App Persistence (Pre-launch)

Keep frequently used apps running but hidden in their workspace. When switching to that workspace, the app is instantly visible — no launch delay.

```toml
[persistence]
# Apps to launch at tileport start and keep running
launch-on-start = [
    { app = "com.mitchellh.ghostty", workspace = "2" },
    { app = "com.tinyspeck.slackmacgap", workspace = "4" },
]
```

This directly addresses the macOS "app launch speed" complaint. The app isn't launching — it's already there.

### 4.5 Keybinding System

Built-in. No skhd dependency.

#### 4.5.1 Architecture

Uses `CGEventTap` on a dedicated thread with `CFRunLoop`. Events are intercepted at the HID level (before any app sees them), allowing tileport to consume or pass-through each keystroke.

Permissions required: Input Monitoring (System Settings → Privacy & Security → Input Monitoring). tileport uses `CGPreflightListenEventAccess` / `CGRequestListenEventAccess` for clean permission flow on first launch.

#### 4.5.2 Binding Modes

Like i3/Aerospace, keybindings live in named modes. Only one mode is active at a time. The menu bar indicator shows the current mode.

```toml
[keybindings.main]
# Window focus
alt-h = "focus left"
alt-j = "focus down"
alt-k = "focus up"
alt-l = "focus right"

# Window move
alt-shift-h = "move left"
alt-shift-j = "move down"
alt-shift-k = "move up"
alt-shift-l = "move right"

# Workspace switching
alt-1 = "workspace 1"
alt-2 = "workspace 2"
# ... etc

# Layout
alt-slash = "layout tiles horizontal vertical"
alt-comma = "layout accordion horizontal vertical"
alt-v = "split vertical"      # i3-style pre-split
alt-b = "split horizontal"    # i3-style pre-split

# Resize mode
alt-shift-r = "mode resize"

# Service mode (for tree manipulation)
alt-shift-semicolon = "mode service"

# Attention quick-cycle (jump between workspaces that need you)
alt-tab = "workspace-attention next"
alt-shift-tab = "workspace-attention prev"

# Scratchpads
alt-grave = "special-workspace terminal"

[keybindings.resize]
h = "resize width -50"
l = "resize width +50"
j = "resize height +50"
k = "resize height -50"
equal = "balance"
escape = "mode main"
enter = "mode main"

[keybindings.service]
r = ["flatten-workspace-tree", "mode main"]
f = ["layout floating tiling", "mode main"]
backspace = ["close-all-windows-but-current", "mode main"]
alt-shift-h = ["join-with left", "mode main"]
alt-shift-j = ["join-with down", "mode main"]
alt-shift-k = ["join-with up", "mode main"]
alt-shift-l = ["join-with right", "mode main"]
escape = "mode main"
```

#### 4.5.3 Key Notation

Support for multiple keyboard layouts:

```toml
[keybindings]
preset = "qwerty"  # qwerty | dvorak | colemak
```

Modifiers: `alt`, `cmd`, `ctrl`, `shift` and combinations (`alt-shift-h`).

### 4.6 Visual Presentation

#### 4.6.1 Gaps

```toml
[gaps]
inner = 8    # pixels between windows
outer = 8    # pixels between windows and screen edge
```

#### 4.6.2 Focus Borders

```toml
[borders]
enabled = true
width = 2
active-color = "#89b4fa"    # Catppuccin blue
inactive-color = "#45475a"  # Catppuccin surface1
```

Implementation: Uses Core Graphics to draw borders around the focused window. Not window decorations (macOS doesn't allow that) — overlay drawing.

#### 4.6.3 Animations

Optional, enabled by default. Subtle and fast.

```toml
[animations]
enabled = true
duration-ms = 150
curve = "ease-out"   # ease-out | ease-in-out | linear | none

# Per-operation overrides
[animations.window-open]
enabled = true
type = "fade"        # fade | scale | slide | none
duration-ms = 100

[animations.workspace-switch]
enabled = true
type = "fade"
duration-ms = 80

[animations.scratchpad]
enabled = true
type = "slide-down"
duration-ms = 150
```

### 4.7 Menu Bar

Minimal menu bar icon via `objc2-app-kit` NSStatusBar.

Displays:
- Current workspace number/name
- Current binding mode (when not "main")
- Current profile name (docked/undocked)
- Small dot indicator when a scratchpad is active

Click to show:
- List of workspaces with window counts
- Profile switcher
- Reload config
- Enable/disable tileport
- Quit

### 4.8 IPC System

Unix socket at `/tmp/tileport-<uid>.sock`. The socket server runs on a dedicated tokio thread inside the tileport-wm process. Commands come in, results go out. Event subscribers connect and receive a JSON stream.

This is NOT a client-server architecture. The WM is one process. The socket exists only for external tool integration.

#### 4.8.1 Command Socket

```bash
# CLI usage
tileport focus left
tileport workspace 3
tileport layout accordion
tileport profile docked
tileport list-windows --json
tileport list-workspaces --json
tileport query focused-window --json
tileport subscribe workspace-change focus-change profile-change

# Workspace attention & naming (see §4.11)
tileport workspace-attention --set 3
tileport workspace-attention --clear 3
tileport workspace-name --set 3 "t3-code"
tileport workspace-attention --next     # cycle to next workspace with attention
```

#### 4.8.2 Event Stream

External tools (Sketchybar, custom scripts) subscribe to events:

| Event | Payload |
|-------|---------|
| `workspace-change` | `{ focused: "3", previous: "1", monitor: "main" }` |
| `focus-change` | `{ window_id: 123, app: "Ghostty", title: "~" }` |
| `profile-change` | `{ profile: "docked", monitors: ["ultrawide-38", "macbook-15"] }` |
| `window-created` | `{ window_id: 456, app: "Firefox", workspace: "3" }` |
| `window-destroyed` | `{ window_id: 456 }` |
| `mode-change` | `{ mode: "resize" }` |
| `layout-change` | `{ workspace: "1", layout: "h_accordion" }` |
| `workspace-attention` | `{ workspace: "3", attention: true }` |
| `workspace-name-change` | `{ workspace: "3", name: "t3-code" }` |

#### 4.8.3 Event Callbacks

Shell commands triggered by events:

```toml
[callbacks]
exec-on-workspace-change = [
    "/bin/bash", "-c",
    "sketchybar --trigger tileport_workspace FOCUSED=$TILEPORT_FOCUSED_WORKSPACE"
]
exec-on-profile-change = [
    "/bin/bash", "-c",
    "notify-send 'Monitor profile: $TILEPORT_PROFILE'"
]
exec-on-workspace-attention = [
    "/bin/bash", "-c",
    "sketchybar --trigger tileport_attention WORKSPACE=$TILEPORT_WORKSPACE ATTENTION=$TILEPORT_ATTENTION"
]
```

### 4.9 Configuration

#### 4.9.1 File Location

`~/.config/tileport/tileport.toml`

Falls back to `$XDG_CONFIG_HOME/tileport/tileport.toml`.

#### 4.9.2 Hot Reload

Uses the `notify` crate to watch for file changes. On save:
1. Parse new config
2. Validate (report errors via menu bar notification + stdout)
3. Apply changes that can be applied live (keybindings, gaps, rules, callbacks)
4. Some changes require re-layout (gap changes, normalization changes)
5. Never requires restart

#### 4.9.3 Default Config

On first launch, if no config exists, tileport generates a sensible default config optimized for a MacBook + external monitor setup. This config is the "omakase" — good enough to be productive immediately.

#### 4.9.4 Config Validation

```bash
tileport check-config
# Success: Config is valid.
# Failure: Error at line 42: unknown key 'laoyut' in [workspaces] — did you mean 'layout'?
```

### 4.10 Crash Recovery

#### 4.10.1 Window Handle Persistence

tileport continuously writes managed window state to `~/.local/state/tileport/windows.json`:
- Window IDs and their positions
- Workspace assignments
- Layout tree structure

#### 4.10.2 Recovery Behavior

- **On abnormal exit (crash)**: All windows are NOT restored (they're still offscreen). On next launch, tileport reads `windows.json` and restores all windows to visible positions before applying layout.
- **On graceful exit**: All windows are moved to visible positions on the primary monitor before shutdown.
- **On `tileport enable off`**: Same as graceful exit — all hidden windows restored.

### 4.11 Workspace Attention & Agent Awareness

When running 3+ AI coding agents (Claude Code, Codex, etc.) across different workspaces simultaneously, the core problem is: **which workspace needs me right now?** Without attention signals, you're manually polling Option+1 through Option+9 to check each agent. This feature turns tileport into a command center for parallel agent work.

#### 4.11.1 Attention Flags

Each workspace has a boolean `attention` flag. When set, the workspace indicator shows a visual signal (dot, color pulse, or ring) so you know to switch there.

**How attention gets set:**

1. **IPC command** (primary, most reliable):
   ```bash
   # Set from a Claude Code hook, shell script, or agent wrapper
   tileport workspace-attention --set 3
   tileport workspace-attention --set current   # mark focused workspace
   ```

2. **AX title-change heuristic** (automatic, zero-config for common cases):
   tileport already watches `AXTitleChanged` notifications. When a window title matches a configurable pattern, attention is set on that window's workspace.
   ```toml
   [attention]
   # Patterns matched against window titles (regex)
   title-triggers = [
       "waiting for input",    # Claude Code waiting state
       "\\[done\\]",           # Agent completed
       "\\[error\\]",          # Agent hit an error
   ]
   debounce-ms = 2000          # Ignore rapid title changes (agent streaming output)
   ```

3. **macOS app activation request** (`NSApplicationDidBecomeActive` / dock bounce):
   If an app requests user attention via the standard macOS mechanism, set attention on its workspace.

**How attention gets cleared:**

- **Auto-clear on focus** (default): Switching to a workspace clears its attention flag. Configurable:
  ```toml
  [attention]
  auto-clear-on-focus = true   # default
  ```
- **Manual clear**: `tileport workspace-attention --clear 3`

#### 4.11.2 Attention Quick-Cycle

A keybinding that jumps only between workspaces with attention flags set — the "inbox" for your agents:

```toml
[keybindings.main]
alt-tab = "workspace-attention next"      # cycle to next workspace needing attention
alt-shift-tab = "workspace-attention prev"  # cycle backwards
```

With 3 Claude Code instances on workspaces 2, 5, and 7 — if ws2 and ws7 have attention, `alt-tab` cycles between just those two. No wasted keystrokes visiting idle workspaces.

#### 4.11.3 Workspace Naming

Workspaces can have display names set in config or at runtime. Names appear in the menu bar indicator, workspace-switch overlay, and IPC events.

```toml
[workspaces]
# Static names (set in config)
names = ["code", "browser", "claude-1", "claude-2", "claude-3", "6", "7", "8", "9"]
```

```bash
# Dynamic naming via IPC (for scripts that set up project contexts)
tileport workspace-name --set 3 "tileport"
tileport workspace-name --set 5 "t3-code"
```

#### 4.11.4 Visual Indicators

The menu bar tray and workspace-switch overlay show attention state:

```
Menu bar (compact):  [●2] [3] [4] [●7]    ← dots on ws2 and ws7
                      ↑ attention

Workspace switch overlay (on Option+N):
  ┌─────────────────────────────┐
  │  1:code  ●2:claude-1  3:browser  4  ●5:t3-code  │
  └─────────────────────────────┘
  ● = attention dot (colored, e.g., amber for waiting, red for error)
```

Attention state is also broadcast as an IPC event and via callbacks (see §4.8.2, §4.8.3), so Sketchybar or custom bars can render it however they want.

#### 4.11.5 Workspace Peek on Attention

When attention fires on another workspace, tileport briefly flashes a small, transient overlay on the current screen (similar to macOS volume indicator) showing which workspace wants you:

```
  ┌──────────────┐
  │  ● claude-1  │    ← appears for 1.5s, then fades
  │  ws 3        │
  └──────────────┘
```

This gives awareness without breaking flow. Configurable:

```toml
[attention]
show-peek = true
peek-duration-ms = 1500
peek-position = "top-right"   # top-right, top-center, bottom-right
```

#### 4.11.6 Integration with Claude Code

Claude Code supports hooks that run shell commands on specific events. A typical setup:

```jsonc
// ~/.claude/settings.json — Claude Code hooks
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "AskUserQuestion",
        "command": "tileport workspace-attention --set current"
      }
    ],
    "Stop": [
      {
        "command": "tileport workspace-attention --set current"
      }
    ]
  }
}
```

This means: whenever Claude Code asks you a question or finishes a task, the workspace it's running in lights up. You never miss an agent that needs you.

### 4.12 Paper Layout (Scrollable Window Mode)

An alternative layout mode for small screens (laptops) where windows maintain their full size and you navigate by scrolling a viewport across them. Inspired by the Niri scrollable window manager for Linux.

#### 4.12.1 Concept

Instead of tiling (which shrinks windows to fit), the paper layout places windows side-by-side at their preferred size. The screen is a viewport — you see one or two windows at a time, and navigate left/right/up/down to shift the viewport.

```
               viewport (screen)
                ┌──────────┐
  ┌──────┐ ┌───┤──────┐   │ ┌──────┐
  │      │ │   │      │   │ │      │
  │  A   │ │   │  B   │   │ │  C   │
  │      │ │   │(focus)│   │ │      │
  └──────┘ └───┤──────┘   │ └──────┘
                └──────────┘
  ← navigate left    navigate right →
```

When you focus a window, the viewport slides to center it. Adjacent windows peek from the edges so you know what's nearby.

#### 4.12.2 Why This Matters for Small Screens

On a 13-15" laptop, fitting 5 windows requires either:
- **Tiling**: Each window gets ~20% of screen — too small for anything useful
- **Monocle**: One window at a time — no spatial context, everything is "somewhere"
- **Paper**: One window is ~full screen, with 4 others peeking from edges. Navigate to promote any to center

The paper layout gives you **spatial awareness** (the browser is always "to the right") with **full-size windows** (the focused one gets nearly the full screen).

#### 4.12.3 Configuration

```toml
[layouts.paper]
type = "paper"
direction = "horizontal"          # horizontal (left/right) or vertical (up/down) or both
focused-width = 0.85              # focused window gets 85% of screen width
peek-width = 40                   # adjacent windows peek by 40px from edge
center-focused = true             # auto-center focused window in viewport
animation = true                  # smooth slide transition (100-150ms)
```

#### 4.12.4 Navigation

| Operation | Description | Default keybinding |
|-----------|-------------|-------------------|
| Scroll left | Shift viewport left (focus previous window) | `alt+h` |
| Scroll right | Shift viewport right (focus next window) | `alt+l` |
| Scroll up | Shift viewport up (if 2D mode) | `alt+k` |
| Scroll down | Shift viewport down (if 2D mode) | `alt+j` |
| New window right | Open new window to the right of focused | automatic (new windows append) |
| Close + slide | Closing a window slides remaining windows to fill gap | automatic |

The same `alt+h/j/k/l` keybindings work — they just scroll the viewport instead of moving focus between zones. This keeps muscle memory consistent across layout modes.

#### 4.12.5 Interaction with Monitor Profiles

Paper layout is ideal for the `undocked` profile:

```toml
[profiles.docked.workspaces]
1 = { monitor = "main", layout = "ultrawide-3col" }

[profiles.undocked.workspaces]
1 = { monitor = "only", layout = "paper" }     # ← auto-switch to paper on undock
```

When you undock from the ultrawide, workspaces automatically switch from zone-based tiling to paper layout — windows keep their content, just the navigation model changes.

---

## 5. Architecture

### 5.1 Crate Structure

```
tileport/
├── Cargo.toml                      # Workspace root
├── LICENSE                          # MIT
├── README.md
├── config/
│   └── default-config.toml          # Ships with binary
│
├── crates/
│   ├── tileport-core/               # Pure Rust — no platform dependencies
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── zone.rs              # Zone layout: define, calculate rects, fill order
│   │       ├── monocle.rs           # Monocle layout: single window carousel
│   │       ├── paper.rs             # Paper layout: scrollable viewport, windows keep full size
│   │       ├── workspace.rs         # Workspace manager: create, switch, assign, attention
│   │       ├── profile.rs           # Monitor profile detection & switching logic
│   │       ├── config.rs            # TOML parsing, validation, defaults
│   │       ├── command.rs           # Command enum, parsing, dispatch table
│   │       ├── event.rs             # Event types for IPC broadcasting
│   │       └── state.rs             # Serializable state for crash recovery
│   │
│   ├── tileport-macos/              # macOS platform bindings
│   │   ├── Cargo.toml               # objc2, core-graphics, accessibility-sys
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── accessibility.rs     # Safe wrappers over AXUIElement FFI
│   │       ├── window.rs            # Enumerate, move, resize, observe windows
│   │       ├── display.rs           # Monitor detection, hotplug callback
│   │       ├── hotkey.rs            # CGEventTap global key interception
│   │       ├── event_loop.rs        # CFRunLoop + AXObserver integration
│   │       ├── tray.rs              # NSStatusBar menu bar icon
│   │       ├── border.rs            # Window border overlay (Core Graphics / NSWindow)
│   │       ├── animation.rs         # Window position interpolation
│   │       └── permission.rs        # Accessibility & Input Monitoring permission flow
│   │
│   └── tileport-cli/                # CLI client binary (thin socket client)
│       ├── Cargo.toml               # clap, serde_json
│       └── src/
│           └── main.rs              # Connects to Unix socket, sends commands, prints results
│
├── tileport-wm/                     # Main daemon binary (single process, everything runs here)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs                  # Entry point: permission check, config load, start threads
│       ├── manager.rs               # Central coordinator: events → state → layout → apply
│       ├── ipc.rs                   # Unix socket server (commands in, events out)
│       └── recovery.rs              # Crash recovery: persist/restore window state
│
└── tests/
    ├── zone_tests.rs                # Unit tests for zone layout calculations
    ├── monocle_tests.rs             # Unit tests for monocle navigation
    ├── paper_tests.rs               # Unit tests for paper layout viewport calculations
    ├── fill_order_tests.rs          # Property tests: N windows fill correctly, no overlaps
    ├── attention_tests.rs           # Unit tests for workspace attention state transitions
    └── profile_tests.rs             # Unit tests for monitor profile matching
```

Note: The IPC server (`ipc.rs`) lives directly in `tileport-wm`, not as a separate crate. It's a thin tokio-based Unix socket handler — no need for a separate crate for ~200 lines of code. The IPC is for external tools only; all internal communication uses `crossbeam-channel`.

### 5.2 Dependency Map

```toml
# tileport-core — ZERO platform dependencies (fully testable on any OS)
[dependencies]
serde = { version = "1", features = ["derive"] }
toml = "0.8"
serde_json = "1"
thiserror = "2"
tracing = "0.1"

# tileport-macos — all macOS-specific code isolated here
[dependencies]
objc2 = "0.6"
objc2-foundation = { version = "0.3", features = ["NSString", "NSArray", "NSDictionary", "NSNotification"] }
objc2-app-kit = { version = "0.3", features = ["NSApplication", "NSStatusBar", "NSStatusItem", "NSMenu", "NSMenuItem", "NSWorkspace", "NSRunningApplication", "NSImage", "NSWindow", "NSScreen"] }
block2 = "0.6"
dispatch2 = "0.3"
accessibility-sys = "0.1"
core-graphics = "0.25"
core-foundation = "0.10"
macos-accessibility-client = "0.0.1"
tileport-core = { path = "../tileport-core" }
crossbeam-channel = "0.5"   # lock-free channels for hot-path event passing
tracing = "0.1"
anyhow = "1"

# tileport-cli — thin client, connects to Unix socket
[dependencies]
clap = { version = "4", features = ["derive"] }
serde_json = "1"
tileport-core = { path = "../tileport-core" }
# Note: uses std::os::unix::net — no tokio needed for the CLI

# tileport-wm — single-process daemon (everything runs here)
[dependencies]
tileport-core = { path = "../crates/tileport-core" }
tileport-macos = { path = "../crates/tileport-macos" }
crossbeam-channel = "0.5"        # internal thread communication (hotkey → manager, AX → manager)
tokio = { version = "1", features = ["rt", "net", "io-util", "sync"] }  # only for IPC socket server
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
tracing-appender = "0.2"
notify = "7"                     # file watcher for config hot-reload
directories = "5"                # XDG config path resolution
serde_json = "1"
anyhow = "1"
```

**Why `crossbeam-channel` for internal communication:**

The hot path is: CGEventTap fires on hotkey thread → Manager processes on manager thread → AXUIElement calls to move windows. This must be sub-millisecond. `crossbeam-channel` is lock-free and allocation-free for bounded channels, making it significantly faster than tokio channels for this use case. tokio is only used for the Unix socket server (the cold path — external CLI commands and Sketchybar subscriptions).

### 5.3 Event Loop Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                      Main Thread                              │
│                                                               │
│  NSApplication.run()                                          │
│  ├── Menu bar tray icon (NSStatusBar)                        │
│  └── Dispatches UI updates via dispatch2                     │
│                                                               │
├──────────────────────────────────────────────────────────────┤
│                    Hotkey Thread                               │
│                                                               │
│  CFRunLoop + CGEventTap                                       │
│  ├── Intercepts key events at HID level                      │
│  ├── Matches against current binding mode                     │
│  ├── Consumes matched events (returns NULL to suppress)      │
│  ├── Passes unmatched events through                         │
│  └── Sends Command via crossbeam-channel (bounded, lock-free)│
│                                                               │
├──────────────────────────────────────────────────────────────┤
│                   Manager Thread                              │
│                                                               │
│  crossbeam::select! on multiple channels:                    │
│  ├── hotkey_rx    → Keybinding commands                      │
│  ├── ax_rx        → Window create/close/move/resize events   │
│  ├── display_rx   → Monitor connect/disconnect events        │
│  ├── ipc_rx       → Commands from CLI / external tools       │
│  ├── config_rx    → Config reload events from file watcher   │
│  └── timer_rx     → Polling fallback tick (250ms)            │
│                                                               │
│  For each event:                                              │
│  1. Update state (zones, workspace, profile)                 │
│  2. Calculate new window positions from zone layout          │
│  3. Apply via AXUIElement (move/resize windows)              │
│  4. Broadcast event to IPC subscribers                       │
│  5. Update menu bar indicator (via dispatch to main thread)  │
│  6. Persist state for crash recovery (debounced, every 1s)   │
│                                                               │
├──────────────────────────────────────────────────────────────┤
│                    IPC Thread (tokio)                          │
│                                                               │
│  tokio::runtime (single-threaded, only for socket I/O)       │
│  ├── Unix socket server at /tmp/tileport-<uid>.sock          │
│  ├── Command handling → sends to ipc_tx crossbeam channel    │
│  ├── Event broadcasting → receives from manager broadcast    │
│  └── Subscriber management (Sketchybar, custom scripts)      │
│                                                               │
├──────────────────────────────────────────────────────────────┤
│                  Config Watcher Thread                         │
│                                                               │
│  notify::Watcher on tileport.toml                            │
│  └── On change: parse → validate → send to config_tx channel │
└──────────────────────────────────────────────────────────────┘
```

**Thread count**: Exactly 5 threads total. No thread pools, no dynamic spawning. Deterministic and debuggable.

### 5.4 Key Data Structures

```rust
/// A zone layout defines how a screen is divided into regions
struct ZoneLayout {
    name: String,
    columns: Vec<f64>,          // column width ratios [0.30, 0.40, 0.30]
    max_per_column: Option<u32>, // max windows per column before overflow
    zen_focus: bool,             // enable focus-zoom behavior
    zen_focused_ratio: f64,      // focused column expands to this ratio
    zen_dim_inactive: bool,      // dim non-focused windows
}

/// A zone is a rectangular region on screen that holds windows
struct Zone {
    column: usize,               // which column (0-indexed)
    row: usize,                  // which row within column (0-indexed)
    rect: Rect,                  // calculated screen coordinates
    windows: Vec<WindowId>,      // windows in this zone (usually 1)
    focused: bool,
}

/// Monocle layout — one window at a time
struct MonocleLayout {
    windows: Vec<WindowId>,      // all windows in order
    focused_index: usize,        // which one is visible
    animation: AnimationType,    // slide, fade, none
}

/// Paper layout — scrollable viewport, windows maintain full size
struct PaperLayout {
    windows: Vec<WindowId>,      // windows in spatial order (left to right)
    focused_index: usize,        // which one is centered in viewport
    direction: PaperDirection,   // Horizontal, Vertical, or Both
    focused_width_ratio: f64,    // focused window gets this ratio of screen (e.g., 0.85)
    peek_px: u32,                // adjacent windows peek by this many pixels
}

enum PaperDirection {
    Horizontal,
    Vertical,
    Both,
}

/// A workspace holds a layout and its assigned windows
struct Workspace {
    name: String,
    display_name: Option<String>,  // user-facing name (e.g., "t3-code", "claude-1")
    layout: WorkspaceLayout,       // Zone, Monocle, Paper, or Float
    monitor: MonitorId,
    focused_window: Option<WindowId>,
    attention: bool,                // true when workspace needs user attention (§4.11)
    /// Saved state for profile switching
    saved_states: HashMap<ProfileName, SavedLayoutState>,
}

enum WorkspaceLayout {
    Zone {
        layout: ZoneLayout,
        zones: Vec<Zone>,        // calculated zones with window assignments
        fill_order: Vec<WindowId>, // order windows were added
    },
    Monocle(MonocleLayout),
    Paper(PaperLayout),          // scrollable viewport layout (§4.12)
    Float,                       // all windows floating
}

/// Monitor profile
struct Profile {
    name: String,
    monitor_patterns: Vec<MonitorPattern>,
    workspace_rules: HashMap<String, WorkspaceRule>,
}

struct WorkspaceRule {
    monitor: MonitorRole,        // "main", "secondary", "only"
    layout: String,              // layout name from [layouts.*]
    zen_focus: Option<bool>,     // override layout's zen setting
}

/// What gets persisted per workspace when switching profiles
struct SavedLayoutState {
    layout_name: String,
    column_ratios: Vec<f64>,     // possibly user-adjusted ratios
    window_order: Vec<WindowId>, // fill order
    focused_window: Option<WindowId>,
}

/// Represents a physical display
struct Monitor {
    id: MonitorId,
    resolution: (u32, u32),
    scale_factor: f64,
    position: (i32, i32),
    is_builtin: bool,            // true for MacBook's own display
}

/// A managed window
struct ManagedWindow {
    id: WindowId,
    ax_element: AXUIElementRef,  // accessibility reference
    app_id: String,              // bundle identifier
    app_name: String,
    title: String,
    workspace: String,
    is_floating: bool,
    floating_rect: Option<Rect>, // remembered position when floating
    is_dialog: bool,
}
```

### 5.5 Process Architecture

tileport is a **single-process daemon** — not a client-server system. One process manages everything: window observation, layout calculation, keybinding interception, and menu bar UI. The CLI client (`tileport-cli`) is a thin tool that connects to a Unix socket to send commands, but the WM itself is one cohesive process.

Why NOT client-server:
- A window manager needs sub-millisecond response to key events. Cross-process IPC adds latency.
- State synchronization between processes is unnecessary complexity.
- A single process can crash-recover more reliably (one state file, one recovery path).
- macOS Accessibility API calls must happen from the process that owns the AX connection.

The Unix socket exists solely for:
- The `tileport-cli` binary (scripting, shell commands)
- External tool integration (Sketchybar subscribing to events)
- NOT for internal communication between tileport components

```
┌─────────────────────────────────────────────────────┐
│              tileport-wm (single process)            │
│                                                      │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────┐ │
│  │ Main Thread  │  │ Hotkey Thread│  │ IPC Thread │ │
│  │ NSApp.run()  │  │ CGEventTap   │  │ tokio unix │ │
│  │ + menu bar   │  │ + CFRunLoop  │  │ socket     │ │
│  └──────┬───────┘  └──────┬───────┘  └─────┬──────┘ │
│         │                  │                 │        │
│         └──────────┬───────┘                 │        │
│                    ▼                         │        │
│         ┌──────────────────┐                 │        │
│         │  Event Channel   │ ◄───────────────┘        │
│         │  (crossbeam)     │                          │
│         └────────┬─────────┘                          │
│                  ▼                                    │
│         ┌──────────────────┐                          │
│         │     Manager      │                          │
│         │ • Process event  │                          │
│         │ • Update state   │                          │
│         │ • Calculate zones│                          │
│         │ • Apply via AX   │                          │
│         │ • Broadcast event│                          │
│         │ • Persist state  │                          │
│         └──────────────────┘                          │
└─────────────────────────────────────────────────────┘

External:
┌─────────────┐      ┌─────────────┐
│ tileport-cli│─────▶│ Unix socket │
│ (commands)  │      │ /tmp/tile.. │
└─────────────┘      └─────────────┘
┌─────────────┐      ┌─────────────┐
│ Sketchybar  │◀─────│ Event stream│
│ (subscribe) │      │ (subscribe) │
└─────────────┘      └─────────────┘
```

Internal communication uses `crossbeam-channel` (not tokio channels) for the hot path between threads. The IPC thread is the only part that uses tokio, and only for the Unix socket server.

---

## 6. Implementation Milestones

### Phase 1: Foundation (Weeks 1-3)

**Goal**: Talk to macOS. Enumerate windows, move them, detect monitors.

- [ ] Cargo workspace skeleton with all crates
- [ ] `tileport-macos/permission.rs`: Accessibility permission prompt on first launch
- [ ] `tileport-macos/accessibility.rs`: Safe wrappers over `accessibility-sys`
  - `AXUIElementCreateApplication`
  - `AXUIElementCopyAttributeValue` (position, size, title, role, subrole)
  - `AXUIElementSetAttributeValue` (position, size)
  - `AXUIElementCopyElementAtPosition`
- [ ] `tileport-macos/window.rs`: Enumerate all windows, move, resize
  - Use `CGWindowListCopyWindowInfo` for initial enumeration
  - Use AX API for move/resize operations
  - Filter out menubar, Dock, desktop, notification windows
- [ ] `tileport-macos/display.rs`: Monitor detection and hotplug
  - `CGGetActiveDisplayList` for current monitors
  - `CGDisplayBounds`, `CGDisplayScreenSize` for resolution/position
  - `CGDisplayRegisterReconfigurationCallback` for connect/disconnect
- [ ] Basic `tracing` + `tracing-subscriber` logging
- [ ] Study Paneru source code for macOS API patterns
- [ ] **Integration test**: List all windows, move one to (100, 100), verify position

### Phase 2: Layout Engine (Weeks 4-6)

**Goal**: Zone-based layouts that calculate correct window positions.

- [ ] `tileport-core/zone.rs`: Zone layout engine
  - Define zone layout from column ratios
  - Calculate zone rects from monitor dimensions + gaps
  - Fill order algorithm: center first, then sides, then subdivide sides
  - Handle dynamic zone creation as windows are added/removed
  - Resize columns (user adjusts column widths)
  - Balance (reset to configured ratios)
  - Swap windows between zones
  - Promote window to center zone
  - Zen focus mode: recalculate ratios when focus changes
- [ ] `tileport-core/monocle.rs`: Monocle layout
  - Single fullscreen window at a time
  - Carousel navigation (next/prev)
  - Track window order and focused index
- [ ] Built-in layout definitions:
  - `ultrawide-3col`: [0.30, 0.40, 0.30] — default for ultrawide
  - `laptop-2col`: [0.40, 0.60] — default for 15" MacBook
  - `monocle`: single window — default for 13" or small screens
  - `equal-2col`: [0.50, 0.50]
  - `equal-3col`: [0.33, 0.34, 0.33]
- [ ] Layout assignment to workspaces (config-driven)
- [ ] Apply gaps (inner + outer) to zone calculations
- [ ] **Unit tests**: Exhaustive testing
  - Insert 1..10 windows into each layout, verify no overlapping rects
  - Verify fill order matches spec (center → left → right → subdivide)
  - Verify zone rects sum to monitor dimensions minus gaps
  - Zen focus: verify focused column expands, others compress
  - Monocle: verify only focused window has visible rect

### Phase 3: Workspace System (Weeks 7-9)

**Goal**: Multiple workspaces with window hiding.

- [ ] `tileport-core/workspace.rs`: Workspace manager
  - Create/destroy workspaces
  - Switch workspace (hide current, show target)
  - Assign window to workspace
  - Track focused window per workspace
- [ ] Offscreen window hiding (move to bottom-right corner)
- [ ] Window restoration on switch (restore calculated positions)
- [ ] `tileport-macos/event_loop.rs`: AXObserver for window events
  - Window created → insert into focused workspace tree
  - Window destroyed → remove from tree, re-layout
  - Window title changed → update internal state
  - App activated → focus tracking
- [ ] Dialog detection heuristic (auto-float)
- [ ] Float/tile toggle with position memory
- [ ] `tileport-wm/recovery.rs`: Crash recovery
  - Persist window state to JSON on every layout change
  - On startup, check for orphaned offscreen windows
  - Restore all windows to visible on graceful shutdown

### Phase 4: Keybindings & Config (Weeks 10-12)

**Goal**: Full keyboard control with hot-reloadable config.

- [ ] `tileport-macos/hotkey.rs`: CGEventTap implementation
  - Dedicated thread with CFRunLoop
  - Key event matching against current binding mode
  - Consume matched events, pass-through unmatched
  - `CGPreflightListenEventAccess` permission flow
- [ ] Binding mode system (main, resize, service, custom)
- [ ] `tileport-core/config.rs`: TOML config with serde
  - Parse all config sections
  - Validate with helpful error messages
  - Generate default config on first launch
  - `tileport check-config` validation command
- [ ] Config hot-reload via `notify` file watcher
- [ ] Window rules engine (per-app float, workspace assignment, ignore)
- [ ] Pre-split direction command (`split horizontal` / `split vertical`)
- [ ] Command chaining in bindings (`["cmd1", "cmd2"]`)

### Phase 5: Monitor Profiles (Weeks 13-15)

**Goal**: The differentiating feature. Seamless dock/undock.

- [ ] `tileport-core/profile.rs`: Profile detection and matching
  - Monitor pattern matching (resolution glob)
  - Profile scoring (best match)
  - Fallback profile
- [ ] Layout state save/restore per profile per workspace
- [ ] Auto layout-mode switching (tiles ↔ accordion)
- [ ] Workspace redistribution on monitor change
- [ ] Manual profile switch command
- [ ] `exec-on-profile-change` callback
- [ ] Edge case handling:
  - Display sleep/wake (not a real disconnect)
  - Lid close/open
  - Rapid connect/disconnect (debounce 500ms)
  - Three monitors → two monitors → one monitor

### Phase 6: CLI, IPC & Workspace Attention (Weeks 16-18)

**Goal**: External scriptability, tool integration, and agent-awareness.

- [ ] `tileport-wm/ipc.rs`: Unix socket server (embedded in daemon, not separate crate)
  - Command handling (parse JSON, dispatch to manager, return result)
  - Event subscription (clients connect and receive JSON events)
  - Subscriber lifecycle (connect, subscribe to event types, disconnect)
- [ ] `tileport-cli`: Thin client binary with clap
  - All commands from spec (workspace, focus, move, layout, profile, etc.)
  - JSON output for scripting (`--json` flag)
  - Shell completion generation (bash, zsh, fish): `tileport completions zsh`
  - Subscribe mode: `tileport subscribe workspace-change focus-change`
  - Uses `std::os::unix::net::UnixStream` — no tokio dependency in CLI
- [ ] Workspace attention system (§4.11)
  - `attention: bool` field on Workspace struct
  - IPC commands: `workspace-attention --set/--clear <N|current>`
  - `workspace-attention next/prev` — quick-cycle through attention workspaces
  - Auto-clear on focus (configurable)
  - AX title-change heuristic: match window titles against configurable regex patterns
  - Debounce title changes (configurable, default 2000ms) to avoid attention fatigue
  - IPC event: `workspace-attention` broadcast
  - Callback: `exec-on-workspace-attention`
- [ ] Workspace naming (§4.11.3)
  - `display_name: Option<String>` field on Workspace struct
  - Static names via config, dynamic override via IPC
  - IPC command: `workspace-name --set <N> <name>`
  - IPC event: `workspace-name-change` broadcast
- [ ] Attention peek overlay (§4.11.5)
  - Transient overlay (NSWindow) showing workspace name + attention source
  - Configurable position and duration

### Phase 7: Zen Focus, Scratchpads & Paper Layout (Weeks 19-21)

**Goal**: Focus-zoom navigation, scratchpads, and scrollable layout for small screens.

- [ ] Zen focus mode implementation
  - On focus change: recalculate column ratios (focused expands, others compress)
  - Smooth animated transition between ratio states
  - Optional dim/opacity reduction on non-focused windows
  - Monocle-compatible: zen on monocle = slide focused window in/out
- [ ] Special workspace implementation (scratchpads)
  - Toggle show/hide with configurable keybinding
  - Positioning: center, top, bottom, corners
  - Size: configurable width/height as ratio of screen
  - Animation: slide-down, slide-up, fade
  - Window persists when hidden (not closed, just offscreen)
- [ ] Paper layout implementation (§4.12)
  - `tileport-core/paper.rs`: Paper layout engine
  - Windows placed side-by-side at preferred size, viewport scrolls to center focused window
  - Focused window gets configurable ratio of screen (default 85%)
  - Adjacent windows peek from edges (configurable peek width in px)
  - Navigation: same alt+h/l keybindings scroll viewport left/right
  - New windows append to the right of focused window
  - Close fills gap by sliding remaining windows
  - Smooth slide animation (100-150ms)
  - Integrates with monitor profiles: auto-switch to paper on undock
- [ ] Auto-layout selection based on monitor size
  - Below 14" (2560×1600 or smaller) → paper by default
  - 14-16" (2880×1800) → laptop-2col by default
  - Above 20" / ultrawide → ultrawide-3col by default
  - User can override per workspace in config
- [ ] **Unit tests**: Paper layout
  - Insert 1..10 windows, verify viewport calculations
  - Verify focused window is centered with correct size
  - Verify peek windows have correct positions
  - Verify navigation updates viewport correctly

### Phase 8: Polish & Release (Weeks 22-24)

**Goal**: Daily-drivable v1.0.

- [ ] `tileport-macos/tray.rs`: Menu bar icon
  - Workspace indicator with attention dots (§4.11.4)
  - Mode indicator
  - Profile indicator
  - Click menu with workspace list (showing names + attention state), reload, quit
- [ ] `tileport-macos/border.rs`: Focus border overlay
- [ ] `tileport-macos/animation.rs`: Smooth window transitions
- [ ] Window finder (fuzzy search all managed windows)
- [ ] Default config tuned for MacBook Air 15" + ultrawide 38"
- [ ] Homebrew formula: `brew install --cask aitechnerd/tap/tileport`
- [ ] README with installation, quick start, config reference
- [ ] Manual testing across common apps:
  - VS Code, Firefox/Chrome/Arc, Ghostty/WezTerm/iTerm2
  - Slack, Discord, Spotify
  - Finder, System Settings, Preview
  - Electron apps (special handling)
  - Java apps (IntelliJ — known AX quirks)
- [ ] App-specific heuristic database for dialog detection
- [ ] Nix flake for home-manager integration (optional)

---

## 7. Risks & Mitigations

| Risk | Severity | Mitigation |
|------|----------|------------|
| AX API inconsistency across apps | High | App-specific heuristic DB; user-defined window rules; study Aerospace & Paneru source |
| AXObserver notifications unreliable | High | Polling fallback (250ms configurable); CGWindowList as secondary source; dual-mode detection |
| CGEventTap disabled by macOS security updates | Medium | Monitor Apple developer forums; fallback to `RegisterEventHotKey` (Carbon, deprecated but universal) |
| USB-C dock hotplug events flaky | Medium | Manual profile switch hotkey; 500ms debounce; multiple re-detection attempts |
| objc2 ecosystem pre-1.0 | Medium | Pin exact versions; the ecosystem is actively maintained and used by Tauri/Paneru/komorebi |
| Offscreen window pixel leak | Low | Minimize sliver size; document limitation; restore on quit |
| Performance: AX calls blocking | Medium | Batch AX operations; async where possible; measure and optimize hot paths |
| macOS update breaks behavior | Low | Only one private API used; monitor macOS betas; community testing |

---

## 8. Non-Goals for v1

Explicitly out of scope:

- **Plugin API**: Keep it simple until architecture stabilizes
- **Mouse-driven tiling / drag zones**: Keyboard-first for v1
- **Custom window decorations**: macOS doesn't support this without hacks
- **GUI preferences**: TOML config only
- **Linux / Windows support**: macOS only, forever (other WMs serve those platforms well)
- **Built-in status bar**: Use Sketchybar via IPC callbacks
- **Wallpaper management**: Use existing tools
- **Notification management**: Out of scope (workspace attention flags are not notifications — they're simple boolean signals)
- **App launching**: Use Raycast/Alfred/Spotlight

---

## 9. Post-v1 Roadmap

- Named layout presets: save/recall arrangements ("coding", "presenting", "reviewing")
- BSP layout mode (optional, for users who prefer i3-style dynamic splitting)
- Master-stack layout mode (dwm-style: one large + stack on side)
- Mouse drag to resize column boundaries
- Picture-in-picture window mode (small floating window that stays on top)
- Multi-display workspace spanning (one workspace across two monitors)
- Window groups / tabbed containers (multiple windows in one zone slot)
- `tileport-homemanager` Nix module for declarative configuration
- Touchpad gesture integration (three-finger swipe for workspace switching)
- Accessibility audit: screen reader compatibility for tileport's own UI elements
- Smart layout auto-detection: analyze monitor resolution and auto-pick best layout

---

## 10. How to Build & Run (Development)

```bash
# Clone
git clone https://github.com/aitechnerd/tileport.git
cd tileport

# Build (requires macOS + Xcode Command Line Tools)
cargo build

# Run the WM daemon
cargo run --bin tileport-wm

# Run the CLI client
cargo run --bin tileport-cli -- workspace 3

# Run tests (core crate works on any platform)
cargo test -p tileport-core

# Run macOS integration tests (requires accessibility permission)
cargo test -p tileport-macos
```

---

## 11. Installation (End User)

```bash
# Homebrew (primary)
brew install --cask aitechnerd/tap/tileport

# From source
cargo install --git https://github.com/aitechnerd/tileport.git tileport-wm tileport-cli

# Nix (via home-manager)
# In your flake.nix:
# inputs.tileport.url = "github:aitechnerd/tileport";
# programs.tileport.enable = true;
```

First launch:
1. macOS asks for Accessibility permission → grant
2. macOS asks for Input Monitoring permission → grant
3. tileport generates default config at `~/.config/tileport/tileport.toml`
4. Windows tile automatically. Start pressing `alt+1..9` to switch workspaces.

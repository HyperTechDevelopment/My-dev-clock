# my-dev-clock

**English** · [Português](README.md)

A terminal clock for Windows, with a timer, a stopwatch, a countdown and a row of
panels that turn the terminal into a status board.

Written in Rust, native to Windows: it does not depend on `sh`, on a bash script
or on any Linux system utility.

**Contents**

- [Installation](#installation)
- [Getting started](#getting-started)
- [Keys](#keys)
- [Modes](#modes)
- [Appearance](#appearance)
- [Split terminal](#split-terminal)
- [Widgets](#widgets)
- [Ready-made recipes](#ready-made-recipes)
- [Configuration](#configuration)
- [Command line](#command-line)
- [Diagnostics and scripting](#diagnostics-and-scripting)
- [Common problems](#common-problems)
- [How it works](#how-it-works)
- [Development](#development)

## Installation

Download `my-dev-clock.exe` from the [releases page](../../releases/latest) and
drop it in any folder. It is a single executable, with no installer and nothing
else to install:

```powershell
.\my-dev-clock.exe
```

On the first run Windows shows the "Windows protected your PC" warning, because
the binary has no digital signature. Click **More info** and then **Run anyway**;
the warning does not come back.

To call the program from any folder, put the `.exe` in a folder that is already
on your `PATH`.

### Building from source

You need stable Rust and the Visual Studio C++ toolset, which provides the
Windows linker.

```powershell
cargo build --release
.\target\release\my-dev-clock.exe
```

If `cargo` complains that it cannot find the linker, use the script that enters
the Visual Studio environment before calling cargo:

```powershell
.\build.cmd build --release
```

It forwards any cargo command, so `.\build.cmd test` and
`.\build.cmd clippy --all-targets -- -D warnings` work too.

## Getting started

Without a configuration file the app opens just the clock, filling the whole
screen. Widgets come from the configuration, so the first step is to create one:

```powershell
my-dev-clock --init
```

That writes a commented `config.toml` to `%APPDATA%\my-dev-clock\`. Open the file,
uncomment a few widgets and run it again. A small start:

```toml
[clock]

[[clock.widgets]]
kind = "cpu"
title = "CPU"

[[clock.widgets]]
kind = "mem"
title = "RAM"

[[clock.widgets]]
kind = "git"
title = "Git"
refresh_secs = 15
```

With the app open, four keys already change a lot:

| Key | What it does |
| --- | --- |
| `?` | Shows the full key list, inside the app itself |
| `Shift+T` | Next theme |
| `Shift+F` | Next digit style |
| `Shift+A` | Next background animation |

Each of those shows what just changed, for a moment, in the footer. If a key has
no visible effect, it says so — `Shift+L`, for example, reports that the clock
already fills the screen when there are no widgets on it.

Saving `config.toml` **reloads the configuration right away**, with no restart.
You can tweak a color or swap a widget with the clock in front of you.

## Keys

| Key | What it does | Where |
| --- | --- | --- |
| `q` | Quits | always |
| `space` | Pauses and resumes | timer and stopwatch |
| `c` / `w` / `t` | Goes to clock / stopwatch / timer | always |
| `Shift+T` | Next theme | always |
| `Shift+F` | Next digit style | always |
| `Shift+A` | Next background animation | always |
| `Shift+L` | Full screen on/off | clock mode |
| `z` | Hides or shows all widgets | clock mode |
| `s` | Opens the field to type the digit size | always |
| `?` | Shows the key list (and `Esc` closes it) | always |
| `l` | Records a lap | stopwatch |
| `n` | Skips to the next interval | timer |
| `+` or `=` / `-` | Adds or removes a minute from the current interval | timer |
| `r` | Restarts from the first interval | timer |
| `Home` / `End` | Goes to the top or the bottom of the focused widget | clock mode |
| `PageUp` / `PageDown` | Scrolls the focused widget by one page | clock mode |
| mouse wheel | Scrolls the widget under the cursor | clock mode |

In the size field, type a number from 1 to 12 and confirm with `Enter`; the clock
resizes as you type, and `Esc` cancels. The four appearance shortcuts keep working
with the field open.

## Modes

| Mode | What it shows |
| --- | --- |
| `clock` | Current time, with the date in Portuguese and an optional timezone. This is the default. |
| `timer` | Time remaining, with sequential durations, repetition and a command at the end. |
| `stopwatch` | Elapsed time, with pause and laps. On exit it prints the total time and the laps. |
| `countdown` | Time remaining until a time or a date. |

```powershell
my-dev-clock clock --timezone America/Sao_Paulo
my-dev-clock timer -d 25m 5m -T Focus Break
my-dev-clock stopwatch --no-millis
my-dev-clock countdown -t 2026-01-01 -T 'New year'
```

**Timer.** It accepts several durations and runs them in sequence. The durations
come from, in this order: `--duration`, the `durations` key in the configuration,
and finally 5 minutes. While it runs: `n` skips the interval, `+` and `-` add or
remove a minute, `r` goes back to the start keeping the state, and `space` pauses.
When time is up, the screen flashes and the `execute` command runs, if there is
one.

**Stopwatch.** `l` records a lap, and the footer shows the lap in progress while it
runs. On exit, every lap appears on screen, and the `laps_csv` file gets all of it
as CSV:

```toml
[stopwatch]
title = "Benchmark"
laps_csv = "laps.csv"
```

```
lap,lap_s,total_s
1,12.480,12.480
2,11.901,24.381
```

**Countdown.** `time` accepts `2026-01-01`, `20:00`, `2026-12-25 20:00:00` or an
RFC3339 timestamp. `continue_on_zero` keeps counting past the target and `reverse`
counts up instead of down.

### Alert when time runs out

`bell = true` in `[timer]` or `[countdown]` rings the terminal bell, on top of the
screen flash. For something stronger, `execute` runs a command at that moment —
two recipes verified on this machine:

```toml
[timer]
# A short beep, with no file to depend on.
execute = ["powershell", "-NoProfile", "-Command", "[console]::beep(880,300)"]

# A real sound, from Windows itself.
# execute = ["powershell", "-NoProfile", "-Command", "(New-Object Media.SoundPlayer 'C:\\Windows\\Media\\Alarm01.wav').PlaySync()"]
```

## Appearance

### Digit styles

Four styles, cycled with `Shift+F` or picked with `--font`. The samples below are
frames from the renderer itself.

**bricks** — solid blocks, the original style.

```
          ████████      ████████████                ████████████  ████████████
          ████████      ████████████                ████████████  ████████████
              ████              ████      ████      ████    ████          ████
              ████              ████      ████      ████    ████          ████
              ████      ████████████                ████    ████  ████████████
              ████      ████████████                ████    ████  ████████████
              ████              ████      ████      ████    ████          ████
              ████              ████      ████      ████    ████          ████
          ████████████  ████████████                ████████████  ████████████
          ████████████  ████████████                ████████████  ████████████
```

**slim** — a thin stroke, taller and lighter.

```
                      ██    ██████                ████    ██████
                      ██    ██████                ████    ██████
                    ████          ██    ████    ██    ██        ██
                    ████          ██    ████    ██    ██        ██
                      ██          ██    ████    ██    ██        ██
                      ██          ██    ████    ██    ██        ██
                      ██      ████              ██    ██    ████
                      ██      ████              ██    ██    ████
                      ██          ██    ████    ██    ██        ██
                      ██          ██    ████    ██    ██        ██
                      ██          ██    ████    ██    ██        ██
                      ██          ██    ████    ██    ██        ██
                    ██████  ██████                ████    ██████
                    ██████  ██████                ████    ██████
```

**slant** — the same blocks, leaning like an italic face.

```
    ████████          ████████████                        ████████████      ████████████
    ████████          ████████████                        ████████████      ████████████
      ████                  ████          ████          ████    ████              ████
      ████                  ████          ████          ████    ████              ████
    ████          ████████████                        ████    ████      ████████████
    ████          ████████████                        ████    ████      ████████████
    ████                  ████          ████          ████    ████              ████
    ████                  ████          ████          ████    ████              ████
████████████      ████████████                        ████████████      ████████████
████████████      ████████████                        ████████████      ████████████
```

**flip** — each digit on a card filled with the clock color, the digit drawn dark
on top and a hinge line cutting through the middle. The frame shows only the
shapes: in the terminal the cards appear in color, with the hinge.

```
  ████████          ████████████                        ████████████      ████████████
  ████████          ████████████                        ████████████      ████████████
      ████                  ████          ████          ████    ████              ████
      ████                  ████          ████          ████    ████              ████
      ████          ████████████                        ████    ████      ████████████
      ████          ████████████                        ████    ████      ████████████
      ████                  ████          ████          ████    ████              ████
      ████                  ████          ████          ████    ████              ████
  ████████████      ████████████                        ████████████      ████████████
  ████████████      ████████████                        ████████████      ████████████
```

### Flip animation

The `flip` face turns the digit flip on by itself: when a digit changes, the
bottom half drops first and the top half follows, like an airport board. To use
it with any face, turn it on by hand.

```toml
[clock]
font = "flip"
flip = true      # turns it on even without the flip face
flip_ms = 180    # how long the flip takes; 0 turns it off
```

### Background animations

Drawn behind the digits, always in a darkened style so they do not get in the way
of reading. Cycled with `Shift+A` or picked with `--animation`.

**matrix** — columns of falling characters, with the head brighter than the tail.

```
 ﾂ  ｶ       ﾓ ﾒ ﾑ     ﾓ  ﾋ ｵﾓﾂｹ    ｻ ｰ  ﾎﾑﾓ  ｹ   ﾓ    ﾏｹﾑ ﾅｶ    ﾓ  ｷﾜﾅﾅ ｹﾆ   ﾎﾎｷﾜ ﾘ ｶｷ ﾅ ｻ
 ﾘ ｼｵ       ﾅ ﾋﾏﾅ  ｻｹﾐ     ﾊ ｰﾊ ﾜ  ｰ    ｰ ﾂ ｶ    ｳ    ｼｻﾐｶｳｶ   ﾐﾜ  ｱｼﾑ  ﾑ     ﾅﾋ   ﾜｻｶ ｹ ﾊ
 ﾆ ﾋﾐ     ████████ ﾅｰﾏﾒ ████████████             ﾐ  ████████████ﾅ ████████████    ｷ  ﾋ ｰ ﾋ
 ｼ ｵ  ﾐ   ████████  ﾂﾐﾐ ████████████      ﾐ ｰ       ████████████ｼ ████████████    ﾒﾓ ｹ
 ﾅ ﾂ  ﾑ  ﾏ  ﾎ ████ ﾎｷｰｻ ｶ   ﾂ   ████  ｱ   ████    ﾅ ████ｷﾑ  ████ｶ  ｷｴ ｳｶ  ████    ﾘﾎ ｱ
   ﾑ  ﾋ    ｶｷ ████ ﾏｷﾏﾐ ﾓ   ﾂ   ████      ████ ﾓ  ﾓｷ████ﾑﾘ  ████ｶ ﾅﾜﾓﾊ    ████     ｱ ﾓ
         ﾅ ﾒﾊ ████ ﾜｰﾆﾅ ████████████ｰ ﾜ   ﾓ  ﾊ ｼ  ﾅ ████ ﾆ  ████  ████████████    ﾎｰ
      ﾐ ﾎﾒ ﾂｼ ████ ﾃﾐ ｷ ████████████ﾜ ﾎ     ﾂﾊ     ﾐ████ ｻ  ████  ████████████    ﾎｶ
     ﾘﾑ ｱｹﾋｷｴ ████ ﾒﾐ   ﾘ      ﾆ████  ﾘ   ████ ﾐ  ｻﾜ████ ﾆ  ████  ﾏ       ████    ﾎﾏ
     ﾂｼ  ｰｱｴ  ████ ﾂﾒ   ﾑ      ｻ████ｶ ﾎ   ████ ﾎ  ﾘﾎ████    ████  ｹ    ﾆ  ████     ﾐ
  ﾂ  ｹﾃ  ｹ████████████  ████████████ｷ ｳ       ﾐﾂ  ﾒﾓ████████████  ████████████   ｹ
  ﾎ  ﾏ   ﾊ████████████  ████████████  ｳ       ﾎｹﾐ ﾐｻ████████████  ████████████   ｻ
ﾐ ﾐ     ﾋﾊﾓ      ﾅﾏ              ｷﾎ ﾐ ﾂ       ﾎ    ﾒ ｵ        ﾏ           ｼ ｴ    ﾒ    ｰ
  ﾏ  ｷ  ﾆ    ｻ    ﾘ               ﾘ ﾊ         ｱ ｶ  ﾃ ﾋ       ﾒ            ｼｴﾘ         ｶ
```

**plasma** — sine fields added together, mapped onto a brightness ramp.

```
+++*****#####################******+++++=======------------------------===========++++++++
+*****#######%%%%%%%%%%#######******+++++======----------------------==========+++++++++++
****######████████%%%%%%████████████++++++======----████████████--████████████+++++++++***
**#####%%%████████%%%%%%████████████*+++++======----████████████--████████████++++********
#####%%%%%%%%%████%%%%%%%%%#####████*+++++████===---████----████========++████+***********
###%%%%%%%%@@@████@@%%%%%%%%####████*+++++████===---████----████======++++████*******#####
##%%%%%%%@@@@@████@@@%%%████████████*+++++=======---████----████==████████████****########
#%%%%%%%@@@@@@████@@@%%%████████████*+++++======----████----████==████████████**##########
%%%%%%%@@@@@@@████@@@%%%%%%#####████+++++=████==----████----████====+++++*████*#########%%
%%%%%%%@@@@@@@████@@%%%%%%#####*████++++==████=-----████----████===+++++**████#######%%%%%
%%%%%%%@@@████████████%%████████████+++======-------████████████==████████████######%%%%%%
%%%%%%%%%@████████████%%████████████++======--------████████████==████████████#####%%%%%%%
%%%%%%%%%%%%%%%%%%%%%%#####*****+++++=====-------------------======++++*****#######%%%%%%%
##%%%%%%%%%%%%%%%%%######*****+++++=====---------------------======+++++****#######%%%%%%%
```

**waves** — concentric rings spreading out from the center.

```
%@@@%#*+=:.    .:-=*#%%@@@%%#*+=-:..                 ..:-=+*#%%@@@%%#*=-:.    .:=+*#%@@@%#
@@@@%#+=-:.   .:-=*#%@@@@%#*+=-:..     ....:::....     ..:-=+*#%@@@@%#*=-:.   .:-=+#%@@@@%
@@@%#*+-:.████████#%@@@@████████████..:--=======--:.████████████%@████████████ .:-+*#%@@@%
@@@%#+=-..████████#%@@@%████████████:-=+**#####**+=-████████████#%████████████ ..-=+#%@@@%
@@%#*+-:.   .:████%@@@%#*+=:.   ████=+*#%%████@%%#*+████    ████*#%@@@%#*=████  .:-+*#%@@@
@@%#*=-:.   .:████%@@@%#*=-:.   ████*#%%@@████@@@%%#████.   ████*#%@@@%#*+████  .:-=*#%@@@
@@%#*=-:.   .:████%@@@%#████████████#%@@@@%###%@@@@%████..  ████+#████████████  .:-=*#%@@@
@@%#*=-:.   .:████@@@%%*████████████#%@@@%#*+*#%@@@%████:.  ████+*████████████  .:-=*#%@@@
@@%#*=-:.   .:████%@@@%#+=:.   .████#%@@@@████%@@@@%████..  ████+#%@@@%%*+████  .:-=*#%@@@
@@%#*=-:.   .:████%@@@%#*=-:.   ████*#%%@@████@@@%%#████.   ████*#%@@@%#*+████  .:-=*#%@@@
@@%#*+-:. ████████████%#████████████=+*#%%@@@@@%%#*+████████████*#████████████  .:-+*#%@@@
@@@%#+=-..████████████@%████████████:-=+**#####**+=-████████████#%████████████ ..-=+#%@@@%
@@@%#*+-:.   .:-=+#%@@@@%#*+-:..    ..:--=======--:..    ..:-+*#%@@@@%#+=-:.   .:-+*#%@@@%
@@@@%#+=-:.   .:-=*#%@@@@%#*+=-:..     ....:::....     ..:-=+*#%@@@@%#*=-:.   .:-=+#%@@@@%
```

**stars** — a sparse field of stars blinking in place.

```
-   -                                       -  - =
            -                      =                                                    -
          ████████      ████████████                ████████████  ████████████    =
          ████████      ████████████                ████████████  ████████████        -
    -         ████              ████      ████ =    ████    ████  -     - ████      -
              ████              ████      ████      ████    ████          ████
              ████      ████████████                ████    ████  ████████████
              ████      ████████████        - -     ████    ████  ████████████
              ████              ████      ████      ████    ████          ████
              ████              ████      ████      ████    ████          ████
          ████████████  ████████████                ████████████  ████████████
  =       ████████████  ████████████                ████████████  ████████████
-                                                   -
     -                                   =                    -
```

Time advances in whole frame steps (`animation_fps`, 12 by default), which gives
the choppy motion of a terminal and caps the drawing cost. The same seed and the
same instant always draw the same frame, and that is what makes the animation
testable instead of something you can only look at.

### Full screen

With widgets on screen, the clock sits in the top half. With `fill`, it takes
everything the widget row does not need.

```toml
[clock]
fill = true
```

Toggled at runtime with `Shift+L`. It only changes anything when there are widgets
on screen; with no widgets (none configured, or hidden with `z`) the clock already
fills everything, and the confirmation says so.

### Themes

| Theme | Look |
| --- | --- |
| `default` | Uses the color asked for in `--color`, or green. |
| `matrix` | Phosphor green. |
| `neon` | Magenta and cyan, high contrast. |
| `system` | The terminal's own colors, with no palette imposed. |
| `fire` | Orange and red. |
| `ice` | Light blues. |

The cycle list comes from `[clock] themes`, and `--theme` picks the one the app
opens with. The theme also sets the color of the background animation (always
darkened) and the widget colors.

A palette of your own comes in as a theme named `custom`:

```toml
[clock]
themes = ["custom", "matrix", "neon"]

[theme.custom]
clock = "#00ff88"       # digits
header = "#cccccc"      # top line and confirmations
widget = "#88aaff"      # widget body
title = "#ffffff"       # widget title
marker = "#ffcc00"      # dot in front of the title
accent = "#ff00aa"      # bars and highlights
animation = "#004422"   # background animation, always darkened
```

Each color accepts a name (`green`, `lightyellow`, …) or `#RRGGBB`, and whatever
is missing comes from the default theme.

## Split terminal

Splitting the window, or resizing it, must not hide anything. When the width
changes, the app rearranges itself:

- Less width means **fewer columns and more rows**. Four widgets in a wide window
  are four columns; in a terminal at half width they become two columns and two
  rows, and all four stay on screen.
- Each widget row is as tall as the tallest widget in it, so the space does not
  end up full of holes.
- If even that does not fit everything, a `+N panel(s) with no room` strip appears
  in the widget footer, in reverse video, instead of quietly dropping them.
- In a panel too narrow for the block digits — under 62 columns for `HH:MM:SS` —
  the time becomes plain centered text, and stays readable instead of getting cut
  in half.
- When the size changes, the area is cleared before the next frame, so no piece of
  the previous layout is left behind.

Only the widgets on screen actually run. A panel that ran out of room consumes no
process and starts running again when the space shows up.

## Widgets

Declared in `[[clock.widgets]]`, they appear below the clock in 2, 4 or 6 columns,
depending on the width (capped by `max_columns`).

| `kind` | What it shows | What it needs |
| --- | --- | --- |
| `cpu` | Processor usage, average across cores, with a bar. | nothing |
| `mem` | RAM used and total. | nothing |
| `disk` | Disk space, with a bar. | nothing (accepts `mount`) |
| `net` | Traffic since boot, plus the rate since the previous read. | nothing (accepts `interface`) |
| `git` | Branch, dirty state, ahead of and behind the remote, stash. | being in a repository, or `path` |
| `commits` | Your latest commits, across every project scanned. | `roots` or `paths` |
| `http` | Status and a value pulled from the JSON of an address. | `url` |
| `weather` | The forecast for now and the next few days. | `city` or `lat`/`lon` |
| `gh` | What is pending for you on GitHub. | `gh auth login` |
| `actions` | The latest CI runs. | `gh auth login` |
| `command` | Any command that prints something and exits. | `command` |

Options on every widget: `title` (title above the content; `""` hides the title
line), `refresh_secs` (how often it runs, default 900) and `timeout_secs` (time
limit, default 30).

### System: cpu, mem, disk, net

Native, with no script and no configuration. The disk picked is the one with the
least free space, unless you point at one:

```toml
[[clock.widgets]]
kind = "disk"
title = "Disk C:"
refresh_secs = 60
[clock.widgets.disk]
mount = "C:"

[[clock.widgets]]
kind = "net"
title = "Network"
refresh_secs = 30
[clock.widgets.net]
interface = "Ethernet"
```

Without `interface`, the network widget adds up every interface except loopback.

### Repository: git and commits

`git` looks at the repository of the current directory (or the one in `path`) and
shows everything in a single call:

```toml
[[clock.widgets]]
kind = "git"
title = "Git"
refresh_secs = 15
[clock.widgets.git]
path = "C:/dev/project"
```

```
main * ↑2 ↓1 stash 3
4 pending files
```

`commits` looks outside the open repository: it scans the configured folders,
finds the repositories and shows your most recent commits across all of them.

```toml
[[clock.widgets]]
kind = "commits"
title = "My commits"
refresh_secs = 300
[clock.widgets.commits]
roots = ["C:/dev"]                 # folders where to look for repositories
paths = ["C:/loose/repo"]          # or standalone repositories
limit = 5
depth = 2
author = "me@example.com"
```

```
online-store · 23 h · Merge pull request #37 from feature/cart
payment-api · 2 d · feat: notification for approved payment
```

Each line is `project · when · subject`. The project is the repository folder
name, and the "when" uses the commit's author date. `roots` are folders scanned up
to `depth` levels; `paths` are standalone repositories. `author` accepts a name or
an email and, without it, the widget asks git for the configured `user.email`; the
comparison ignores case, because the name stored in the commit rarely matches the
one in the configuration. The folder scan is cached for five minutes.

### GitHub: gh and actions

`gh` groups what is pending by category, with a count line at the top, and each
item carries what helps you decide whether it is worth opening: the project, how
long it has been sitting there, how many comments it has and its state.

```toml
[[clock.widgets]]
kind = "gh"
title = "Inbox"
refresh_secs = 600
[clock.widgets.gh]
limit = 3
```

```
2 reviews · 1 of my PRs · 0 issues
my PRs
#1 Adjust the shipping calculation for Nor… · my-org/online-store · 1 month · 3 comments
```

- **reviews**: PRs waiting for your review. They show who wrote them, because it
  is not you.
- **my PRs**: yours, open. They carry `draft`, `approved`, `changes requested` or
  `awaiting review` — you can see what is blocked and by whom.
- **issues**: the ones assigned to you.

A category with no items gets no header. Everything comes from **one** GraphQL
call.

`actions` shows the latest CI runs, marked with `✓`, `✗`, `…` (running) or `×`
(cancelled):

```toml
[[clock.widgets]]
kind = "actions"
title = "CI"
refresh_secs = 120
[clock.widgets.gh]
repo = "owner/project"    # without this, the repository of the current directory
```

Both need the authenticated `gh` CLI (`gh auth login`).

### Addresses: http

To watch a dev server, with the value pulled from the JSON:

```toml
[[clock.widgets]]
kind = "http"
title = "API"
refresh_secs = 30
timeout_secs = 5
[clock.widgets.http]
url = "http://localhost:3000/health"
path = ".status"          # dotted path, accepts an index: .items.0.name
expect = "ok"             # optional: the widget reports whether the value matches
```

```
HTTP 200 · 12 ms
ok
expected: ok
```

A `4xx` or `5xx` response is **not** treated as a widget error: it is the
information a health check exists to give.

### Weather

```toml
[[clock.widgets]]
kind = "weather"
title = "Weather"
refresh_secs = 900
[clock.widgets.weather]
city = "Curitiba"   # or lat / lon
units = "metric"    # metric (°C, km/h) or imperial (°F, mph)
days = 2            # forecast rows, from 1 to 7
```

```
22.4°C · partly cloudy
feels like 21.8° · humidity 78% · wind 11 km/h
today   17.1° / 25.3°
tomorrow 16.4° / 26.0°
```

It uses [Open-Meteo](https://open-meteo.com), which asks for no API key.

### Any command

```toml
[[clock.widgets]]
kind = "command"
title = "Battery"
command = "powercfg /batteryreport /output -"
refresh_secs = 300
timeout_secs = 20
```

A single string goes through the shell (`cmd /C`); a list runs the program
directly, with no shell, which is the right path when the command has quotes:

```toml
command = ["powershell", "-NoProfile", "-Command", "$v = Get-Volume -DriveLetter C; '{0:N1} GB free' -f ($v.SizeRemaining/1GB)"]
```

Every command receives the current theme in the `MY_DEV_CLOCK_THEME` variable. A
command that goes past the time limit is terminated **along with everything it
started**, through a Windows Job Object, so closing the app leaves no process
hanging around.

## Ready-made recipes

**Pomodoro**, with a sound alert and repetition:

```toml
[default]
mode = "timer"
color = "lightred"

[timer]
durations = ["25m", "5m"]
titles = ["Focus", "Break"]
bell = true
repeat = true

[[clock.widgets]]
kind = "gh"
title = "Inbox"
```

**Dev dashboard**, to leave on a side monitor:

```toml
[clock]
font = "flip"
max_columns = 5

[[clock.widgets]]
kind = "git"
title = "Repo"
refresh_secs = 15

[[clock.widgets]]
kind = "commits"
title = "My commits"
refresh_secs = 300
[clock.widgets.commits]
roots = ["~/dev"]   # swap for the folder where your repositories live

[[clock.widgets]]
kind = "gh"
title = "Inbox"
refresh_secs = 600

[[clock.widgets]]
kind = "actions"
title = "CI"
refresh_secs = 120

[[clock.widgets]]
kind = "cpu"
title = "CPU"
refresh_secs = 3
```

**Server monitor**, with the health check up front:

```toml
[clock]
max_columns = 4

[[clock.widgets]]
kind = "http"
title = "API"
refresh_secs = 30
[clock.widgets.http]
url = "http://localhost:3000/health"   # point it at your server
path = ".status"
expect = "ok"

[[clock.widgets]]
kind = "cpu"
title = "CPU"
refresh_secs = 3

[[clock.widgets]]
kind = "mem"
title = "RAM"
refresh_secs = 5

[[clock.widgets]]
kind = "disk"
title = "Disk"
refresh_secs = 60

[[clock.widgets]]
kind = "net"
title = "Network"
refresh_secs = 10
```

**Wall clock**, just the clock, big and with an animated background:

```toml
[default]
mode = "clock"
size = 6
show_seconds = false

[clock]
fill = true
animation = "matrix"
animation_fps = 10
```

That last one goes well with `--theme matrix` on the command line, or write
`themes = ["matrix"]`.

## Configuration

The app reads the first file that exists, in this order:

1. the path passed in `--config <file>`
2. `%XDG_CONFIG_HOME%\my-dev-clock\config.toml`, when the variable points at an absolute path
3. `%APPDATA%\my-dev-clock\config.toml`
4. `%USERPROFILE%\.config\my-dev-clock\config.toml`

The precedence of the display options is: **command-line flag**, then the mode
section, then `[default]`, and finally the built-in default for that mode.

Built-in defaults: the clock shows the date and seconds; timer and stopwatch show
tenths; the countdown shows whole seconds.

### `[default]`

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `mode` | text | `"clock"` | Initial mode: `clock`, `timer`, `stopwatch`, `countdown`. An unknown name falls back to the clock. |
| `color` | text | `"green"` | Base color of the digits: a name (`green`, `cyan`, `lightred`, …) or `"#RRGGBB"`. |
| `size` | number | absent | Digit scale, 1 to 12. Absent means fit to the window. |
| `title` | text | absent | Title above the digits. |
| `show_date` | boolean | per mode | Shows the date line on the clock. |
| `show_seconds` | boolean | per mode | Shows the seconds; off, every mode becomes `H:MM`. |
| `show_millis` | boolean | per mode | Shows tenths of a second. |
| `start_paused` | boolean | `false` | Starts paused (timer and stopwatch). |

### `[clock]`

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `font` | text | `"bricks"` | Digit style: `bricks`, `slim`, `slant`, `flip`. |
| `flip` | boolean | follows the font | Turns the digit flip on. Absent: on for the `flip` face, off for the others. |
| `flip_ms` | number | `180` | Duration of the flip, in milliseconds. `0` turns it off. |
| `animation` | text | `"none"` | Background animation: `none`, `matrix`, `plasma`, `waves`, `stars`. |
| `animation_fps` | number | `12` | Frames per second of the animation, from 1 to 60. |
| `fill` | boolean | `false` | The clock takes the whole area instead of just the top half. |
| `format` | text | derived | Time format in the chrono syntax, for example `"%H:%M:%S"`. `"%I:%M %p"` gives a 12-hour clock with AM/PM. An invalid format is refused with a warning. |
| `date_format` | text | long form | Date line format in the chrono syntax, for example `"%d/%m/%Y"`. |
| `world` | list | empty | Extra timezones shown next to the date, as IANA names: `["America/New_York", "Europe/Lisbon"]`. |
| `timezone` | text | absent | IANA timezone, for example `"America/Sao_Paulo"`. |
| `themes` | list | the built-in list | Names that `Shift+T` cycles through. |
| `max_columns` | number | no limit | Cap on widget columns (the terminal also has a say). |
| `widgets` | list of tables | empty | The panels. |

`[clock]` also accepts the display keys (`title`, `show_date`, `show_seconds`,
`show_millis`, `start_paused`).

### `[timer]`

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `durations` | list | empty → 5m | Durations in sequence, e.g. `["25m", "5m"]`. |
| `titles` | list | empty | One title per duration; it wins over `title`. |
| `repeat` | boolean | `false` | Restarts when it finishes. |
| `auto_quit` | boolean | `false` | Quits the app when time runs out. |
| `execute` | text or list | empty | Command to run when time runs out. |
| `bell` | boolean | `false` | Rings the terminal bell when time runs out. |

### `[stopwatch]` and `[countdown]`

`[stopwatch]` accepts the display keys plus `laps_csv` (path to a file where the
laps are written as CSV on exit). `[countdown]` accepts the display keys plus
`time` (text: `2026-01-01`, `20:00`, `2026-12-25 20:00:00` or RFC3339),
`continue_on_zero`, `reverse` and `bell`.

### `[theme.custom]`

A palette of your own, used when the active theme is named `custom`. The keys are
`clock`, `header`, `widget`, `title`, `marker`, `accent` and `animation`.

### `[[clock.widgets]]`

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `kind` | text | required | Panel type, from the [Widgets](#widgets) table. |
| `title` | text | command name | Panel title; `""` hides the title line. |
| `command` | text or list | — | Only for `kind = "command"`. |
| `refresh_secs` | number | `900` | Interval between runs. |
| `timeout_secs` | number | `30` | Time limit for the command. |

Sub-tables: `[clock.widgets.git]` with `path`; `[clock.widgets.commits]` with
`roots`, `paths`, `author`, `limit` and `depth`; `[clock.widgets.gh]` with `limit`
and `repo`; `[clock.widgets.http]` with `url`, `path` and `expect`;
`[clock.widgets.weather]` with `city`, `lat`, `lon`, `units` and `days`;
`[clock.widgets.disk]` with `mount`; `[clock.widgets.net]` with `interface`.

## Command line

Display options, which apply in any mode and before or after the mode name:

| Option | Effect |
| --- | --- |
| `-T, --title <TEXT>...` | Title above the digits. In the timer, one per duration. |
| `--seconds` / `-S, --no-seconds` | Shows or hides the seconds. |
| `-m, --millis` / `-M, --no-millis` | Shows or hides the tenths. |
| `--date` / `-D, --no-date` | Shows or hides the date line. |
| `-P, --paused` | Starts paused. |
| `-c, --color <COLOR>` | Base color, a name or `#RRGGBB`. |
| `-s, --size <1-12>` | Digit scale. |
| `--theme <NAME>` | Initial theme. |
| `--font <NAME>` | Digit style. |
| `--animation <NAME>` | Background animation. |
| `--fill` / `--no-fill` | Clock on the whole screen or in the top half. |
| `--print [FORMAT]` | Prints one line using the markers and exits. |
| `--config <FILE>` | A specific configuration file. |
| `--init` | Writes the example configuration to the default location and exits. |
| `--check` | Checks the configuration and runs every widget once. |

Options for each mode:

| Mode | Options |
| --- | --- |
| `clock` | `--timezone <TIMEZONE>` |
| `timer` | `-d, --duration <DURATION>...`, `-r, --repeat`, `-Q, --auto-quit`, `-e, --execute <COMMAND>...` |
| `stopwatch` | — |
| `countdown` | `-t, --time <TARGET>`, `-C, --continue`, `--reverse` |

## Diagnostics and scripting

**`--check`** shows where the configuration came from, what the resolved
appearance is, and runs every widget once, printing what it returned. It is the
first place to look when a panel does not show up or shows up empty:

```powershell
my-dev-clock --check
```

```
config: C:\Users\example\AppData\Roaming\my-dev-clock\config.toml
initial theme: default
font: bricks · animation: none (12 fps) · fill: no · flip: no
format: (default) · date: (default) · extra timezones: none
custom theme: no · bell: no · laps as CSV: (no)

[1/7] cpu · CPU · 0.5s
██░░░░░░░░░░
21.6%
8 cores
```

**`--print`** prints one line and exits, without opening the alternate screen. It
is the mode for tmux, a prompt or any status bar:

```powershell
my-dev-clock --print "{time} · {git} · cpu {cpu} · ram {mem} · {host}"
```

```
14:09:15 · main * · cpu 15% · ram 68% · DESKTOP-8KQ2M4
```

| Marker | Becomes |
| --- | --- |
| `{time}` | The time, with the format from `[clock] format` |
| `{date}` | The date line, with the extra timezones |
| `{git}` | Branch and state of the current directory, like `main *` |
| `{cpu}` | Processor usage, as a percentage |
| `{mem}` | Memory usage, as a percentage |
| `{host}` | Machine name |

An unknown marker stays as it is, so a typo shows up instead of turning into an
empty line.

## Common problems

**The shortcuts seem not to work.** Press `?` to see the list and look at the
confirmation line in the footer: every appearance shortcut says what changed. If
it does not appear, the shortcut did not arrive — and the most common cause is the
size field being open (it shows up on the last line). `Esc` closes it.

**A widget does not show up.** It may be out of room: look for the
`+N panel(s) with no room` strip. Or the `kind` is wrong — `--check` shows the
exact error for each one. Widgets that do not fit do not run, so there is no
hidden cost.

**The app does not build: `linker link.exe not found`.** `cargo` could not find
the C++ toolset. Use `.\build.cmd`, which enters the Visual Studio environment
before calling cargo. If even that does not fix it, install the "Desktop
development with C++" workload in Visual Studio.

**`failed to remove ... my-dev-clock.exe`.** The binary is in use: close the app
and build again.

**The `flip` face looks the same as `bricks`.** Its cards are background-colored.
In a console that does not paint the background (the old `conhost`), it ends up
the same as `bricks` — the confirmation line will say `font: flip` while the
drawing does not change. In Windows Terminal it appears as expected.

**The weather does not load.** It depends on Open-Meteo, an outside service. When
the network fails, the widget says what happened — and the message tells "the city
is missing from the configuration" apart from "the request did not answer".

**A native Windows toast does not work.** The WinRT projection is not available in
this installation: both PowerShell 7 and 5.1 answer `Unable to find type
[Windows.UI.Notifications.ToastNotificationManager]`. For a sound alert use the
[`execute` recipes](#alert-when-time-runs-out); for a real toast, install a module
that does it (BurntToast, for example) and call it from `execute`.

**The live tests do not run.** They are marked `#[ignore]` because they use the
network, git and the `gh` CLI:

```powershell
.\build.cmd test -- --ignored
```

## How it works

- Each widget runs on its own thread and returns the text over a channel; the
  drawing loop never waits for a command.
- The animations use a brightness ramp and correct for the cell aspect ratio (a
  terminal cell is about twice as tall as it is wide). Without that correction,
  every round shape comes out stretched vertically.
- Animation time is injected and the randomness comes from a fixed seed, so a
  frame is reproducible — that is what makes animation testable.
- Widget commands run inside a Windows Job Object with `kill-on-close`: the
  process and everything it started die together.
- The GitHub widget makes a single GraphQL call with all three searches (review
  requests, your PRs, assigned issues). It used to be three sequential REST calls,
  and the difference shows on the clock: about 1.2 s against 4.4 s.
- Saving `config.toml` is noticed through the file timestamp. The appearance is
  reapplied right away; a running timer or stopwatch keeps counting.
- An invalid date or time format in the file is refused with a warning, instead of
  crashing the app while drawing.

## Development

```powershell
.\build.cmd test                                  # tests
.\build.cmd clippy --all-targets -- -D warnings    # lint, no warnings
.\build.cmd fmt --all --check                      # formatting
.\build.cmd test -- --ignored                      # tests that use network, git and gh
```

CI runs the same cycle on `windows-latest`, and a `v*` tag publishes the
executable, a package with the example configuration and the checksum to the
release; the tag has to match the version in `Cargo.toml`, or the workflow fails.
The rendering tests draw real frames on a test backend and check the content,
which is what lets us claim a font or an animation works without depending on
someone looking at it.

MIT license, in [LICENSE](LICENSE).

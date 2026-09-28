# omaday

Daily notes for Omarchy. One Markdown file per day, stored in your Obsidian
vault as `YYYY-MM-DD.md`, so Obsidian shows them as its daily notes.

Months in the header, years in the footer, the days of the month in between.
Click a day and a plain text editor unfolds under it. Colors follow the
active Omarchy theme, the font follows the Omarchy font.

## Build and install

Needs Rust and GTK 4. GTK 4 comes with Omarchy; if Rust is missing,
install it with `sudo pacman -S rust`.

```sh
make install
```

This puts the binary in `~/.local/bin`, a desktop entry in
`~/.local/share/applications` and an icon in `~/.local/share/icons`.

Optional Hyprland binding, in `~/.config/hypr/bindings.lua`:

```lua
o.bind("SUPER + SHIFT + D", "omaday", { launch = "omaday" })
```

## Configuration

Optional file `~/.config/omaday/config.toml`:

```toml
vault = "~/vault"        # folder holding YYYY-MM-DD.md files
font = "monospace"       # fontconfig family name
font_size = 12           # points
```

## Keys

| Keys | Action |
|---|---|
| Up, Down | Move between days |
| Enter | Open or close the day |
| Escape | Close the editor |
| Ctrl+Left, Ctrl+Right | Previous, next month |
| Ctrl+Down, Ctrl+Up | Previous, next year |
| Ctrl+T | Today |
| Ctrl+Q | Quit |

Inside the editor Ctrl+Left, Ctrl+Right, Ctrl+Up and Ctrl+Down move the
caret by word and paragraph as usual; press Escape first to use them for
months and years. Ctrl+T and Ctrl+Q work everywhere.

Notes save two seconds after you stop typing, and at once when you close
the day, switch month or year, leave the window or quit. An emptied note
deletes its file. If the same file changed outside while it was open, the
outside version is kept under `.omaday/conflicts/` in the vault. If a save
fails, a notice shows under the note and the day stays open: closing it,
switching month or year and quitting all wait for a good save, but a second
try to quit closes the window anyway.

Set `OMADAY_DEBUG=1` to see save and theme events on stderr.

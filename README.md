# `ninomiya`: a simple, beautiful notification daemon.

A notification daemon in the style of
[dunst](https://github.com/dunst-project/dunst) but with an emphasis on beauty.
Written in GTK, uses your GNOME colors if they're set, but can be used and
themed via CSS.

If you want something battle-tested with active development and features, use dunst. I used
it before I wrote this and it works perfectly well. If you want to support
people writing software in Rust, if you want true background transparency (dunst
only supports setting the entire window's opacity), use ninomiya.

## How to use

Build it using `cargo build`. Run the daemon using `ninomiya`; if you want
logging, run like `RUST_LOG=debug`. Valid log levels are `error`, `warn`,
`info`, `debug`, and `trace` (which *will* spam stdout).

You can also use it to *send* notifications by invoking it like

```
ninomiya notify --app-name "some app" --body "body" --summary "the summary"
```

If you run the daemon with `--testing`, it will listen on a separate DBus name;
you can then invoke `ninomiya --testing notify` to send to that. This is useful
for checking it out without messing with your actual notification setup, or for
debugging it when you're hacking on it.

## What's in a name?

It's named after [an anime character I
like](https://gatchaman.fandom.com/wiki/Rui_Ninomiya). Fans of Gatchaman Crowds
might point out that there's [a more appropriate
name](https://gatchaman.fandom.com/wiki/Berg_Katze_(Crowds)) for a daemon that
shows you NOTEs, but I like Rui.

//! This file implements the `notify` subcommand, which is used to send notifications.

use crate::dbus_codegen::client::OrgFreedesktopNotifications;
use crate::hints::{Hints, ImageRef};
use crate::server::Action;
use anyhow::{anyhow, ensure, Context, Result};
use clap::arg_enum;
use dbus::blocking::{Connection, Proxy};
use std::path::PathBuf;
use std::time::Duration;
use structopt::StructOpt;

arg_enum! {
#[derive(Debug)]
enum ImageAs {
    Path,
    Bytes,
}
}

fn parse_action(s: &str) -> Result<Action> {
    let v: Vec<&str> = s.splitn(2, ":").collect();
    ensure!(
        v.len() == 2,
        "action must have a colon to delimit key from label",
    );
    Ok(Action {
        key: v[0].into(),
        label: v[1].into(),
    })
}

#[derive(Debug, StructOpt)]
pub struct NotifyOpt {
    /// The application name the notification is from.
    #[structopt(short, long)]
    app_name: Option<String>,
    /// The name of the icon to display, or a path to it. Paths are interpreted as relative to
    /// the current directory, and should contain a '.' or a '/' to disambiguate from icon
    /// names.
    #[structopt(short = "c", long)]
    icon: Option<String>,
    /// The path to the image to display. Paths are interpreted as relative to the current directory.
    #[structopt(short = "m", long)]
    image: Option<String>,
    /// The summary of the notification.
    #[structopt(short, long)]
    summary: String,
    /// Valid actions to take. Each action separates the key from the label by a colon.
    #[structopt(long, parse(try_from_str = parse_action))]
    action: Vec<Action>,
    /// The body of the notification.
    #[structopt(short, long)]
    body: Option<String>,
    /// DEBUG: Whether to send the image as a path or as bytes.
    #[structopt(long, possible_values = &ImageAs::variants(), case_insensitive = true, default_value = "path", hidden_short_help = true)]
    image_as: ImageAs,
}
pub fn notify(dbus_name: &str, options: NotifyOpt) -> Result<()> {
    let c = Connection::new_session()?;
    let proxy = Proxy::new(
        dbus_name,
        "/org/freedesktop/Notifications",
        Duration::from_millis(1000),
        &c,
    );
    let hints = fill_hints(&options).context("can't populate hints dictionary")?;
    // Actions are passed by alternating the key and the label.
    let actions: Vec<&str> = options
        .action
        .iter()
        .map(|act| vec![act.key.as_str(), act.label.as_str()].into_iter())
        .flatten()
        .collect();

    proxy
        .notify(
            options.app_name.as_deref().unwrap_or(""),
            // replaces_id; it's mandatory for some reason, but most client libraries seem to set
            // it to 0 by default.
            0,
            &format_icon(&options.icon)
                .with_context(|| format!("loading icon from {:?}", options.icon))?,
            &options.summary,
            options.body.as_deref().unwrap_or(""),
            actions,
            hints.into_dbus(),
            -1, // expiration timeout
        )
        .context("failed to send notification")?;
    return Ok(());
}

fn format_icon(icon: &Option<String>) -> Result<String> {
    if let Some(icon) = icon {
        if icon.contains(".") || icon.contains("/") {
            let path = PathBuf::from(icon).canonicalize()?;
            let url = url::Url::from_file_path(&path)
                .map_err(|_| anyhow!("cannot convert path {:?} to URL", path))?;
            Ok(url.into_string())
        } else {
            Ok(icon.clone())
        }
    } else {
        Ok("".to_owned())
    }
}

fn fill_hints(options: &NotifyOpt) -> Result<Hints> {
    let mut hints = Hints::new();
    if let Some(image_path) = &options.image {
        match options.image_as {
            ImageAs::Path => hints.image = Some(image_path.parse()?),
            ImageAs::Bytes => {
                let pixbuf = gdk_pixbuf::Pixbuf::new_from_file(image_path)?;
                let bytes = unsafe { pixbuf.get_pixels().to_owned() };
                hints.image = Some(ImageRef::Image {
                    width: pixbuf.get_width(),
                    height: pixbuf.get_height(),
                    has_alpha: pixbuf.get_has_alpha(),
                    bits_per_sample: pixbuf.get_bits_per_sample(),
                    image_data: bytes,
                });
            }
        }
    }
    Ok(hints)
}

//! This file implements the `notify` subcommand, which is used to send notifications.

use crate::dbus_codegen::client::OrgFreedesktopNotifications;
use anyhow::{anyhow, Context, Result};
use dbus::blocking::{Connection, Proxy};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct NotifyOpt {
    /// The application name the notification is from.
    #[structopt(short, long)]
    app_name: Option<String>,
    /// The name of the icon to display, or a path to it. Paths are interpreted as relative to
    /// the current directory, and should contain a '.' or a '/' to disambiguate from icon
    /// names.
    #[structopt(short, long)]
    icon: Option<String>,
    /// The summary of the notification.
    #[structopt(short, long)]
    summary: String,
    /// The body of the notification.
    #[structopt(short, long)]
    body: Option<String>,
}
pub fn notify(dbus_name: &str, options: NotifyOpt) -> Result<()> {
    let c = Connection::new_session()?;
    let proxy = Proxy::new(
        dbus_name,
        "/org/freedesktop/Notifications",
        Duration::from_millis(1000),
        &c,
    );
    proxy.notify(
        options.app_name.as_deref().unwrap_or(""),
        // replaces_id; it's mandatory for some reason, but most client libraries seem to set
        // it to 0 by default.
        0,
        &format_icon(&options.icon)
            .with_context(|| format!("loading icon from {:?}", options.icon))?,
        &options.summary,
        options.body.as_deref().unwrap_or(""),
        vec![],         // actions
        HashMap::new(), // hints
        -1,             // expiration timeout
    )?;
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

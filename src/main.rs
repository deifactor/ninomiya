mod config;
mod dbus_codegen;
mod gui;
mod server;

use crate::config::Config;
use anyhow::{anyhow, Context, Result};
use dbus::blocking::{Connection, LocalConnection, Proxy};
use dbus_codegen::client::OrgFreedesktopNotifications;
use log::{error, info, trace, warn};
use std::collections::HashMap;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use structopt::StructOpt;

static DBUS_NAME: &str = "org.freedesktop.Notifications";
static DBUS_TESTING_NAME: &str = "org.freedesktop.NotificationsNinomiyaTesting";

#[derive(Debug, StructOpt)]
#[structopt(name = "example", about = "A beautiful notification daemon.")]
struct Opt {
    /// If true, uses a separate DBus name. This is mostly useful for development purposes.
    #[structopt(short, long)]
    testing: bool,

    #[structopt(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, StructOpt)]
enum Command {
    Notify {
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
    },
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::builder().format_module_path(true).init();
    let opt = Opt::from_args();
    let dbus_name = if opt.testing {
        DBUS_TESTING_NAME
    } else {
        DBUS_NAME
    };

    if let Some(Command::Notify {
        app_name,
        icon,
        summary,
        body,
    }) = opt.command
    {
        let c = Connection::new_session()?;
        let proxy = Proxy::new(
            dbus_name,
            "/org/freedesktop/Notifications",
            Duration::from_millis(1000),
            &c,
        );
        proxy.notify(
            app_name.as_deref().unwrap_or(""),
            // replaces_id; it's mandatory for some reason, but most client libraries seem to set
            // it to 0 by default.
            0,
            &format_icon(&icon).with_context(|| format!("loading icon from {:?}", icon))?,
            &summary,
            body.as_deref().unwrap_or(""),
            vec![],         // actions
            HashMap::new(), // hints
            -1,             // expiration timeout
        )?;
        return Ok(());
    }

    info!("Starting up.");
    let config = Config::load().unwrap_or_else(|err| {
        warn!("Failed to load config ({:?}); falling back to default", err);
        Config::default()
    });

    let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
    let gui = gui::Gui::new(config, tx.clone());
    let css = gui::load_css().context("failed to load CSS")?;
    gtk::StyleContext::add_provider_for_screen(
        &gdk::Screen::get_default().context("Error initializing gtk css provider.")?,
        &css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    // Start off the server thread, which will grab incoming messages from DBus and send them onto
    // the channel.
    thread::spawn(move || {
        info!("Hello from the server thread.");
        let mut c = LocalConnection::new_session().expect("couldn't connect to dbus");
        let request_reply = c
            .request_name(
                dbus_name, /* allow_replacement */ true, /* replace_existing */ true,
                /* do_not_queue */ true,
            )
            .expect("requesting the name failed");
        if request_reply
            != dbus::blocking::stdintf::org_freedesktop_dbus::RequestNameReply::PrimaryOwner
        {
            error!(
                "Failed to get the name we wanted (reason: {:?}); dying.",
                request_reply
            );
            // TODO: Die nicer here.
            std::process::exit(1);
        }
        let server =
            server::NotifyServer::new(move |event| tx.send(event).expect("failed to send"));
        let tree = server::create_tree(server);
        tree.start_receive(&c);
        loop {
            c.process(std::time::Duration::from_millis(1000))
                .expect("death while processing messages");
            trace!("Another turn around the loop.");
        }
    });
    // XXX: We should call with the command-line options here, but GTK wants to do its own argument
    // parsing, and that's annoying.
    match gui.run(rx, &[]) {
        0 => Ok(()),
        _ => Err("error when running application".into()),
    }
}

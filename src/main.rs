mod client;
mod config;
mod dbus_codegen;
mod demo;
mod gui;
mod hints;
mod image;
mod server;

#[cfg(test)]
mod gtk_test_runner;

use crate::config::Config;
use anyhow::{anyhow, Context, Result};
use dbus::blocking::LocalConnection;
use log::{info, warn};
use std::sync::mpsc;
use std::thread;
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
    Notify(client::NotifyOpt),
    Demo,
}

fn main() -> Result<()> {
    env_logger::builder().format_module_path(true).init();
    let opt = Opt::from_args();
    let dbus_name = if opt.testing {
        DBUS_TESTING_NAME
    } else {
        DBUS_NAME
    };

    if let Some(Command::Notify(notify_opt)) = opt.command {
        return client::notify(dbus_name, notify_opt);
    }

    info!("Starting up.");
    let config = Config::load().unwrap_or_else(|err| {
        warn!("Failed to load config ({:?}); falling back to default", err);
        Config::default()
    });

    let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
    let (signal_tx, signal_rx) = mpsc::channel();
    let theme_path = config.full_theme_path()?;
    let gui = gui::Gui::new(config, tx.clone(), signal_tx);
    gui::add_css("data/style.css")?;
    if theme_path.exists() {
        gui::add_css(theme_path)?;
    } else {
        warn!("Theme path {:?} doesn't exist, not loading it", theme_path);
    }

    if let Some(Command::Demo) = opt.command {
        demo::send_notifications(tx.clone()).context("failed sending demo notifications")?;
        thread::spawn(move || -> Result<()> {
            loop {
                // Don't put this inside the info! macro, otherwise if we're not actually logging
                // then we'll never try to read from the signal queue, resulting in this being an
                // infinite loop.
                let gui_signal = signal_rx.recv()?;
                info!("Received signal from GUI: {:?}", gui_signal);
            }
        });
    } else {
        // Start off the server thread, which will grab incoming messages from DBus and send them onto
        // the channel.
        thread::spawn(move || {
            info!("Hello from the server thread.");
            let server =
                server::NotifyServer::new(move |event| tx.send(event).expect("failed to send"));
            let connection = LocalConnection::new_session().expect("couldn't connect to dbus");
            server
                .run(dbus_name, connection, signal_rx)
                .expect("Server died unexpectedly");
        });
    }

    // XXX: We should call with the command-line options here, but GTK wants to do its own argument
    // parsing, and that's annoying.
    match gui.run(rx, &[]) {
        0 => Ok(()),
        _ => Err(anyhow!("error when running application")),
    }
}

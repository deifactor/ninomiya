mod client;
mod config;
mod dbus_codegen;
mod gui;
mod hints;
mod image;
mod server;

#[cfg(test)]
mod gtk_test_runner;

use crate::config::Config;
use crate::server::NinomiyaEvent;
use anyhow::{anyhow, Result};
use dbus::blocking::LocalConnection;
use log::{error, info, trace, warn};
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
    let theme_path = config.full_theme_path()?;
    let gui = gui::Gui::new(config, tx.clone());
    gui::add_css("data/style.css")?;
    if theme_path.exists() {
        gui::add_css(theme_path)?;
    } else {
        warn!("Theme path {:?} doesn't exist, not loading it", theme_path);
    }

    // Start off the server thread, which will grab incoming messages from DBus and send them onto
    // the channel.
    thread::spawn(move || server_thread(dbus_name, tx));
    // XXX: We should call with the command-line options here, but GTK wants to do its own argument
    // parsing, and that's annoying.
    match gui.run(rx, &[]) {
        0 => Ok(()),
        _ => Err(anyhow!("error when running application")),
    }
}

fn server_thread(dbus_name: &str, tx: glib::Sender<NinomiyaEvent>) {
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
    let server = server::NotifyServer::new(move |event| tx.send(event).expect("failed to send"));
    let tree = server::create_tree(server);
    tree.start_receive(&c);
    loop {
        c.process(std::time::Duration::from_millis(1000))
            .expect("death while processing messages");
        trace!("Another turn around the loop.");
    }
}

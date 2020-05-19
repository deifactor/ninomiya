use crate::dbus_codegen::server as dbus_server;
use crate::hints::{Hints, ImageRef};
use anyhow::{bail, Context, Result};
use dbus::blocking::stdintf::org_freedesktop_dbus::RequestNameReply;
use dbus::blocking::LocalConnection;
use dbus::channel::Sender;
use dbus::message::SignalArgs;
use dbus::{self, arg, tree};
use log::{debug, error, info, trace};
use std::cell::Cell;
use std::collections::HashMap;
use std::fmt;
use std::sync::mpsc::{Receiver, TryRecvError};

/// Indicates that the notification has some action that the user can take.
#[derive(Debug)]
pub struct Action {
    /// An internal ID, to be used when sending the signal back to the originating application.
    pub key: String,
    /// The localized string to be displayed to the user.
    pub label: String,
}

#[derive(Debug)]
pub struct Notification {
    /// An arbitrary ID number. Generated by `ninomiya`, only used internally.
    pub id: u32,
    /// Actions that the user can take in response to the notification.
    pub actions: Vec<Action>,
    /// An application icon, if any was specified. This should be loaded using [load_icon], but we
    /// defer that to the GUI thread because Pixbuf isn't thread-safe.
    pub icon: Option<ImageRef>,
    /// Human-readable name of the application. Can be blank.
    pub application_name: Option<String>,
    /// A brief summary of the notification.
    pub summary: String,
    /// The notification body.
    pub body: Option<String>,
    pub hints: Hints,
}

#[derive(Debug)]
pub enum NinomiyaEvent {
    /// A notification to be displayed.
    Notification(Notification),
    /// The given notification should be closed.
    CloseNotification(u32),
}

/// Represents all the signals that we can emit, according to the DBus notification specification.
#[derive(Debug)]
pub enum Signal {
    /// The user invoked an action on the notification.
    ActionInvoked { id: u32, key: String },
}

fn owned_if_nonempty(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_owned())
    }
}

/// Handles the state of the notification server. This doesn't deal with talking with DBus or
/// anything.
pub struct NotifyServer {
    /// The ID of the next notification to be returned. This isn't global state, so you should only
    /// have one NotificationServer at a time.
    next_id: Cell<u32>,
    callback: Box<dyn Fn(NinomiyaEvent) -> ()>,
}

impl fmt::Debug for NotifyServer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "NotifyServer {{ {:?} }}", self.next_id)
    }
}

impl NotifyServer {
    pub fn new<F: Fn(NinomiyaEvent) -> () + 'static>(callback: F) -> Self {
        NotifyServer {
            // A lot of client libraries seem to use 0 as the fallback ID for sent notifications,
            // so we shouldn't use 0 as the default.
            next_id: Cell::new(1),
            callback: Box::new(callback),
        }
    }

    /// Runs the notification server forever.
    ///
    /// The server return if it fails to acquire the given name or if the connectoin closes. Under
    /// normal behavior, this function never returns. So you can think of it as having type
    /// `Result<!>`, when that gets stabilized.
    pub fn run(
        self,
        dbus_name: &str,
        mut connection: LocalConnection,
        signal_rx: Receiver<Signal>,
    ) -> Result<()> {
        let request_reply = connection
            .request_name(
                dbus_name, /* allow_replacement */ true, /* replace_existing */ true,
                /* do_not_queue */ true,
            )
            .context("requesting the name failed")?;
        if request_reply != RequestNameReply::PrimaryOwner {
            bail!("Failed to get the name we wanted (reason: {:?}), request_reply");
        }
        let tree = create_tree(self);
        tree.start_receive(&connection);
        loop {
            connection.process(std::time::Duration::from_millis(50))?;
            handle_signal_events(&connection, &signal_rx)?;
            trace!("Another turn around the loop.");
        }
    }

    fn new_id(&self) -> u32 {
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        id
    }
}

/// Drains the receiver of signals that are queued to be sent, then sends them over the connection.
fn handle_signal_events(connection: &LocalConnection, signal_rx: &Receiver<Signal>) -> Result<()> {
    let path = dbus::strings::Path::new("/org/freedesktop/Notifications")
        .expect("failed to parse dbus path name; this is really weird!");
    loop {
        match signal_rx.try_recv() {
            Ok(Signal::ActionInvoked { id, key }) => {
                debug!("Sending signal: {} invoked on {}", key, id);
                let sig = dbus_server::OrgFreedesktopNotificationsActionInvoked {
                    id,
                    action_key: key,
                };
                if connection.send(sig.to_emit_message(&path)).is_err() {
                    error!("Failed to send signal over dbus");
                }
            }
            Err(TryRecvError::Empty) => return Ok(()),
            Err(TryRecvError::Disconnected) => bail!("GUI closed its signal tx"),
        }
    }
}

impl dbus_server::OrgFreedesktopNotifications for NotifyServer {
    fn get_capabilities(&self) -> Result<Vec<String>, tree::MethodErr> {
        Ok(vec!["body", "actions", "body-markup"]
            .into_iter()
            .map(|s| s.to_string())
            .collect())
    }

    fn notify(
        &self,
        app_name: &str,
        _replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: Vec<&str>,
        hints: HashMap<&str, arg::Variant<Box<dyn arg::RefArg>>>,
        _expire_timeout: i32,
    ) -> Result<u32, tree::MethodErr> {
        let icon: Option<ImageRef> = if app_icon.is_empty() {
            None
        } else {
            Some(
                app_icon
                    .parse()
                    .map_err(|err| tree::MethodErr::failed(&err))?,
            )
        };

        if actions.len() % 2 != 0 {
            return Err(tree::MethodErr::failed(&format!(
                "Action length {} must be a multiple of 2",
                actions.len()
            )));
        }
        let actions = actions
            .chunks_exact(2)
            .map(|c| Action {
                key: c[0].to_owned(),
                label: c[1].to_owned(),
            })
            .collect::<Vec<_>>();

        let id = self.new_id();
        let hints = Hints::from_dbus(hints);
        if let Err(err) = &hints {
            error!("Failed to build hints dict: {:?}", err);
        }
        let notification = Notification {
            id,
            icon,
            actions,
            application_name: owned_if_nonempty(app_name),
            summary: summary.to_owned(),
            body: owned_if_nonempty(body),
            hints: hints.map_err(|err| tree::MethodErr::failed(&err))?,
        };
        info!("Got notification {}", notification.id);
        (self.callback)(NinomiyaEvent::Notification(notification));
        Ok(id)
    }

    fn close_notification(&self, id: u32) -> Result<(), tree::MethodErr> {
        (self.callback)(NinomiyaEvent::CloseNotification(id));
        Ok(())
    }

    fn get_server_information(&self) -> Result<(String, String, String, String), tree::MethodErr> {
        // name, vendor, version, spec_version
        Ok((
            "ninomiya".to_owned(),
            "deifactor".to_owned(),
            env!("CARGO_PKG_VERSION").to_owned(),
            "1.2".to_owned(),
        ))
    }
}

#[derive(Copy, Clone, Default, Debug)]
pub struct TData;
impl tree::DataType for TData {
    type Tree = NotifyServer;
    type ObjectPath = ();
    type Property = ();
    type Interface = ();
    type Method = ();
    type Signal = ();
}

pub fn create_tree(server: NotifyServer) -> tree::Tree<tree::MTFn<TData>, TData> {
    let f = tree::Factory::new_fn();
    let iface =
        dbus_server::org_freedesktop_notifications_server(&f, (), move |_m| _m.tree.get_data());
    let mut tree = f.tree(server);
    tree = tree.add(
        f.object_path("/org/freedesktop/Notifications", ())
            .introspectable()
            .add(iface),
    );
    tree
}

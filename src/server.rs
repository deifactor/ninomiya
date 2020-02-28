use crate::notify_codegen;
use dbus::{self, arg, tree};
use std::cell::Cell;
use std::collections::HashMap;
use std::fmt;

#[derive(Clone, Debug)]
pub struct Notification {
    /// An arbitrary ID number. Generated by `ninomiya`, only used internally.
    id: u32,
    /// Human-readable name of the application. Can be blank.
    pub application_name: Option<String>,
    /// A brief summary of the notification.
    pub summary: String,
    /// The notification body.
    pub body: Option<String>,
}

#[derive(Clone, Debug)]
pub enum NinomiyaEvent {
    Notification(Notification),
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
            next_id: Cell::new(0),
            callback: Box::new(callback),
        }
    }

    fn new_id(&self) -> u32 {
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        id
    }
}

impl notify_codegen::OrgFreedesktopNotifications for NotifyServer {
    fn get_capabilities(&self) -> Result<Vec<String>, tree::MethodErr> {
        Ok(vec!["body".to_owned()])
    }

    fn notify(
        &self,
        app_name: &str,
        _replaces_id: u32,
        _app_icon: &str,
        summary: &str,
        body: &str,
        _actions: Vec<&str>,
        _hints: HashMap<&str, arg::Variant<Box<dyn arg::RefArg>>>,
        _expire_timeout: i32,
    ) -> Result<u32, tree::MethodErr> {
        let id = self.new_id();
        let notification = Notification {
            id,
            application_name: owned_if_nonempty(app_name),
            summary: summary.to_owned(),
            body: owned_if_nonempty(body),
        };
        (self.callback)(NinomiyaEvent::Notification(notification));
        Ok(id)
    }

    fn close_notification(&self, _id: u32) -> Result<(), tree::MethodErr> {
        Err(tree::MethodErr::failed("not implemented"))
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
        notify_codegen::org_freedesktop_notifications_server(&f, (), move |_m| _m.tree.get_data());
    let mut tree = f.tree(server);
    tree = tree.add(
        f.object_path("/org/freedesktop/Notifications", ())
            .introspectable()
            .add(iface),
    );
    tree
}

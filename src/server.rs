use crate::notify_codegen;
use dbus::{self, arg, tree};
use std::collections::HashMap;

#[derive(Copy, Clone, Debug)]
pub struct NotifyServer;

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
        actions: Vec<&str>,
        _hints: HashMap<&str, arg::Variant<Box<dyn arg::RefArg>>>,
        _expire_timeout: i32,
    ) -> Result<u32, tree::MethodErr> {
        println!("{:?} {:?} {:?} {:?}", app_name, summary, body, actions);
        Ok(0)
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
    type Tree = ();
    type ObjectPath = ();
    type Property = ();
    type Interface = ();
    type Method = ();
    type Signal = ();
}

pub fn create_tree(server: &'static NotifyServer) -> tree::Tree<tree::MTFn<TData>, TData> {
    let f = tree::Factory::new_fn();
    let iface = notify_codegen::org_freedesktop_notifications_server(&f, (), move |_m| server);
    let mut tree = f.tree(());
    tree = tree.add(
        f.object_path("/org/freedesktop/Notifications", ())
            .introspectable()
            .add(iface),
    );
    tree
}

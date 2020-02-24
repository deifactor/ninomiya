// This code was autogenerated with `dbus-codegen-rust -d org.freedesktop.Notifications -p /org/freedesktop/Notifications`, see https://github.com/diwic/dbus-rs
use dbus;
use dbus::arg;
use dbus::tree;

pub trait OrgFreedesktopNotifications {
    fn get_capabilities(&self) -> Result<Vec<String>, tree::MethodErr>;
    fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: Vec<&str>,
        hints: ::std::collections::HashMap<&str, arg::Variant<Box<dyn arg::RefArg>>>,
        expire_timeout: i32,
    ) -> Result<u32, tree::MethodErr>;
    fn close_notification(&self, id: u32) -> Result<(), tree::MethodErr>;
    fn get_server_information(&self) -> Result<(String, String, String, String), tree::MethodErr>;
}

pub fn org_freedesktop_notifications_server<F, T, D>(
    factory: &tree::Factory<tree::MTFn<D>, D>,
    data: D::Interface,
    f: F,
) -> tree::Interface<tree::MTFn<D>, D>
where
    D: tree::DataType,
    D::Method: Default,
    D::Signal: Default,
    T: OrgFreedesktopNotifications,
    F: 'static + for<'z> Fn(&'z tree::MethodInfo<tree::MTFn<D>, D>) -> &'z T,
{
    let i = factory.interface("org.freedesktop.Notifications", data);
    let f = ::std::sync::Arc::new(f);
    let fclone = f.clone();
    let h = move |minfo: &tree::MethodInfo<tree::MTFn<D>, D>| {
        let d = fclone(minfo);
        let capabilities = d.get_capabilities()?;
        let rm = minfo.msg.method_return();
        let rm = rm.append1(capabilities);
        Ok(vec![rm])
    };
    let m = factory.method("GetCapabilities", Default::default(), h);
    let m = m.out_arg(("capabilities", "as"));
    let i = i.add_m(m);

    let fclone = f.clone();
    let h = move |minfo: &tree::MethodInfo<tree::MTFn<D>, D>| {
        let mut i = minfo.msg.iter_init();
        let app_name: &str = i.read()?;
        let replaces_id: u32 = i.read()?;
        let app_icon: &str = i.read()?;
        let summary: &str = i.read()?;
        let body: &str = i.read()?;
        let actions: Vec<&str> = i.read()?;
        let hints: ::std::collections::HashMap<&str, arg::Variant<Box<dyn arg::RefArg>>> =
            i.read()?;
        let expire_timeout: i32 = i.read()?;
        let d = fclone(minfo);
        let id = d.notify(
            app_name,
            replaces_id,
            app_icon,
            summary,
            body,
            actions,
            hints,
            expire_timeout,
        )?;
        let rm = minfo.msg.method_return();
        let rm = rm.append1(id);
        Ok(vec![rm])
    };
    let m = factory.method("Notify", Default::default(), h);
    let m = m.in_arg(("app_name", "s"));
    let m = m.in_arg(("replaces_id", "u"));
    let m = m.in_arg(("app_icon", "s"));
    let m = m.in_arg(("summary", "s"));
    let m = m.in_arg(("body", "s"));
    let m = m.in_arg(("actions", "as"));
    let m = m.in_arg(("hints", "a{sv}"));
    let m = m.in_arg(("expire_timeout", "i"));
    let m = m.out_arg(("id", "u"));
    let i = i.add_m(m);

    let fclone = f.clone();
    let h = move |minfo: &tree::MethodInfo<tree::MTFn<D>, D>| {
        let mut i = minfo.msg.iter_init();
        let id: u32 = i.read()?;
        let d = fclone(minfo);
        d.close_notification(id)?;
        let rm = minfo.msg.method_return();
        Ok(vec![rm])
    };
    let m = factory.method("CloseNotification", Default::default(), h);
    let m = m.in_arg(("id", "u"));
    let i = i.add_m(m);

    let fclone = f.clone();
    let h = move |minfo: &tree::MethodInfo<tree::MTFn<D>, D>| {
        let d = fclone(minfo);
        let (name, vendor, version, spec_version) = d.get_server_information()?;
        let rm = minfo.msg.method_return();
        let rm = rm.append1(name);
        let rm = rm.append1(vendor);
        let rm = rm.append1(version);
        let rm = rm.append1(spec_version);
        Ok(vec![rm])
    };
    let m = factory.method("GetServerInformation", Default::default(), h);
    let m = m.out_arg(("name", "s"));
    let m = m.out_arg(("vendor", "s"));
    let m = m.out_arg(("version", "s"));
    let m = m.out_arg(("spec_version", "s"));
    let i = i.add_m(m);
    let s = factory.signal("NotificationClosed", Default::default());
    let s = s.arg(("id", "u"));
    let s = s.arg(("reason", "u"));
    let i = i.add_s(s);
    let s = factory.signal("ActionInvoked", Default::default());
    let s = s.arg(("id", "u"));
    let s = s.arg(("action_key", "s"));
    let i = i.add_s(s);
    i
}

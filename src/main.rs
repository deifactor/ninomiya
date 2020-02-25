mod gui;
mod notify_codegen;
mod server;

use dbus::blocking::LocalConnection;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut c = LocalConnection::new_session()?;
    c.request_name("org.freedesktop.Notifications", false, false, true)?;
    let tree = server::create_tree(server::NotifyServer::new());
    tree.start_receive(&c);
    loop {
        c.process(std::time::Duration::from_millis(1000))?;
    }
}

mod gui;
mod notify_codegen;
mod server;

use dbus::blocking::LocalConnection;
use std::{env, thread};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let gui = gui::Gui::new(gui::Config {
        width: 300,
        height: 100,
    });
    let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
    // Start off the server thread, which will grab incoming messages from DBus and send them onto
    // the channel.
    thread::spawn(|| {
        let mut c = LocalConnection::new_session().expect("couldn't connect to dbus");
        c.request_name("org.freedesktop.Notifications", false, false, true)
            .expect("couldn't grab name");
        let server =
            server::NotifyServer::new(move |event| tx.send(event).expect("failed to send"));
        let tree = server::create_tree(server);
        tree.start_receive(&c);
        loop {
            c.process(std::time::Duration::from_millis(1000))
                .expect("death while processing messages");
        }
    });
    match gui.run(rx, &env::args().collect::<Vec<_>>()) {
        0 => Ok(()),
        _ => Err("error when running application".into()),
    }
}

mod gui;
mod notify_codegen;
mod server;

use dbus::blocking::LocalConnection;
use glium::glutin::event_loop::EventLoop;
use std::sync::mpsc;
use std::thread;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (tx, rx) = mpsc::channel();
    let event_loop = EventLoop::with_user_event();

    // Start off the server thread, which will grab incoming messages from DBus and send them onto
    // the channel.
    thread::spawn(|| {
        let mut c = LocalConnection::new_session().expect("couldn't connect to dbus");
        c.request_name("org.freedesktop.Notifications", false, false, true)
            .expect("couldn't grab name");
        let tree = server::create_tree(server::NotifyServer::new(tx));
        tree.start_receive(&c);
        loop {
            c.process(std::time::Duration::from_millis(1000))
                .expect("death while processing messages");
        }
    });

    // Another thread, to take those messages off the channel and pass them off to the event loop.
    let proxy = event_loop.create_proxy();
    thread::spawn(move || {
        for notification in rx {
            proxy.send_event(gui::NinomiyaEvent::Notification(notification));
        }
    });

    // The main thread just runs the event loop forever.
    let manager = gui::WindowManager::new(gui::Config {
        width: 300.0,
        height: 100.0,
    });
    manager.run(event_loop);
    Ok(())
}

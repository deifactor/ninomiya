use crate::server::{NinomiyaEvent, Notification};
use gio::prelude::*;
use glib::{clone, object::WeakRef};
use gtk::prelude::*;
use log::{debug, error};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Mutex;
use std::time::Duration;

/// Configures how the GUI is rendered.
#[derive(Debug)]
pub struct Config {
    /// Width of notification windows.
    pub width: i32,
    /// Height of notification windows.
    pub height: i32,
    /// Amount of seconds to show windows before closing them.
    pub duration: Duration,
}

pub struct Gui {
    app: gtk::Application,
    config: Config,
    /// Used to send notifications on a delay.
    tx: glib::Sender<NinomiyaEvent>,
    windows: Mutex<HashMap<u32, WeakRef<gtk::ApplicationWindow>>>,
}

impl Gui {
    pub fn new(config: Config, tx: glib::Sender<NinomiyaEvent>) -> Rc<Self> {
        let app = gtk::Application::new(
            Some("deifactor.ninomiya"),
            gio::ApplicationFlags::FLAGS_NONE,
        )
        .expect("failed to construct application");
        debug!("Application constructed.");
        Rc::new(Gui {
            app,
            config,
            tx,
            windows: Mutex::new(HashMap::new()),
        })
    }

    pub fn run(self: std::rc::Rc<Self>, rx: glib::Receiver<NinomiyaEvent>, argv: &[String]) -> i32 {
        let this = self.clone();
        rx.attach(
            None,
            clone!(@weak this => @default-return glib::Continue(false),
            move |event| {
                debug!("Got event {:?}", event);
                match event {
                    NinomiyaEvent::Notification(notification) =>
                        this.notification_window(notification),
                    NinomiyaEvent::CloseNotification(id) =>
                        this.close_notification(id)
                }
                glib::Continue(true)
            }),
        );
        // Not actually necessary, but shuts up GTK.
        self.app.connect_activate(|_app| {
            debug!("Activated.");
        });
        self.app.hold();
        self.app.run(argv)
    }

    fn notification_window(&self, notification: Notification) {
        let window = gtk::ApplicationWindowBuilder::new()
            .show_menubar(false)
            .accept_focus(false)
            .application(&self.app)
            .decorated(false)
            .default_width(self.config.width)
            .default_height(self.config.height)
            .deletable(false)
            .focus_visible(false)
            .focus_on_map(false)
            .resizable(false)
            .can_focus(false)
            .build();
        let screen = gdk::Screen::get_default().expect("couldn't get screen");

        window.move_(screen.get_width() - self.config.width, 0);

        let boxx = gtk::Box::new(gtk::Orientation::Vertical, 0);
        if let Some(name) = &notification.application_name {
            boxx.add(&gtk::Label::new(Some(name)));
        }
        boxx.add(&gtk::Label::new(Some(&notification.summary)));
        if let Some(body) = &notification.body {
            boxx.add(&gtk::Label::new(Some(body)));
        }

        window.add(&boxx);
        window.show_all();

        let id = notification.id;
        let mut windows = self.windows.lock().unwrap();
        if windows.insert(id, window.downgrade()).is_some() {
            error!("Got duplicate notifications for id {}", id);
        }
        // Register a timeout to close this window in the future.
        glib::timeout_add(
            self.config.duration.as_millis() as u32,
            clone!(@strong self.tx as tx => move || {
                if let Err(err) = tx.send(NinomiyaEvent::CloseNotification(id)) {
                    error!("Failed to send close notification for {}: {:?}", id, err);
                }
                Continue(false)
            }),
        );
    }

    fn close_notification(&self, id: u32) {
        let mut windows = self.windows.lock().unwrap();
        if let Some(window) = windows.remove(&id).and_then(|weak| weak.upgrade()) {
            window.close();
        } else {
            error!("Couldn't grab window for notification {}", id);
        }
    }
}

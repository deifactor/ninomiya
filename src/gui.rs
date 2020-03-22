use crate::config::Config;
use crate::server::{NinomiyaEvent, Notification};
use anyhow::Result;
use gio::prelude::*;
use glib::{clone, object::WeakRef};
use gtk::prelude::*;
use log::{debug, error, info};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Mutex;

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
        let screen = gdk::Screen::get_default().expect("couldn't get screen");
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
        // Necessary to get transparent backgrounds working.
        let visual = screen.get_rgba_visual();
        window.set_visual(visual.as_ref());

        window.move_(screen.get_width() - self.config.width, self.next_y());

        let boxx = gtk::Box::new(gtk::Orientation::Vertical, 0);
        boxx.add(
            &gtk::LabelBuilder::new()
                .label(&notification.summary)
                .name("summary")
                .halign(gtk::Align::Start)
                .build(),
        );
        if let Some(body) = &notification.body {
            boxx.add(
                &gtk::LabelBuilder::new()
                    .label(body)
                    .name("body")
                    .halign(gtk::Align::Start)
                    .build(),
            );
        }
        if let Some(name) = &notification.application_name {
            boxx.add(
                &gtk::LabelBuilder::new()
                    .label(name)
                    .name("application-name")
                    .halign(gtk::Align::Start)
                    .build(),
            );
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
                info!("Automatically closing window for notification {}", id);
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

    /// Returns the y-coordinate of the lowest window.
    fn next_y(&self) -> i32 {
        self.windows
            .lock()
            .unwrap()
            .values()
            .filter_map(|weak| weak.upgrade())
            .map(|win| win.get_size().1 + win.get_position().1)
            .max()
            .map_or(0, |bottom| bottom + self.config.notification_spacing)
    }
}

pub fn load_css() -> Result<gtk::CssProvider, anyhow::Error> {
    let path = PathBuf::from("data/style.css");
    // we don't use ? here because if the path doesn't exist canonicalize() returns an Err
    info!("Attempting to load CSS from {:?}", path.canonicalize());
    let provider = gtk::CssProvider::new();
    provider.load_from_file(&gio::File::new_for_path(path))?;
    Ok(provider)
}

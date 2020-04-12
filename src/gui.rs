use crate::config::Config;
use crate::server::{NinomiyaEvent, Notification};
use anyhow::{anyhow, Context, Result};
use gio::prelude::*;
use glib::{clone, object::WeakRef};
use gtk::prelude::*;
use log::{debug, error, info};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Mutex;
use url::Url;

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
            .accept_focus(false)
            .application(&self.app)
            .default_width(self.config.width)
            .default_height(self.config.height)
            // Automatically sets up override redirect, so the window manager won't touch our
            // windows at all.
            .type_(gtk::WindowType::Popup)
            .type_hint(gdk::WindowTypeHint::Notification)
            .build();
        // Necessary to get transparent backgrounds working.
        let visual = screen.get_rgba_visual();
        window.set_visual(visual.as_ref());

        window.move_(screen.get_width() - self.config.width, self.next_y());

        let image: Option<gtk::Image> = notification.icon.and_then(|icon| {
            let image = load_image(&icon, 100, self.config.height);
            if let Err(ref err) = image {
                info!("Failed to load icon from {}: {}", icon, err);
            }
            image.ok()
        });

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

        let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        hbox.add(&boxx);
        if let Some(image) = image {
            hbox.pack_end(&image, false, false, 0);
        }

        window.add(&hbox);
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

pub fn add_css<P: AsRef<Path>>(path: P) -> Result<(), anyhow::Error> {
    // we don't use ? here because if the path doesn't exist canonicalize() returns an Err
    info!(
        "Attempting to load CSS from {:?}",
        &path.as_ref().canonicalize()
    );
    let provider = gtk::CssProvider::new();
    provider
        .load_from_file(&gio::File::new_for_path(path))
        .context("failed to load CSS")?;
    gtk::StyleContext::add_provider_for_screen(
        &gdk::Screen::get_default().context("Error initializing gtk css provider.")?,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    Ok(())
}

/// Loads an image from the given string. The string should either be a freedesktop.org-compliant
/// icon name (not yet supported) or a file:// URI.
///
/// The max_width and max_height parameters will be used to upper bound the size of the image. The
/// resizing will always be proportional.
fn load_image(source: &str, max_width: i32, max_height: i32) -> Result<gtk::Image> {
    if !source.contains("://") {
        return Err(anyhow!("icons not supported yet"));
    }
    let url = Url::parse(source)?;
    if url.scheme() != "file" {
        return Err(anyhow!("image URL {} must be file", source));
    }
    let pixbuf = gdk_pixbuf::Pixbuf::new_from_file_at_size(url.path(), max_width, max_height)?;

    Ok(gtk::Image::new_from_pixbuf(Some(&pixbuf)))
}

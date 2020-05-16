use crate::config::Config;
use crate::hints::ImageRef;
use crate::image;
use crate::server::{NinomiyaEvent, Notification};
use anyhow::{anyhow, Context, Result};
use gdk_pixbuf::Pixbuf;
use gio::prelude::*;
use glib::{clone, object::WeakRef};
use gtk::prelude::*;
use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::sync::Mutex;
use url::Url;

pub struct Gui {
    app: gtk::Application,
    loader: image::Loader,
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
        let loader = image::Loader::new();
        debug!("Application constructed.");
        Rc::new(Gui {
            app,
            loader,
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
            .width_request(self.config.width)
            .height_request(self.config.height)
            // Automatically sets up override redirect, so the window manager won't touch our
            // windows at all.
            .type_(gtk::WindowType::Popup)
            .type_hint(gdk::WindowTypeHint::Notification)
            .build();
        // Necessary to get transparent backgrounds working.
        let visual = screen.get_rgba_visual();
        window.set_visual(visual.as_ref());

        window.move_(screen.get_width() - self.config.width, self.next_y());

        // Contains the icon, text, and image.
        let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        hbox.set_widget_name("container");
        let icon: Option<gtk::Image> = notification.icon.and_then(|icon| {
            let image = self
                .imageref_to_pixbuf(icon, self.config.icon_height, self.config.icon_height)
                .map(|pixbuf| gtk::Image::new_from_pixbuf(Some(&pixbuf)));
            if let Err(ref err) = image {
                info!("Failed to load icon: {}", err);
            }
            image.ok()
        });
        if let Some(icon) = icon {
            hbox.add(&icon);
        }

        // Important: all the labels *must* set wrap to true, so that we can actually set the
        // window's width properly.
        let notification_text_container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        notification_text_container.set_hexpand(true);
        notification_text_container.add(
            &gtk::LabelBuilder::new()
                .label(&notification.summary)
                .name("summary")
                .wrap(true)
                .halign(gtk::Align::Start)
                .build(),
        );
        if let Some(body) = &notification.body {
            notification_text_container.add(
                &gtk::LabelBuilder::new()
                    .label(body)
                    .name("body")
                    .wrap(true)
                    .halign(gtk::Align::Start)
                    .build(),
            );
        }
        if let Some(name) = &notification.application_name {
            notification_text_container.add(
                &gtk::LabelBuilder::new()
                    .label(name)
                    .name("application-name")
                    .wrap(true)
                    .halign(gtk::Align::Start)
                    .build(),
            );
        }

        hbox.add(&notification_text_container);

        let image = notification.hints.image.and_then(|image| {
            let image = self.imageref_to_pixbuf(image, self.config.height, self.config.height);
            if let Err(ref err) = image {
                info!("Failed to load image from {:?}: {}", image, err);
            }
            image.ok()
        });
        if let Some(image) = image {
            let image = resize_pixbuf(image, self.config.height, self.config.height);
            let image = gtk::Image::new_from_pixbuf(Some(&image));
            hbox.add(&image);
        }

        let id = notification.id;
        // On click, close the notification.
        window.connect_button_press_event(clone!(@strong self.tx as tx => move |_, _| {
            debug!("Clicked on notification {}", id);
            if let Err(err) = tx.send(NinomiyaEvent::CloseNotification(id)) {
                error!("Failed to send close notification for {}: {:?}", id, err);
            }
            gtk::Inhibit(false)
        }));

        window.add(&hbox);
        // Necessary to actually properly enforce the size. Otherwise long summaries/bodies will
        // just run off the side of the screen.
        window.resize(self.config.width, self.config.height);
        window.show_all();

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

    fn imageref_to_pixbuf(
        &self,
        image_ref: ImageRef,
        max_width: i32,
        max_height: i32,
    ) -> Result<Pixbuf> {
        match image_ref {
            ImageRef::Url(url) => Ok(resize_pixbuf(
                self.loader.load_from_url(&url)?,
                max_width,
                max_height,
            )),
            ImageRef::IconName(icon_name) => self.loader.load_from_icon(&icon_name, max_height),
            ImageRef::Image {
                width,
                height,
                has_alpha,
                bits_per_sample,
                image_data,
            } => {
                let row_stride = (image_data.len() as i32) / height;
                let pixbuf = Pixbuf::new_from_mut_slice(
                    image_data,
                    gdk_pixbuf::Colorspace::Rgb,
                    has_alpha,
                    bits_per_sample,
                    width,
                    height,
                    row_stride,
                );
                Ok(resize_pixbuf(pixbuf, max_width, max_height))
            }
        }
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

/// Resizes the given pixbuf to fit within the given dimensions. Preserves the aspect ratio.
fn resize_pixbuf(input: Pixbuf, max_width: i32, max_height: i32) -> Pixbuf {
    let input_width = input.get_width() as f32;
    let input_height = input.get_height() as f32;
    let scale_factor = f32::min(
        (max_width as f32) / input_width,
        (max_height as f32) / input_height,
    );
    // Both the max dimensions are greater than the input dimensions, so we don't need to scale.
    if scale_factor >= 1.0 {
        input
    } else {
        input
            .scale_simple(
                (input_width * scale_factor) as i32,
                (input_height * scale_factor) as i32,
                gdk_pixbuf::InterpType::Bilinear,
            )
            .expect("failed to resize; OOM?")
    }
}

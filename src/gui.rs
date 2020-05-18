use crate::config::Config;
use crate::hints::ImageRef;
use crate::image;
use crate::server::{Action, NinomiyaEvent, Notification, Signal};
use anyhow::{Context, Result};
use gdk_pixbuf::Pixbuf;
use gio::prelude::*;
use glib::{clone, object::WeakRef};
use gtk::prelude::*;
use log::{debug, error, info};
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::sync::{mpsc, Mutex};

pub struct Gui {
    app: gtk::Application,
    loader: image::Loader,
    config: Config,
    /// Used to send notifications on a delay.
    tx: glib::Sender<NinomiyaEvent>,
    signal_tx: mpsc::Sender<Signal>,
    windows: Mutex<HashMap<u32, WeakRef<gtk::ApplicationWindow>>>,
}

/// This is the 'default' action key; if present, clicking an action will fire it.
const DEFAULT_KEY: &str = "default";

impl Gui {
    pub fn new(
        config: Config,
        tx: glib::Sender<NinomiyaEvent>,
        signal_tx: mpsc::Sender<Signal>,
    ) -> Rc<Self> {
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
            signal_tx,
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
            // Automatically sets up override redirect, so the window manager won't touch our
            // windows at all.
            .type_(gtk::WindowType::Popup)
            .type_hint(gdk::WindowTypeHint::Notification)
            .build();
        // Necessary to get transparent backgrounds working.
        let visual = screen.get_rgba_visual();
        window.set_visual(visual.as_ref());

        window.move_(
            screen.get_width() - self.config.width - self.config.padding_x,
            self.next_y(),
        );

        // Contains the icon, text, and image.
        let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        hbox.set_widget_name("container");

        notification
            .hints
            .image
            .and_then(|image_ref| {
                let pixbuf = self.imageref_to_pixbuf(
                    image_ref,
                    self.config.image_height,
                    self.config.image_height,
                );
                if let Err(ref err) = pixbuf {
                    info!("Failed to load image: {}", err);
                }
                pixbuf.ok()
            })
            .map(|image| {
                hbox.add(
                    &gtk::ImageBuilder::new()
                        .name("image")
                        .valign(gtk::Align::Start)
                        .pixbuf(&image)
                        .build(),
                )
            });

        // Important: all the labels *must* set wrap to true, so that we can actually set the
        // window's width properly.
        let notification_text_container = gtk::BoxBuilder::new()
            .orientation(gtk::Orientation::Vertical)
            .name("text")
            .hexpand(true)
            .build();
        notification_text_container.add(
            &gtk::LabelBuilder::new()
                .label(&notification.summary)
                .name("summary")
                .xalign(0.0)
                .wrap(true)
                .halign(gtk::Align::Start)
                .build(),
        );
        if let Some(body) = &notification.body {
            notification_text_container.add(
                &gtk::LabelBuilder::new()
                    .label(body)
                    .use_markup(true)
                    .name("body")
                    .xalign(0.0)
                    .wrap(true)
                    .halign(gtk::Align::Start)
                    .build(),
            );
        }

        self.action_buttons(notification.id, &notification.actions)
            .map(|buttons| notification_text_container.add(&buttons));

        hbox.add(&notification_text_container);

        let icon_and_name = gtk::BoxBuilder::new()
            .name("icon-and-name")
            .halign(gtk::Align::End)
            .build();

        if let Some(app_name) = notification.application_name {
            icon_and_name.add(
                &gtk::LabelBuilder::new()
                    .name("application-name")
                    .label(&app_name)
                    .max_width_chars(15)
                    .build(),
            )
        };

        notification
            .icon
            .and_then(|image_ref| {
                let pixbuf = self.imageref_to_pixbuf(
                    image_ref,
                    self.config.icon_height,
                    self.config.icon_height,
                );
                if let Err(ref err) = pixbuf {
                    info!("Failed to load icon: {}", err);
                }
                pixbuf.ok()
            })
            .map(|pixbuf| {
                icon_and_name.add(
                    &gtk::ImageBuilder::new()
                        .name("icon")
                        .pixbuf(&pixbuf)
                        .valign(gtk::Align::Start)
                        .build(),
                )
            });

        notification_text_container.add(&icon_and_name);

        let id = notification.id;
        let has_default = notification
            .actions
            .iter()
            .any(|act| act.key == DEFAULT_KEY);
        // On click, close the notification.
        window.connect_button_press_event(
            clone!(@strong self.tx as tx, @strong self.signal_tx as signal_tx => move |_, _| {
                debug!("Clicked on notification {}", id);
                if has_default {
                        let res = signal_tx.send(Signal::ActionInvoked { id, key: DEFAULT_KEY.into() });
                        if let Err(err) = res {
                            error!("Failed sending signal to GUI thread: {:?}", err);
                        }
                }
                if let Err(err) = tx.send(NinomiyaEvent::CloseNotification(id)) {
                    error!("Failed to send close notification for {}: {:?}", id, err);
                }
                gtk::Inhibit(false)
            }),
        );

        window.add(&hbox);
        // Necessary to actually properly enforce the size. Otherwise long summaries/bodies will
        // just run off the side of the screen.
        window.resize(self.config.width, self.config.image_height);
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

    // Builds a box that contains the buttons for the given notification. Returns None if there
    // shouldn't be a button bar, which can occur if there are no actions *or* if the only action
    // is a default action with an empty label.
    fn action_buttons(&self, id: u32, actions: &Vec<Action>) -> Option<gtk::Box> {
        if actions.is_empty() {
            return None;
        }
        let buttons = gtk::BoxBuilder::new().name("buttons").build();
        assert!(!actions.is_empty());
        // Some programs (such as Telegram) send a default action with an empty label, assuming
        // that clicking on the notification is how users will interact with it. So we avoid
        // displaying empty buttons in that case.
        for action in actions
            .iter()
            .filter(|act| !(act.key == DEFAULT_KEY && act.label.is_empty()))
        {
            let button = gtk::ButtonBuilder::new().label(&action.label).build();
            button.connect_clicked(
                clone!(@strong action.key as key, @strong self.signal_tx as signal_tx => move |_| {
                    debug!("Clicked key {} on notification id {}", key, id);
                    let res = signal_tx.send(Signal::ActionInvoked { id, key: key.clone() });
                    if let Err(err) = res {
                        error!("Failed sending signal to GUI thread: {:?}", err);
                    }
                }),
            );
            buttons.add(&button);
        }
        Some(buttons)
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
            .map_or(self.config.padding_y, |bottom| {
                bottom + self.config.notification_spacing
            })
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
                gdk_pixbuf::InterpType::Hyper,
            )
            .expect("failed to resize; OOM?")
    }
}

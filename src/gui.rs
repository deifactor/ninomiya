use crate::server::{NinomiyaEvent, Notification};
use gio::prelude::*;
use glib::clone;
use gtk::prelude::*;
use std::rc::Rc;

/// Configures how the GUI is rendered.
#[derive(Debug)]
pub struct Config {
    /// Width of notification windows.
    pub width: i32,
    /// Height of notification windows.
    pub height: i32,
}

pub struct Gui {
    app: gtk::Application,
    config: Config,
}

impl Gui {
    pub fn new(config: Config) -> Rc<Self> {
        let app = gtk::Application::new(
            Some("deifactor.ninomiya"),
            gio::ApplicationFlags::FLAGS_NONE,
        )
        .expect("failed to construct application");
        Rc::new(Gui { app, config })
    }

    pub fn run(self: std::rc::Rc<Self>, rx: glib::Receiver<NinomiyaEvent>, argv: &[String]) -> i32 {
        let this = self.clone();
        rx.attach(
            None,
            clone!(@weak this => @default-return glib::Continue(false),
            move |event| {
                println!("Got event {:?}", event);
                match event {
                    NinomiyaEvent::Notification(notification) =>
                        this.notification_window(notification)
                }
                glib::Continue(true)
            }),
        );
        // Not actually necessary, but shuts up GTK.
        self.app.connect_activate(|_app| {});
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
        if let Some(name) = notification.application_name {
            boxx.add(&gtk::Label::new(Some(&name)));
        }
        boxx.add(&gtk::Label::new(Some(&notification.summary)));
        if let Some(body) = notification.body {
            boxx.add(&gtk::Label::new(Some(&body)));
        }

        window.add(&boxx);
        window.show_all();
    }
}

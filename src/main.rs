mod gui;

pub struct Notification {
    /// Human-readable name of the application. Can be blank.
    pub application_name: String,
    /// A brief summary of the notification.
    pub summary: String,
    /// The notification body.
    pub body: Option<String>,
}

fn main() {
    let config = gui::Config {
        width: 300.0,
        height: 100.0,
    };
    let notification = Notification {
        application_name: "hi".to_owned(),
        summary: "hello".to_owned(),
        body: Some("what".to_owned()),
    };
    gui::NotificationWindow::new(config).show(notification);
}

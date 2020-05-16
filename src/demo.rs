//! Implements the `demo` subcommand.
//!
//! The `demo` subcommand sends a series of notifications intended to capture a variety of
//! possibilities: icon present/absent, image present/absent, etc.

use crate::hints::{Hints, ImageRef};
use crate::image::{demo_icon_url, demo_image_url};
use crate::server::{NinomiyaEvent, Notification};
use anyhow::Result;

/// Sends all demo notifications
pub fn send_notifications(tx: glib::Sender<NinomiyaEvent>) -> Result<()> {
    for notification in demo_notifications().into_iter() {
        tx.send(NinomiyaEvent::Notification(notification))?;
    }
    Ok(())
}

/// The list of notifications to send for demo purposes.
fn demo_notifications() -> Vec<Notification> {
    let no_icon_no_image = Notification {
        id: 1,
        icon: None,
        application_name: Some("demo-app-name".into()),
        summary: "no image or icon".into(),
        body: Some("we are not alone / yowaku te tsuyoi hitori hitori da".into()),
        hints: Hints::new(),
    };
    let icon_no_image = Notification {
        id: 2,
        icon: Some(demo_icon_url().into_string()),
        application_name: Some("demo-app-name".into()),
        summary: "icon, no image".into(),
        body: Some("<loax_galax> let's upgrade the world!".into()),
        hints: Hints::new(),
    };
    let no_icon_image = Notification {
        id: 3,
        icon: None,
        application_name: Some("demo-app-name".into()),
        summary: "image, no icon".into(),
        body: Some("gatchaman crowds is a good anime".into()),
        hints: Hints {
            image: Some(ImageRef::Url(demo_image_url())),
        },
    };
    let image_icon = Notification {
        id: 4,
        icon: Some(demo_icon_url().into_string()),
        application_name: Some("demo-app-name".into()),
        summary: "image and icon".into(),
        body: Some("some weird alien gave me this book".into()),
        hints: Hints {
            image: Some(ImageRef::Url(demo_image_url())),
        },
    };
    vec![no_icon_no_image, icon_no_image, no_icon_image, image_icon]
}

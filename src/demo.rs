//! Implements the `demo` subcommand.
//!
//! The `demo` subcommand sends a series of notifications intended to capture a variety of
//! possibilities: icon present/absent, image present/absent, etc.

use crate::hints::{Hints, ImageRef};
use crate::image::{demo_icon_url, demo_image_url};
use crate::server::{Action, NinomiyaEvent, Notification};
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
    let base = || Notification {
        id: 0,
        icon: None,
        actions: vec![],
        application_name: Some("galax".into()),
        summary: "placeholder".into(),
        body: None,
        hints: Hints::new(),
    };

    let demo_icon = ImageRef::Url(demo_icon_url());
    let demo_image = ImageRef::Url(demo_image_url());
    let no_icon_no_image = Notification {
        id: 1,
        summary: "no image or icon".into(),
        body: Some("we are not alone / yowaku te tsuyoi hitori hitori da".into()),
        ..base()
    };
    let icon_no_image = Notification {
        id: 2,
        icon: Some(demo_icon.clone()),
        summary: "icon, no image".into(),
        body: Some("<load_galax> let's upgrade the world!".into()),
        ..base()
    };
    let no_icon_image = Notification {
        id: 3,
        summary: "image, no icon".into(),
        body: Some("<load_galax> gatchaman crowds is a good anime".into()),
        hints: Hints {
            image: Some(demo_image.clone()),
        },
        ..base()
    };
    let image_icon = Notification {
        id: 4,
        icon: Some(demo_icon.clone()),
        summary: "image and icon".into(),
        body: Some("<load_galax> some weird alien gave me this book".into()),
        hints: Hints {
            image: Some(demo_image.clone()),
        },
        ..base()
    };
    let image_and_actions = Notification {
        id: 5,
        icon: Some(demo_icon.clone()),
        summary: "image and actions".into(),
        body: Some("<load_galax> what will you do?".into()),
        hints: Hints {
            image: Some(demo_image.clone()),
        },
        actions: vec![
            Action {
                key: "key-1".into(),
                label: "fight".into(),
            },
            Action {
                key: "key-2".into(),
                label: "perish like a MESS".into(),
            },
        ],
        ..base()
    };
    vec![
        no_icon_no_image,
        icon_no_image,
        no_icon_image,
        image_icon,
        image_and_actions,
    ]
}

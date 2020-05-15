//! Code for loading icons and images.
use anyhow::{anyhow, bail, Context, Result};
use gdk_pixbuf::{Pixbuf, PixbufLoader, PixbufLoaderExt};
use gtk::IconTheme;
use log::warn;
use url::Url;

pub struct Loader {
    /// The GTK icon theme to use when loading icons. If this is `None`, then we failed to get an
    /// icon theme.
    icon_theme: Option<gtk::IconTheme>,
}

impl Loader {
    pub fn new() -> Self {
        Loader::new_with_icon_theme(None)
    }

    /// Constructs an image loader that will use the given icon theme. Passing `None` will result
    /// in using the default icon theme; if this can't be loaded, we will emit a warning.
    pub fn new_with_icon_theme(icon_theme: Option<IconTheme>) -> Self {
        let icon_theme = icon_theme.or_else(|| {
            let theme = IconTheme::get_default();
            if theme.is_none() {
                warn!("Failed to get GTK icon theme");
            }
            theme
        });
        Loader { icon_theme }
    }

    /// Loads the image from the given path. The path can either be a URI or an icon name.
    ///
    /// If the path is a URI, it must either be a file:// URI, which will be loaded from disk, or
    /// one of the special constants `DEMO_ICON_URI` and `DEMO_IMAGE_URI`, which will load images
    /// that are compiled into the binary.
    ///
    /// If the path is an icon name, it will be loaded from the built-in icon theme.
    pub fn load_from_path(&self, path: &str) -> Result<Pixbuf> {
        if path.contains("://") {
            let url = Url::parse(path)?;
            match url.scheme() {
                "ninomiya" => self.load_builtin(url.path()),
                "file" => Ok(Pixbuf::new_from_file(url.path())?),
                _ => bail!(
                    "Can't handle URLs {}: invalid schema (must be 'file' or 'ninomiya')",
                    path
                ),
            }
        } else {
            Ok(Pixbuf::new_from_file(path)?)
        }
    }

    fn load_builtin(&self, path: &str) -> Result<Pixbuf> {
        let image_bytes: &[u8] = match path {
            "/demo-image.png" => include_bytes!("../data/demo-image.png"),
            "/demo-icon.png" => include_bytes!("../data/demo-icon.png"),
            _ => bail!("Unknown builtin image {}", path),
        };
        let loader = PixbufLoader::new();
        loader
            .write(image_bytes)
            .context("failed to write in-memory bytes to  loader")?;
        loader.close().context("failed to close loader")?;
        loader.get_pixbuf().context("Pixbuf didn't finish loading")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gtk_test_runner::run_test;
    use std::path::PathBuf;

    #[test]
    pub fn load_builtins() -> Result<()> {
        run_test(|| -> Result<()> {
            let loader = Loader::new();
            let demo_icon = loader
                .load_from_path("ninomiya:///demo-icon.png")
                .context("failed to load demo icon")?;
            assert_eq!(demo_icon.get_width(), 200);
            assert_eq!(demo_icon.get_height(), 200);

            let demo_image = loader
                .load_from_path("ninomiya:///demo-image.png")
                .context("failed to load demo image")?;
            assert_eq!(demo_image.get_width(), 133);
            assert_eq!(demo_image.get_height(), 190);
            Ok(())
        })
    }

    #[test]
    pub fn load_nonexistent_from_disk() {
        run_test(|| {
            assert!(Loader::new()
                .load_from_path("file:///404/not/found")
                .is_err())
        })
    }

    #[test]
    pub fn load_from_disk() -> Result<()> {
        run_test(|| -> Result<()> {
            let path = PathBuf::from("data/demo-image.png").canonicalize()?;
            let url =
                url::Url::from_file_path(path).map_err(|_| anyhow!("failed to convert url"))?;
            let image = Loader::new().load_from_path(url.as_str())?;
            assert_eq!(image.get_width(), 133);
            assert_eq!(image.get_height(), 190);
            Ok(())
        })
    }

    #[test]
    pub fn load_nonexistent_builtin() {
        run_test(|| {
            let loader = Loader::new();
            assert!(loader
                .load_from_path("ninomiya:///i-do-not-exist.png")
                .is_err())
        });
    }
}

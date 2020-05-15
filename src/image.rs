//! Code for loading icons and images.
use anyhow::{anyhow, bail, Context, Result};
use gdk_pixbuf::{Pixbuf, PixbufLoader, PixbufLoaderExt};
use gtk::prelude::*;
use gtk::IconTheme;
use log::warn;
use url::Url;

pub struct Loader {
    /// The GTK icon theme to use when loading icons. If this is `None`, then we failed to get an
    /// icon theme.
    icon_theme: Option<gtk::IconTheme>,
}

impl Loader {
    /// Constructs a loader that will use the default GTK icon theme.
    pub fn new() -> Self {
        let theme = IconTheme::get_default();
        if theme.is_none() {
            warn!("Failed to get GTK icon theme");
        }
        Loader::new_with_icon_theme(theme)
    }

    /// Constructs an image loader that will use the given icon theme. Passing `None` will result
    /// in using no icon theme.
    pub fn new_with_icon_theme(icon_theme: Option<IconTheme>) -> Self {
        Loader { icon_theme }
    }

    /// Loads the image from the given URI.
    ///
    /// It must either be a file:// URI, which will be loaded from disk, or
    /// one of the special constants `DEMO_ICON_URI` and `DEMO_IMAGE_URI`, which will load images
    /// that are compiled into the binary.
    pub fn load_from_url(&self, url: &Url) -> Result<Pixbuf> {
        match url.scheme() {
            "ninomiya" => self.load_builtin(url.path()),
            "file" => Ok(Pixbuf::new_from_file(url.path())?),
            _ => bail!(
                "Can't handle URLs {}: invalid schema (must be 'file' or 'ninomiya')",
                url
            ),
        }
    }

    /// Loads the icon with the given name.
    pub fn load_from_icon(&self, icon_name: &str, size: i32) -> Result<Pixbuf> {
        self.icon_theme
            .as_ref()
            .context("no icon theme specified")?
            .load_icon(icon_name, size, gtk::IconLookupFlags::FORCE_SIZE)?
            .with_context(|| anyhow!("icon {} not found", icon_name))
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
    use std::path::PathBuf;

    #[test]
    pub fn load_builtins() -> Result<()> {
        let loader = Loader::new_with_icon_theme(None);
        let demo_icon = loader
            .load_from_url(&Url::parse("ninomiya:///demo-icon.png")?)
            .context("failed to load demo icon")?;
        assert_eq!(demo_icon.get_width(), 200);
        assert_eq!(demo_icon.get_height(), 200);

        let demo_image = loader
            .load_from_url(&Url::parse("ninomiya:///demo-image.png")?)
            .context("failed to load demo image")?;
        assert_eq!(demo_image.get_width(), 133);
        assert_eq!(demo_image.get_height(), 190);
        Ok(())
    }

    #[test]
    pub fn load_nonexistent_from_disk() -> Result<()> {
        assert!(Loader::new_with_icon_theme(None)
            .load_from_url(&Url::parse("file:///404/not/found")?)
            .is_err());
        Ok(())
    }

    #[test]
    pub fn load_from_disk() -> Result<()> {
        let path = PathBuf::from("data/demo-image.png").canonicalize()?;
        let url = url::Url::from_file_path(path).map_err(|_| anyhow!("failed to convert url"))?;
        let image = Loader::new_with_icon_theme(None).load_from_url(&url)?;
        assert_eq!(image.get_width(), 133);
        assert_eq!(image.get_height(), 190);
        Ok(())
    }

    #[test]
    pub fn load_nonexistent_builtin() -> Result<()> {
        let loader = Loader::new_with_icon_theme(None);
        assert!(loader
            .load_from_url(&Url::parse("ninomiya:///i-do-not-exist.png")?)
            .is_err());
        Ok(())
    }
}

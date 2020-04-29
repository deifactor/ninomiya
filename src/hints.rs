use anyhow::{anyhow, Result};
use dbus::arg;
use log::error;
use std::collections::HashMap;
use std::path::PathBuf;

pub enum ImageRef {
    Image(gdk_pixbuf::Pixbuf),
    Path(PathBuf),
}

pub type HintMap<'a> = HashMap<&'a str, arg::Variant<Box<dyn arg::RefArg>>>;

static IMAGE_DATA: &str = "image-data";
static IMAGE_PATH: &str = "image-path";
static APP_ICON: &str = "app-icon";

/// Provides convenient access to the standardized hints of a notification.
pub struct Hints<'a> {
    pub image: Option<ImageRef>,
    pub icon: Option<String>,
    pub other: HintMap<'a>,
}
impl<'a> Hints<'a> {
    pub fn new() -> Self {
        Hints {
            image: None,
            icon: None,
            other: HashMap::new(),
        }
    }

    /// Converts this into a format suitable to be passed to the dbus API.
    pub fn into_dbus(self) -> HintMap<'a> {
        let mut map = self.other;
        if let Some(ImageRef::Image(pixbuf)) = self.image {
            map.insert(IMAGE_DATA, Self::pixbuf_to_variant(&pixbuf));
        } else if let Some(ImageRef::Path(path)) = self.image {
            map.insert(IMAGE_PATH, Self::pathbuf_to_variant(path));
        }
        if let Some(path) = self.icon {
            map.insert(
                APP_ICON,
                arg::Variant(Box::new(path) as Box<dyn arg::RefArg>),
            );
        }
        map
    }

    /// Takes a pixbuf and encodes it in the format the dbus notification server expects.
    /// Per the specification, raw images are
    /// "raw image data structure of signature (iiibiiay) which describes the width, height, rowstride,
    /// has alpha, bits per sample, channels and image data respectively".
    pub fn pixbuf_to_variant(pixbuf: &gdk_pixbuf::Pixbuf) -> arg::Variant<Box<dyn arg::RefArg>> {
        let tuple = (
            pixbuf.get_width(),
            pixbuf.get_height(),
            pixbuf.get_rowstride(),
            pixbuf.get_has_alpha(),
            pixbuf.get_bits_per_sample(),
            pixbuf.get_n_channels(),
            unsafe { pixbuf.get_pixels().to_owned() },
        );
        arg::Variant(Box::new(tuple) as Box<dyn arg::RefArg>)
    }

    fn pathbuf_to_variant(path: PathBuf) -> arg::Variant<Box<dyn arg::RefArg>> {
        let path_str: String = path.to_string_lossy().into_owned();
        arg::Variant(Box::new(path_str) as Box<dyn arg::RefArg>)
    }
}

/// Attempts to parse the given variant value as a raw image. Per the specification, raw images are
/// "raw image data structure of signature (iiibiiay) which describes the width, height, rowstride,
/// has alpha, bits per sample, channels and image data respectively".
fn raw_image_from_variant(
    variant: &arg::Variant<Box<dyn arg::RefArg>>,
) -> Result<gdk_pixbuf::Pixbuf> {
    let expected_signature =
        dbus::strings::Signature::new("(iiibiiay)").expect("parsing expected signature failed?!");
    let signature = variant.0.signature();
    if signature != expected_signature {
        return Err(anyhow!(
            "Unexpected signature when getting image {} (expected {})"
        ));
    }
    // use an anonymous function so we can use ? to bail out early, then convert the None into an
    // Err case
    let pixbuf = (|| {
        let mut iter = variant.0.as_iter()?;
        let width = iter.next()?.as_i64()? as i32;
        let height = iter.next()?.as_i64()? as i32;
        let _rowstride = iter.next()?.as_i64()?;
        let has_alpha = iter.next()?.as_i64()? != 0;
        let bits_per_sample = iter.next()?.as_i64()? as i32;
        let _channels = iter.next()?.as_i64()?;
        let pixbuf = gdk_pixbuf::Pixbuf::new(
            gdk_pixbuf::Colorspace::Rgb,
            has_alpha,
            bits_per_sample,
            width,
            height,
        )?;

        // Unfortunately, we need the box_clone here because `iter.next()?` gets us a reference
        // bounded by the lifetime of the original variant, and `as_any` (which `cast` calls)
        // requires a static reference.
        let dyn_image_data = iter.next()?.box_clone();
        let image_data = arg::cast::<Vec<u8>>(&*dyn_image_data)?;
        unsafe {
            let pixbuf_pixels = pixbuf.get_pixels();
            if image_data.len() != pixbuf_pixels.len() {
                error!(
                    "image data had {} bytes, but we wanted to fill {} bytes for the pixbuf",
                    image_data.len(),
                    pixbuf_pixels.len()
                );
                return None;
            }
            pixbuf_pixels.copy_from_slice(image_data);
        }
        Some(pixbuf)
    })()
    .ok_or_else(|| anyhow!("Failed to get field from image struct"))?;
    Ok(pixbuf)
}

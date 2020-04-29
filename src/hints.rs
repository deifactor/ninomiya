use anyhow::{anyhow, Context, Result};
use dbus::arg;
use log::error;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug)]
pub enum ImageRef {
    Image {
        width: i32,
        height: i32,
        has_alpha: bool,
        bits_per_sample: i32,
        image_data: Vec<u8>,
    },
    Path(PathBuf),
}

pub type HintMap<'a> = HashMap<&'a str, arg::Variant<Box<dyn arg::RefArg>>>;

static IMAGE_DATA: &str = "image-data";
static IMAGE_PATH: &str = "image-path";
static APP_ICON: &str = "app-icon";

/// Provides convenient access to the standardized hints of a notification.
#[derive(Debug)]
pub struct Hints {
    pub image: Option<ImageRef>,
    pub icon: Option<String>,
}
impl Hints {
    pub fn new() -> Self {
        Hints {
            image: None,
            icon: None,
        }
    }

    /// Builds a new instance of this using the given dbus hint map.
    pub fn from_dbus(mut map: HintMap) -> Result<Self> {
        let mut hints = Hints::new();

        // icon is always taken from app_ic
        if let Some(icon) = map.remove(APP_ICON) {
            let icon_str = icon
                .0
                .as_str()
                .context("`app-icon` did not have expected signature")?;
            hints.icon = Some(icon_str.to_owned());
        }

        // image-data takes priority over image-path. We do image-path first so we'll always clear
        // both out of the map.
        if let Some(image_path) = map.remove(IMAGE_PATH) {
            let image_path_str = image_path
                .0
                .as_str()
                .context("`image-path` did not have expected signature")?;
            hints.image = Some(ImageRef::Path(image_path_str.into()));
        }
        if let Some(image_bytes) = map.remove(IMAGE_DATA) {
            hints.image = Some(raw_image_from_variant(&image_bytes)?);
        }

        Ok(hints)
    }

    /// Converts this into a format suitable to be passed to the dbus API.
    pub fn into_dbus(self) -> HintMap<'static> {
        let mut map = HashMap::new();
        if let Some(ImageRef::Image {
            width,
            height,
            has_alpha,
            bits_per_sample,
            image_data,
        }) = self.image
        {
            let rowstride = width * bits_per_sample;
            let n_channels = if has_alpha { 4 } else { 3 };
            let tuple = (
                width,
                height,
                rowstride,
                has_alpha,
                bits_per_sample,
                n_channels,
                image_data,
            );
            map.insert(
                IMAGE_DATA,
                arg::Variant(Box::new(tuple) as Box<dyn arg::RefArg>),
            );
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
fn raw_image_from_variant(variant: &arg::Variant<Box<dyn arg::RefArg>>) -> Result<ImageRef> {
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
        // Unfortunately, we need the box_clone here because `iter.next()?` gets us a reference
        // bounded by the lifetime of the original variant, and `as_any` (which `cast` calls)
        // requires a static reference.
        let dyn_image_data = iter.next()?.box_clone();
        let image_data = arg::cast::<Vec<u8>>(&*dyn_image_data)?;
        let image = ImageRef::Image {
            width,
            height,
            has_alpha,
            bits_per_sample,
            // TODO: we wind up cloning the image data here *twice*. we shouldn't really need to do
            // that.
            image_data: image_data.clone(),
        };
        Some(image)
    })()
    .ok_or_else(|| anyhow!("Failed to get field from image struct"))?;
    Ok(pixbuf)
}

use anyhow::{anyhow, Context, Result};
use dbus::arg;
use derivative::Derivative;
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

fn show_pixel_count(image_data: &Vec<u8>, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    write!(f, "{} bytes", image_data.len())
}

#[derive(Derivative)]
#[derivative(Debug)]
pub enum ImageRef {
    Image {
        width: i32,
        height: i32,
        has_alpha: bool,
        bits_per_sample: i32,
        #[derivative(Debug(format_with = "show_pixel_count"))]
        image_data: Vec<u8>,
    },
    Path(PathBuf),
}

pub type HintMap<'a> = HashMap<&'a str, arg::Variant<Box<dyn arg::RefArg>>>;

static IMAGE_DATA: &str = "image-data";
static IMAGE_PATH: &str = "image-path";
// Despite the name, this stores the *image*. I guess that's why it's deprecated.
static ICON_DATA: &str = "icon_data";

/// Provides convenient access to the standardized hints of a notification.
#[derive(Debug)]
pub struct Hints {
    pub image: Option<ImageRef>,
}
impl Hints {
    pub fn new() -> Self {
        Hints { image: None }
    }

    /// Builds a new instance of this using the given dbus hint map.
    pub fn from_dbus(mut map: HintMap) -> Result<Self> {
        let mut hints = Hints::new();

        // We do these in reverse precedence order so we always clear them out from the map.
        if let Some(icon_data) = map.remove(ICON_DATA) {
            hints.image = Some(raw_image_from_variant(icon_data)?);
        }
        if let Some(image_path) = map.remove(IMAGE_PATH) {
            let image_path_str = image_path
                .0
                .as_str()
                .context("`image-path` did not have expected signature")?;
            hints.image = Some(ImageRef::Path(image_path_str.into()));
        }
        if let Some(image_bytes) = map.remove(IMAGE_DATA) {
            hints.image = Some(raw_image_from_variant(image_bytes)?);
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
        map
    }

    fn pathbuf_to_variant(path: PathBuf) -> arg::Variant<Box<dyn arg::RefArg>> {
        let path_str: String = path.to_string_lossy().into_owned();
        arg::Variant(Box::new(path_str) as Box<dyn arg::RefArg>)
    }
}

/// Converts a refarg, which *must* contain a Vec<u8>, into the corresponding Vec<u8>.
///
/// This function is necessary because we can't get a `&(dyn arg::RefArg + 'static)`, but we need
/// that `'static` bound in order to use `arg::cast`.
unsafe fn refarg_to_bytes<'a>(refarg: &'a dyn arg::RefArg) -> &'a Vec<u8> {
    assert_eq!(
        refarg.signature(),
        dbus::strings::Signature::new("ay").unwrap()
    );
    // This *should* be safe. For one, Vec<u8> and dbus-rs's InternalArray type actually don't own
    // any references, so they're 'static. For another, I *think* lying to the compiler about
    // lifetimes is safe as long as you don't actually violate those lifetimes. And since the
    // underlying lifetime in this case is the lifetime of the `raw_image_from_variant` body, and
    // we're cloning the vec anyway in order to return it... I think we're good.
    let refarg =
        std::mem::transmute::<&'a dyn arg::RefArg, &'a (dyn arg::RefArg + 'static)>(refarg);
    arg::cast(refarg).expect("thought we were getting a Vec<u8>???")
}

/// Attempts to parse the given variant value as a raw image. Per the specification, raw images are
/// "raw image data structure of signature (iiibiiay) which describes the width, height, rowstride,
/// has alpha, bits per sample, channels and image data respectively".
fn raw_image_from_variant(variant: arg::Variant<Box<dyn arg::RefArg>>) -> Result<ImageRef> {
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
    (|| {
        let mut iter = variant.0.as_iter()?;
        let width = iter.next()?.as_i64()? as i32;
        let height = iter.next()?.as_i64()? as i32;
        let _rowstride = iter.next()?.as_i64()?;
        let has_alpha = iter.next()?.as_i64()? != 0;
        let bits_per_sample = iter.next()?.as_i64()? as i32;
        let _channels = iter.next()?.as_i64()?;
        let cloned = iter.next()?;
        let bytes = unsafe { refarg_to_bytes(&*cloned) };
        let image = ImageRef::Image {
            width,
            height,
            has_alpha,
            bits_per_sample,
            // TODO: we wind up cloning the image data here *twice*. we shouldn't really need to do
            // that.
            image_data: bytes.clone(),
        };
        Some(image)
    })()
    .context("falied to unpack raw image from dbus")
}

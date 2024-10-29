use ahash::HashMap;
use egui::{
    load::{BytesPoll, ImageLoadResult, ImageLoader, ImagePoll, LoadError, SizeHint},
    mutex::Mutex,
    ColorImage,
};
use std::{mem::size_of, path::Path, sync::Arc};

type Entry = Result<Arc<ColorImage>, String>;

#[derive(Default)]
pub struct JxlLoader {
    cache: Mutex<HashMap<String, Entry>>,
}

impl JxlLoader {
    pub const ID: &'static str = egui::generate_loader_id!(JxlLoader);
}

fn is_supported_uri(uri: &str) -> bool {
    let Some(ext) = Path::new(uri).extension().and_then(|ext| ext.to_str()) else {
        return false;
    };

    ext == "jxl"
}

fn is_unsupported_mime(mime: &str) -> bool {
    mime != "image/jxl"
}

impl ImageLoader for JxlLoader {
    fn id(&self) -> &str {
        Self::ID
    }

    fn load(&self, ctx: &egui::Context, uri: &str, _: SizeHint) -> ImageLoadResult {
        // three stages of guessing if we support loading the image:
        // 1. URI extension
        // 2. Mime from `BytesPoll::Ready`
        // 3. image::guess_format

        // (1)
        if !is_supported_uri(uri) {
            return Err(LoadError::NotSupported);
        }

        let mut cache = self.cache.lock();
        if let Some(entry) = cache.get(uri).cloned() {
            match entry {
                Ok(image) => Ok(ImagePoll::Ready { image }),
                Err(err) => Err(LoadError::Loading(err)),
            }
        } else {
            match ctx.try_load_bytes(uri) {
                Ok(BytesPoll::Ready { bytes, mime, .. }) => {
                    // (2 and 3)
                    if mime.as_deref().is_some_and(is_unsupported_mime) {
                        return Err(LoadError::NotSupported);
                    }

                    log::trace!("started loading {uri:?}");
                    let result = crate::image::load_image_bytes_jxl(&bytes).map(Arc::new);
                    log::trace!("finished loading {uri:?}");
                    cache.insert(uri.into(), result.clone());
                    match result {
                        Ok(image) => Ok(ImagePoll::Ready { image }),
                        Err(err) => Err(LoadError::Loading(err)),
                    }
                }
                Ok(BytesPoll::Pending { size }) => Ok(ImagePoll::Pending { size }),
                Err(err) => Err(err),
            }
        }
    }

    fn forget(&self, uri: &str) {
        let _ = self.cache.lock().remove(uri);
    }

    fn forget_all(&self) {
        self.cache.lock().clear();
    }

    fn byte_size(&self) -> usize {
        self.cache
            .lock()
            .values()
            .map(|result| match result {
                Ok(image) => image.pixels.len() * size_of::<egui::Color32>(),
                Err(err) => err.len(),
            })
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_support() {
        assert!(!is_supported_uri("https://test.png"));
        assert!(!is_supported_uri("test.jpeg"));
        assert!(!is_supported_uri("http://test.gif"));
        assert!(!is_supported_uri("test.webp"));
        assert!(!is_supported_uri("file://test"));
        assert!(!is_supported_uri("test.svg"));
        assert!(is_supported_uri("test.jxl"));
    }
}

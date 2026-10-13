use crate::domain::{Fit, ReadingMode};
use iced::widget::image::Handle;
use std::{collections::HashMap, sync::Arc};

#[derive(Debug, Clone)]
pub struct Decoded {
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<Vec<u8>>,
}
struct Pixels(Arc<Vec<u8>>);
impl AsRef<[u8]> for Pixels {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl Decoded {
    pub fn handle(&self) -> Handle {
        Handle::from_rgba(
            self.width,
            self.height,
            bytes::Bytes::from_owner(Pixels(self.rgba.clone())),
        )
    }
}

#[derive(Debug, Clone)]
pub struct PageImage {
    pub handle: Handle,
    pub width: u32,
    pub height: u32,
    pub bytes: usize,
    pub enhanced: bool,
    pub original: Option<Decoded>,
}

#[derive(Debug, Clone)]
pub struct ChapterSpan {
    pub id: i64,
    pub start: usize,
    pub count: usize,
}

pub struct Reader {
    pub chapter_id: i64,
    pub urls: Vec<String>,
    pub spans: Vec<ChapterSpan>,
    pub append_loading: Option<i64>,
    pub append_error: Option<String>,
    pub notice: Option<String>,
    pub current: usize,
    pub mode: ReadingMode,
    pub fit: Fit,
    pub images: HashMap<usize, PageImage>,
    pub dimensions: HashMap<usize, (u32, u32)>,
    pub errors: HashMap<usize, String>,
    pub loading: std::collections::HashSet<usize>,
    pub enhancing: std::collections::HashSet<usize>,
    pub zoom: f32,
    pub scroll: f32,
    pub viewport: f32,
    pub width: f32,
}
impl Reader {
    pub fn new(
        chapter_id: i64,
        urls: Vec<String>,
        page: usize,
        mode: ReadingMode,
        fit: Fit,
    ) -> Self {
        let current = page.min(urls.len().saturating_sub(1));
        let count = urls.len();
        Self {
            chapter_id,
            urls,
            spans: vec![ChapterSpan {
                id: chapter_id,
                start: 0,
                count,
            }],
            append_loading: None,
            append_error: None,
            notice: None,
            current,
            mode,
            fit,
            images: HashMap::new(),
            dimensions: HashMap::new(),
            errors: HashMap::new(),
            loading: Default::default(),
            enhancing: Default::default(),
            zoom: 1.0,
            scroll: 0.0,
            viewport: 800.0,
            width: 800.0,
        }
    }
    pub fn span_at(&self, index: usize) -> &ChapterSpan {
        self.spans
            .iter()
            .rev()
            .find(|s| s.start <= index)
            .unwrap_or(&self.spans[0])
    }
    pub fn local_page(&self) -> usize {
        self.current
            .saturating_sub(self.span_at(self.current).start)
    }
    pub fn set_current(&mut self, index: usize) {
        self.current = index.min(self.urls.len().saturating_sub(1));
        self.chapter_id = self.span_at(self.current).id;
    }
    pub fn append(&mut self, id: i64, urls: Vec<String>) {
        self.append_loading = None;
        if urls.is_empty() || self.spans.iter().any(|s| s.id == id) {
            return;
        }
        self.spans.push(ChapterSpan {
            id,
            start: self.urls.len(),
            count: urls.len(),
        });
        self.urls.extend(urls);
    }
    pub fn wanted(&self, prefetch: usize) -> std::ops::Range<usize> {
        self.current.saturating_sub(1)..(self.current + prefetch + 1).min(self.urls.len())
    }
    pub fn insert(&mut self, index: usize, image: PageImage, budget: usize) {
        let anchor = self.locate(self.scroll);
        let within = self.scroll - self.offset(anchor);
        self.dimensions.insert(index, (image.width, image.height));
        if self.mode.strip() {
            self.scroll = self.offset(anchor) + within;
        }
        self.loading.remove(&index);
        self.errors.remove(&index);
        self.images.insert(index, image);
        loop {
            let total: usize = self.images.values().map(|i| i.bytes).sum();
            if total <= budget {
                break;
            }
            let victim = self
                .images
                .keys()
                .copied()
                .filter(|&i| i != self.current && i != self.current + 1)
                .max_by_key(|&i| i.abs_diff(self.current));
            match victim {
                Some(i) => {
                    self.images.remove(&i);
                }
                None => break,
            }
        }
    }
    pub fn finished(&self) -> bool {
        if self.urls.is_empty() {
            return false;
        }
        if self.mode.strip() {
            let last = self.span_at(self.current).start
                + self.span_at(self.current).count.saturating_sub(1);
            if !self.dimensions.contains_key(&last) {
                return false;
            }
            self.scroll + self.viewport
                >= self.offset(self.span_at(self.current).start + self.span_at(self.current).count)
                    - self.gap()
                    - 2.0
        } else {
            self.local_page() + 1 >= self.span_at(self.current).count
        }
    }
    pub fn fitted_size(&self, index: usize, available: (f32, f32)) -> (f32, f32) {
        let (w, h) = self.dimensions.get(&index).copied().unwrap_or((800, 1200));
        let (w, h) = (w as f32, h as f32);
        let scale = match self.fit {
            Fit::Screen => (available.0 / w).min(available.1 / h),
            Fit::Width => available.0 / w,
            Fit::Height => available.1 / h,
            Fit::Original => 1.0,
        } * self.zoom;
        (w * scale, h * scale)
    }
    pub fn page_height(&self, index: usize) -> f32 {
        let (w, h) = self.dimensions.get(&index).copied().unwrap_or((800, 1200));
        self.width * self.zoom * h as f32 / w.max(1) as f32
    }
    pub fn offset(&self, index: usize) -> f32 {
        (0..index).map(|i| self.page_height(i) + self.gap()).sum()
    }
    pub fn gap(&self) -> f32 {
        if self.mode == ReadingMode::WebtoonGaps {
            12.0
        } else {
            0.0
        }
    }
    pub fn locate(&self, y: f32) -> usize {
        let mut top = 0.0;
        for i in 0..self.urls.len() {
            top += self.page_height(i) + self.gap();
            if top > y {
                return i;
            }
        }
        self.urls.len().saturating_sub(1)
    }
    pub fn visible(&self) -> std::ops::Range<usize> {
        self.locate((self.scroll - 500.0).max(0.0))
            ..(self.locate(self.scroll + self.viewport + 500.0) + 1).min(self.urls.len())
    }
}

pub fn decode(bytes: &[u8]) -> anyhow::Result<Decoded> {
    use image::ImageReader;
    let mut reader = ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(256 * 1024 * 1024);
    limits.max_image_width = Some(32768);
    limits.max_image_height = Some(131072);
    reader.limits(limits);
    let decoded = reader.decode()?;
    anyhow::ensure!(
        u64::from(decoded.width()) * u64::from(decoded.height()) * 4 <= 256 * 1024 * 1024,
        "Decoded page exceeds 256 MiB"
    );
    let decoded = decoded.into_rgba8();
    Ok(Decoded {
        width: decoded.width(),
        height: decoded.height(),
        rgba: Arc::new(decoded.into_raw()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strip_geometry_and_prefetch_stay_bounded() {
        let r = Reader::new(
            1,
            vec![String::new(); 1000],
            500,
            ReadingMode::Webtoon,
            Fit::Width,
        );
        assert_eq!(r.wanted(3), 499..504);
        assert_eq!(r.locate(r.offset(500) + 1.0), 500);
        assert!(r.visible().len() < 5);
    }
    #[test]
    fn corrupt_pages_are_errors() {
        assert!(decode(b"not an image").is_err());
    }

    #[test]
    fn avif_images_decode_without_an_encoder() {
        let image = decode(include_bytes!("../assets/test/cover.avif")).unwrap();
        assert_eq!((image.width, image.height), (24, 32));
        assert_eq!(image.rgba.len(), 24 * 32 * 4);
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;
    fn page(w: u32, h: u32) -> PageImage {
        PageImage {
            handle: Handle::from_rgba(1, 1, vec![0, 0, 0, 255]),
            width: w,
            height: h,
            bytes: 4,
            enhanced: false,
            original: None,
        }
    }
    #[test]
    fn continuous_chapters_keep_local_page_numbers_and_completion() {
        let mut r = Reader::new(
            1,
            vec![String::new(); 2],
            0,
            ReadingMode::Webtoon,
            Fit::Width,
        );
        r.append(2, vec![String::new(); 3]);
        r.insert(1, page(800, 1200), 1024);
        r.scroll = r.offset(2) - r.viewport;
        r.set_current(1);
        assert!(r.finished());
        r.set_current(2);
        assert_eq!(r.chapter_id, 2);
        assert_eq!(r.local_page(), 0);
        assert!(!r.finished());
        r.set_current(4);
        assert_eq!(r.local_page(), 2);
        assert!(!r.finished());
        r.append(2, vec![String::new(); 3]);
        assert_eq!(r.urls.len(), 5);
    }
    #[test]
    fn loading_earlier_dimensions_preserves_the_visible_anchor() {
        let mut r = Reader::new(
            1,
            vec![String::new(); 5],
            2,
            ReadingMode::WebtoonGaps,
            Fit::Width,
        );
        r.scroll = r.offset(2) + 80.0;
        r.insert(0, page(800, 2400), 1024);
        assert_eq!(r.locate(r.scroll), 2);
        assert!((r.scroll - r.offset(2) - 80.0).abs() < 0.01);
    }
    #[test]
    fn fit_width_height_and_original_have_distinct_geometry() {
        let mut r = Reader::new(
            1,
            vec![String::new()],
            0,
            ReadingMode::LeftToRight,
            Fit::Width,
        );
        r.dimensions.insert(0, (400, 1200));
        assert_eq!(r.fitted_size(0, (800.0, 600.0)), (800.0, 2400.0));
        r.fit = Fit::Height;
        assert_eq!(r.fitted_size(0, (800.0, 600.0)), (200.0, 600.0));
        r.fit = Fit::Original;
        assert_eq!(r.fitted_size(0, (800.0, 600.0)), (400.0, 1200.0));
    }
    #[test]
    fn a_long_final_strip_is_not_finished_until_its_bottom() {
        let mut r = Reader::new(1, vec![String::new()], 0, ReadingMode::Webtoon, Fit::Width);
        r.insert(0, page(800, 10000), 1024);
        assert!(!r.finished());
        r.scroll = 9200.0;
        assert!(r.finished());
    }
}

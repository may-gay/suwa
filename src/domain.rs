use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Manga {
    pub id: i64,
    pub title: String,
    pub thumbnail_url: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub genre: Vec<String>,
    pub status: String,
    pub in_library: bool,
    pub unread_count: i64,
    pub download_count: i64,
    pub bookmark_count: i64,
    pub chapters: Counts,
    pub latest_read_chapter: Option<ChapterDates>,
    pub latest_uploaded_chapter: Option<ChapterDates>,
    pub in_library_at: String,
    pub categories: Nodes<Category>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Counts {
    pub total_count: i64,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ChapterDates {
    pub last_read_at: String,
    pub upload_date: String,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LibrarySort {
    #[default]
    Title,
    RecentlyRead,
    LatestChapter,
    ChapterCount,
    UnreadCount,
    Added,
}
impl LibrarySort {
    pub const ALL: [Self; 6] = [
        Self::Title,
        Self::RecentlyRead,
        Self::LatestChapter,
        Self::ChapterCount,
        Self::UnreadCount,
        Self::Added,
    ];
}
impl fmt::Display for LibrarySort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Title => "Title",
            Self::RecentlyRead => "Recently read",
            Self::LatestChapter => "Latest chapter",
            Self::ChapterCount => "Chapter count",
            Self::UnreadCount => "Unread count",
            Self::Added => "Recently added",
        })
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LibraryStatus {
    #[default]
    All,
    Ongoing,
    Completed,
    Hiatus,
    Cancelled,
    Licensed,
    Finished,
    Unknown,
}
impl LibraryStatus {
    pub const ALL: [Self; 8] = [
        Self::All,
        Self::Ongoing,
        Self::Completed,
        Self::Hiatus,
        Self::Cancelled,
        Self::Licensed,
        Self::Finished,
        Self::Unknown,
    ];
    pub fn matches(self, status: &str) -> bool {
        self == Self::All
            || status
                == match self {
                    Self::All => "",
                    Self::Ongoing => "ONGOING",
                    Self::Completed => "COMPLETED",
                    Self::Hiatus => "ON_HIATUS",
                    Self::Cancelled => "CANCELLED",
                    Self::Licensed => "LICENSED",
                    Self::Finished => "PUBLISHING_FINISHED",
                    Self::Unknown => "UNKNOWN",
                }
    }
}
impl fmt::Display for LibraryStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::All => "Any status",
            Self::Ongoing => "Ongoing",
            Self::Completed => "Completed",
            Self::Hiatus => "On hiatus",
            Self::Cancelled => "Cancelled",
            Self::Licensed => "Licensed",
            Self::Finished => "Publishing finished",
            Self::Unknown => "Unknown",
        })
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Chapter {
    pub id: i64,
    pub manga_id: i64,
    pub name: String,
    pub chapter_number: f64,
    pub source_order: i64,
    pub scanlator: Option<String>,
    pub is_read: bool,
    pub is_bookmarked: bool,
    pub is_downloaded: bool,
    #[serde(deserialize_with = "page_index")]
    pub last_page_read: usize,
    #[serde(deserialize_with = "page_index")]
    pub page_count: usize,
}

fn page_index<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<usize, D::Error> {
    let value = i64::deserialize(deserializer)?;
    Ok(value.max(0) as usize)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub default: bool,
    pub order: i64,
    pub is_default_category: bool,
}

impl Manga {
    pub fn in_category(&self, id: i64, default_category: Option<i64>) -> bool {
        (Some(id) == default_category && self.categories.nodes.is_empty())
            || self
                .categories
                .nodes
                .iter()
                .any(|category| category.id == id)
    }

    pub fn visible_in_library(
        &self,
        hidden: &std::collections::HashSet<i64>,
        default_category: Option<i64>,
    ) -> bool {
        !hidden
            .iter()
            .any(|id| self.in_category(*id, default_category))
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Source {
    pub id: String,
    pub display_name: String,
    pub lang: String,
    pub supports_latest: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Nodes<T> {
    pub nodes: Vec<T>,
}
impl<T> Default for Nodes<T> {
    fn default() -> Self {
        Self { nodes: Vec::new() }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReadingMode {
    #[default]
    RightToLeft,
    LeftToRight,
    Vertical,
    Webtoon,
    WebtoonGaps,
}
impl ReadingMode {
    pub const ALL: [Self; 5] = [
        Self::RightToLeft,
        Self::LeftToRight,
        Self::Vertical,
        Self::Webtoon,
        Self::WebtoonGaps,
    ];
    pub fn strip(self) -> bool {
        matches!(self, Self::Webtoon | Self::WebtoonGaps)
    }
}
impl fmt::Display for ReadingMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::RightToLeft => "Right to left",
            Self::LeftToRight => "Left to right",
            Self::Vertical => "Vertical pages",
            Self::Webtoon => "Webtoon",
            Self::WebtoonGaps => "Webtoon with gaps",
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fit {
    #[default]
    Screen,
    Width,
    Height,
    Original,
}
impl Fit {
    pub const ALL: [Self; 4] = [Self::Screen, Self::Width, Self::Height, Self::Original];
}
impl fmt::Display for Fit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Screen => "Fit screen",
            Self::Width => "Fit width",
            Self::Height => "Fit height",
            Self::Original => "Original size",
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthMode {
    #[default]
    None,
    Basic,
    UiLogin,
    SimpleLogin,
}
impl AuthMode {
    pub const ALL: [Self; 4] = [Self::None, Self::Basic, Self::UiLogin, Self::SimpleLogin];
}
impl fmt::Display for AuthMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::None => "No login",
            Self::Basic => "HTTP Basic",
            Self::UiLogin => "UI login",
            Self::SimpleLogin => "Simple login",
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub server: String,
    pub username: String,
    pub auth: AuthMode,
    pub read_only: bool,
    pub remember_login: bool,
    pub mode: ReadingMode,
    pub fit: Fit,
    pub dark: bool,
    pub oled: bool,
    pub double_page: bool,
    pub prefetch: usize,
    pub cache_mib: usize,
    pub disk_cache_mib: usize,
    pub theme: String,
    pub accent: String,
    pub model_path: String,
    pub cuda_device: i32,
    pub upscale: bool,
    pub strip_width: f32,
    pub library_sort: LibrarySort,
    pub reverse_sort: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            server: "http://localhost:4567".into(),
            username: String::new(),
            auth: AuthMode::None,
            read_only: true,
            remember_login: true,
            mode: ReadingMode::default(),
            fit: Fit::default(),
            dark: true,
            oled: false,
            double_page: false,
            prefetch: 3,
            cache_mib: 192,
            disk_cache_mib: 1024,
            theme: "Pastel goth".into(),
            accent: "#c8a4e8".into(),
            model_path: String::new(),
            cuda_device: 0,
            upscale: false,
            strip_width: 850.0,
            library_sort: LibrarySort::Title,
            reverse_sort: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Progress {
    pub chapter_id: i64,
    pub page: usize,
    pub read: bool,
    pub explicit: bool,
    pub baseline_page: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Reconcile {
    Apply { page: usize, read: bool },
    Conflict,
}
pub fn reconcile(local: &Progress, remote: &Chapter) -> Reconcile {
    if local.explicit {
        return Reconcile::Apply {
            page: local.page,
            read: local.read,
        };
    }
    if remote.is_read {
        return Reconcile::Apply {
            page: remote.last_page_read.max(local.page),
            read: true,
        };
    }
    if remote.last_page_read != local.baseline_page
        && remote.last_page_read != local.page
        && !local.read
    {
        return Reconcile::Conflict;
    }
    Reconcile::Apply {
        page: local.page,
        read: local.read,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_server_page_counts_are_not_allocation_sizes() {
        let chapter: Chapter =
            serde_json::from_value(serde_json::json!({"id":1,"pageCount":-1,"lastPageRead":-1}))
                .unwrap();
        assert_eq!(chapter.page_count, 0);
        assert_eq!(chapter.last_page_read, 0);
    }
    #[test]
    fn completed_chapters_do_not_become_unread() {
        let p = Progress {
            chapter_id: 1,
            page: 2,
            read: false,
            explicit: false,
            baseline_page: 0,
        };
        let remote = Chapter {
            is_read: true,
            last_page_read: 20,
            ..Default::default()
        };
        assert_eq!(
            reconcile(&p, &remote),
            Reconcile::Apply {
                page: 20,
                read: true
            }
        );
    }
    #[test]
    fn concurrent_resume_positions_need_resolution() {
        let p = Progress {
            chapter_id: 1,
            page: 2,
            read: false,
            explicit: false,
            baseline_page: 0,
        };
        assert_eq!(
            reconcile(
                &p,
                &Chapter {
                    last_page_read: 5,
                    ..Default::default()
                }
            ),
            Reconcile::Conflict
        );
    }
}

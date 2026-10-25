use crate::domain::Chapter;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    pub deduplicate: bool,
    pub preferred_group: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            deduplicate: true,
            preferred_group: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Group {
    Automatic,
    Named(String),
}

impl fmt::Display for Group {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Automatic => f.write_str("Automatic (most chapters)"),
            Self::Named(name) if name.is_empty() => f.write_str("Unknown group"),
            Self::Named(name) => f.write_str(name),
        }
    }
}

fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

pub fn groups(chapters: &[Chapter]) -> Vec<Group> {
    let mut seen = HashSet::new();
    let mut groups: Vec<_> = chapters
        .iter()
        .filter_map(|c| {
            let name = c.scanlator.as_deref().unwrap_or("").trim();
            seen.insert(normalize(name))
                .then(|| Group::Named(name.to_owned()))
        })
        .collect();
    groups.sort_by_key(|g| g.to_string().to_lowercase());
    groups.insert(0, Group::Automatic);
    groups
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Key {
    Number(u64),
    Title(String),
    Id(i64),
}

fn key(chapter: &Chapter) -> Key {
    if chapter.chapter_number.is_finite() && chapter.chapter_number > 0.0 {
        Key::Number(chapter.chapter_number.to_bits())
    } else {
        // zero and negative numbers are also used for unnumbered extras.
        let title = normalize(&chapter.name);
        if title.is_empty() {
            Key::Id(chapter.id)
        } else {
            Key::Title(title)
        }
    }
}

pub fn chapters<'a>(all: &'a [Chapter], options: &Options) -> Vec<&'a Chapter> {
    if !options.deduplicate {
        return all.iter().collect();
    }

    let mut coverage: HashMap<String, HashSet<Key>> = HashMap::new();
    for c in all {
        coverage
            .entry(normalize(c.scanlator.as_deref().unwrap_or("")))
            .or_default()
            .insert(key(c));
    }
    let mut groups: Vec<_> = coverage.iter().collect();
    groups.sort_by(|(a, ac), (b, bc)| {
        bc.len()
            .cmp(&ac.len())
            .then_with(|| a.is_empty().cmp(&b.is_empty()))
            .then_with(|| a.cmp(b))
    });
    let ranks: HashMap<_, _> = groups
        .into_iter()
        .enumerate()
        .map(|(rank, (name, _))| (name.as_str(), rank))
        .collect();
    let preferred = options.preferred_group.as_deref().map(normalize);
    let score = |c: &Chapter| {
        let group = normalize(c.scanlator.as_deref().unwrap_or(""));
        (
            c.last_page_read > 0 && !c.is_read,
            c.is_read,
            preferred.as_ref() == Some(&group),
            std::cmp::Reverse(ranks[group.as_str()]),
            c.is_bookmarked,
            c.is_downloaded,
            std::cmp::Reverse(c.source_order),
            std::cmp::Reverse(c.id),
        )
    };
    let mut positions = HashMap::new();
    let mut selected: Vec<&Chapter> = Vec::new();
    for chapter in all {
        let position = positions.entry(key(chapter)).or_insert_with(|| {
            selected.push(chapter);
            selected.len() - 1
        });
        if score(chapter) > score(selected[*position]) {
            selected[*position] = chapter;
        }
    }

    // release groups can appear in separate blocks in source order.
    let mut numbered: Vec<_> = selected
        .iter()
        .copied()
        .filter(|c| matches!(key(c), Key::Number(_)))
        .collect();
    numbered.sort_by(|a, b| a.chapter_number.total_cmp(&b.chapter_number));
    let mut numbered = numbered.into_iter();
    for c in &mut selected {
        if matches!(key(c), Key::Number(_)) {
            *c = numbered.next().unwrap();
        }
    }
    selected
}

pub fn adjacent(all: &[Chapter], options: &Options, id: i64, delta: i32) -> Option<i64> {
    let current = all.iter().find(|c| c.id == id)?;
    let visible = chapters(all, options);
    let position = visible
        .iter()
        .position(|c| c.id == id || (options.deduplicate && key(c) == key(current)))?;
    let target = position.checked_add_signed(delta as isize)?;
    visible.get(target).map(|c| c.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chapter(id: i64, number: f64, group: &str) -> Chapter {
        Chapter {
            id,
            name: format!("Chapter {number}"),
            chapter_number: number,
            scanlator: Some(group.into()),
            source_order: id,
            ..Default::default()
        }
    }

    #[test]
    fn anything_okay_prefers_gamma_and_hides_repeated_lilymanga_releases() {
        let mut all = Vec::new();
        for n in 1..=40 {
            if n <= 28 {
                all.push(chapter(all.len() as i64 + 1, n as f64, "LilyManga"));
                if (6..=9).contains(&n) {
                    all.push(chapter(all.len() as i64 + 1, n as f64, "LilyManga"));
                }
            }
            all.push(chapter(all.len() as i64 + 1, n as f64, "Gamma"));
        }
        assert_eq!(all.len(), 72);
        let visible = chapters(&all, &Options::default());
        assert_eq!(visible.len(), 40);
        for (i, c) in visible.iter().enumerate() {
            assert_eq!(c.chapter_number, (i + 1) as f64);
            assert_eq!(c.scanlator.as_deref(), Some("Gamma"));
        }

        let options = Options {
            preferred_group: Some("LilyManga".into()),
            ..Default::default()
        };
        let visible = chapters(&all, &options);
        assert_eq!(visible.len(), 40);
        assert!(
            visible[..28]
                .iter()
                .all(|c| c.scanlator.as_deref() == Some("LilyManga"))
        );
        assert!(
            visible[28..]
                .iter()
                .all(|c| c.scanlator.as_deref() == Some("Gamma"))
        );
        let options = Options {
            deduplicate: false,
            ..options
        };
        assert_eq!(chapters(&all, &options).len(), 72);
    }

    #[test]
    fn coverage_counts_distinct_chapters_and_sorts_releases_from_separate_blocks() {
        let all = vec![
            chapter(1, 1.0, "A"),
            chapter(2, 1.0, "A"),
            chapter(3, 1.0, "A"),
            chapter(4, 4.0, "A"),
            chapter(5, 1.0, "B"),
            chapter(6, 2.0, "B"),
            chapter(7, 3.0, "B"),
        ];
        let visible = chapters(&all, &Options::default());
        assert_eq!(
            visible.iter().map(|c| c.id).collect::<Vec<_>>(),
            [5, 6, 7, 4]
        );
        let mut reversed = all.clone();
        reversed.reverse();
        assert_eq!(chapters(&reversed, &Options::default())[0].id, 5);
    }

    #[test]
    fn missing_gamma_chapter_uses_lilymanga_then_returns_to_gamma() {
        let mut all: Vec<_> = (1..=8)
            .filter(|n| *n != 6)
            .map(|n| chapter(n, n as f64, "Gamma"))
            .collect();
        all.push(chapter(15, 5.0, "LilyManga"));
        all.push(chapter(16, 6.0, "LilyManga"));
        let options = Options::default();
        let visible = chapters(&all, &options);
        assert_eq!(visible.len(), 8);
        assert_eq!(visible[4].scanlator.as_deref(), Some("Gamma"));
        assert_eq!(visible[5].scanlator.as_deref(), Some("LilyManga"));
        assert_eq!(visible[6].scanlator.as_deref(), Some("Gamma"));
        assert_eq!(adjacent(&all, &options, 5, 1), Some(16));
        assert_eq!(adjacent(&all, &options, 16, 1), Some(7));
    }

    #[test]
    fn fractional_chapters_and_distinct_unnumbered_extras_survive() {
        let mut all = vec![
            chapter(1, 1.0, "A"),
            chapter(2, 1.5, "A"),
            chapter(3, 1.0, "B"),
            chapter(4, 1.5, "B"),
            chapter(5, 0.0, "A"),
            chapter(6, 0.0, "A"),
            chapter(7, -1.0, "A"),
            chapter(8, -1.0, "A"),
        ];
        all[4].name = "Prologue".into();
        all[5].name = "Bonus".into();
        all[6].name = "Afterword".into();
        all[7].name.clear();
        assert_eq!(chapters(&all, &Options::default()).len(), 6);
    }

    #[test]
    fn read_and_in_progress_copies_survive_group_preferences() {
        let mut all = vec![
            chapter(1, 1.0, "A"),
            chapter(2, 1.0, "B"),
            chapter(3, 2.0, "A"),
            chapter(4, 2.0, "B"),
        ];
        all[1].is_read = true;
        all[3].last_page_read = 4;
        let options = Options {
            preferred_group: Some("A".into()),
            ..Default::default()
        };
        assert_eq!(
            chapters(&all, &options)
                .iter()
                .map(|c| c.id)
                .collect::<Vec<_>>(),
            [2, 4]
        );
        assert_eq!(all[3].last_page_read, 4);
    }

    #[test]
    fn navigation_skips_duplicate_releases_even_when_opened_from_history() {
        let all = vec![
            chapter(1, 1.0, "A"),
            chapter(2, 1.0, "B"),
            chapter(3, 2.0, "B"),
        ];
        let options = Options::default();
        assert_eq!(adjacent(&all, &options, 1, 1), Some(3));
        assert_eq!(adjacent(&all, &options, 3, -1), Some(2));
        assert_eq!(adjacent(&all, &options, 1, -1), None);
        assert_eq!(adjacent(&all, &options, 3, 1), None);
        let options = Options {
            deduplicate: false,
            ..options
        };
        assert_eq!(adjacent(&all, &options, 1, 1), Some(2));
    }

    #[test]
    fn group_names_are_normalized_and_unknown_groups_work() {
        let mut all = vec![
            chapter(1, 1.0, "  Gamma "),
            chapter(2, 2.0, "gamma"),
            chapter(3, 1.0, ""),
        ];
        all[2].scanlator = None;
        assert_eq!(groups(&all).len(), 3);
        let options = Options {
            preferred_group: Some("GAMMA".into()),
            ..Default::default()
        };
        assert_eq!(chapters(&all, &options)[0].id, 1);
        let options = Options {
            preferred_group: Some(String::new()),
            ..options
        };
        assert_eq!(chapters(&all, &options)[0].id, 3);
    }
}

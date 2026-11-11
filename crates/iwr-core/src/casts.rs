//! The cast map: the codecasts found around the served project, as a folder tree.
//!
//! `codecast serve` scans from the git root down (`GET /api/casts`) and the UI's map page draws
//! the result: one node per folder that holds a codecast, plus the folders between them, so the
//! viewer can jump to the codecast of a sub-folder or of a parent folder. Shared by the server
//! and the UI (serde); [`CastMap::layout`] places the nodes so the UI only draws.

use serde::{Deserialize, Serialize};

/// What the map shows about one codecast, read from its script without analyzing its project.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CastInfo {
    pub title: String,
    pub parts: usize,
    pub cues: usize,
    /// How long it lasts in seconds, guessed from the words.
    pub secs: f64,
    /// Languages it has recordings in (`en`, `ja`), sorted.
    pub langs: Vec<String>,
}

/// A folder of the map: one that holds a codecast, or one on the way to such a folder.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CastNode {
    /// Path from the top folder, `/`-separated; empty for the top itself. What
    /// `POST /api/root` takes.
    pub path: String,
    /// The folder's own name (the top folder's name for the top); `a/b/c` when the folders
    /// above it held nothing else and were folded into it.
    pub name: String,
    /// Index of the parent folder's node; `None` for the top.
    pub parent: Option<usize>,
    /// The folder's codecast; `None` for a folder that only connects others.
    pub cast: Option<CastInfo>,
    /// The folder `codecast serve` is showing now; for a topic, the one that plays.
    pub current: bool,
    /// Set on a topic of the folder's codecast (its directory inside `codecast/`): `path` is
    /// then the folder's, and `cast` what the topic holds.
    #[serde(default)]
    pub topic: Option<String>,
}

/// A topic: a codecast of its own about the same code, kept in a sub-folder of the `codecast/`
/// directory (`codecast/error_handling/`, same files inside as in `codecast/`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Topic {
    /// The sub-folder's name: what `POST /api/topic` takes.
    pub dir: String,
    /// What the viewer reads: the `# Title` of the topic's `index.md`, else [`topic_label`].
    pub label: String,
    pub info: CastInfo,
}

/// The topics of the served codecast, and which one plays (`GET /api/topics`, `topics.json` in
/// an export).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Topics {
    /// Is there a general codecast (parts right in `codecast/`, or a single-file script)?
    pub general: bool,
    /// Title of the general codecast, when it has one.
    pub title: String,
    /// The topic that plays, by its `dir`; empty for the general codecast.
    pub current: String,
    pub topics: Vec<Topic>,
}

/// A topic's label from its folder name: an ordering prefix (`02-`) dropped, `_` read as a
/// space (`02-error_handling` → `error handling`).
pub fn topic_label(dir: &str) -> String {
    let rest = match dir.split_once('-') {
        Some((n, rest)) if !n.is_empty() && !rest.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => rest,
        _ => dir,
    };
    rest.replace('_', " ").trim().to_string()
}

/// The folders with a codecast under the top folder (the git root, else the served folder).
/// Parents come before their children, siblings in name order.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CastMap {
    /// The top folder, as an absolute path.
    pub top: String,
    pub nodes: Vec<CastNode>,
    /// `true` when the scan stopped early (too many folders): some codecasts may be missing.
    pub partial: bool,
}

/// Where a node is drawn: its column (depth in the tree) and its row (may fall between two
/// rows: a parent sits in the middle of its children).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Place {
    pub col: usize,
    pub row: f64,
}

impl CastMap {
    /// Build the map from folder paths (relative to the top, `/`-separated, `""` for the top),
    /// each with its codecast if it has one. Missing folders in between are added, a chain of
    /// them (`crates/` › `core/` › `tests/`) as one node; `current` is always on the map, even
    /// without a codecast.
    pub fn build(top: &str, top_name: &str, folders: Vec<(String, Option<CastInfo>)>, current: &str) -> CastMap {
        Self::build_with_topics(top, top_name, folders, vec![], current, "")
    }

    /// [`CastMap::build`], with the topics of the folders that have some (folder path, its
    /// topics): each topic is a node under its folder, before the sub-folders. `current_topic`
    /// is the topic that plays in the `current` folder (empty: its general codecast).
    pub fn build_with_topics(top: &str, top_name: &str, folders: Vec<(String, Option<CastInfo>)>, topics: Vec<(String, Vec<Topic>)>, current: &str, current_topic: &str) -> CastMap {
        let mut paths: Vec<(String, Option<CastInfo>)> = folders;
        if !paths.iter().any(|(p, _)| p == current) {
            paths.push((current.to_string(), None));
        }
        // a folder with topics only (no general codecast) is on the map too
        for (p, _) in &topics {
            if !paths.iter().any(|(q, _)| q == p) {
                paths.push((p.clone(), None));
            }
        }
        // every ancestor of a listed folder is a node too, the top included
        let mut all: std::collections::BTreeMap<Vec<String>, Option<CastInfo>> = Default::default();
        all.insert(vec![], None);
        for (p, info) in paths {
            let segs: Vec<String> = p.split('/').filter(|s| !s.is_empty()).map(str::to_string).collect();
            for n in 0..segs.len() {
                all.entry(segs[..n].to_vec()).or_insert(None);
            }
            let slot = all.entry(segs).or_insert(None);
            if info.is_some() {
                *slot = info;
            }
        }
        // sorted by segments: a parent comes right before its subtree
        let keys: Vec<&Vec<String>> = all.keys().collect();
        let nodes: Vec<CastNode> = all
            .iter()
            .map(|(segs, info)| {
                let path = segs.join("/");
                CastNode {
                    name: segs.last().cloned().unwrap_or_else(|| top_name.to_string()),
                    parent: segs.split_last().and_then(|(_, up)| keys.iter().position(|k| k.as_slice() == up)),
                    cast: info.clone(),
                    current: path == current,
                    path,
                    topic: None,
                }
            })
            .collect();
        // topics go right after their folder: the subtree stays in one run
        let at: Vec<usize> = nodes
            .iter()
            .scan(0, |next, n| {
                let i = *next;
                *next += 1 + topics.iter().filter(|(p, _)| *p == n.path).map(|(_, t)| t.len()).sum::<usize>();
                Some(i)
            })
            .collect();
        let mut all = Vec::new();
        for (i, mut n) in nodes.into_iter().enumerate() {
            n.parent = n.parent.map(|p| at[p]);
            let (path, here) = (n.path.clone(), n.current);
            all.push(n);
            for t in topics.iter().filter(|(p, _)| *p == path).flat_map(|(_, t)| t) {
                all.push(CastNode { path: path.clone(), name: t.label.clone(), parent: Some(at[i]), cast: Some(t.info.clone()), current: here && t.dir == current_topic, topic: Some(t.dir.clone()) });
            }
        }
        CastMap { top: top.to_string(), nodes: fold_ways(all), partial: false }
    }

    /// Is a topic the thing that plays (then the served folder's own box is a place to go)?
    pub fn topic_plays(&self) -> bool {
        self.nodes.iter().any(|n| n.topic.is_some() && n.current)
    }

    /// How many codecasts the map has.
    pub fn casts(&self) -> usize {
        self.nodes.iter().filter(|n| n.cast.is_some()).count()
    }

    /// Depth of a node: `0` for the top.
    pub fn depth(&self, i: usize) -> usize {
        let mut d = 0;
        let mut at = i;
        while let Some(p) = self.nodes[at].parent {
            d += 1;
            at = p;
        }
        d
    }

    /// A tidy tree read left to right: one column per depth, each leaf on its own row, a parent
    /// in the middle of its children. One [`Place`] per node, in node order.
    pub fn layout(&self) -> Vec<Place> {
        let n = self.nodes.len();
        let mut kids: Vec<Vec<usize>> = vec![vec![]; n];
        for (i, node) in self.nodes.iter().enumerate() {
            if let Some(p) = node.parent {
                kids[p].push(i);
            }
        }
        let mut places = vec![Place::default(); n];
        let mut next_row = 0.0;
        // children come after their parent in `nodes`, so walking backwards sees them first;
        // leaves take their rows in a forward pass to keep name order top to bottom
        for i in 0..n {
            places[i].col = self.depth(i);
            if kids[i].is_empty() {
                places[i].row = next_row;
                next_row += 1.0;
            }
        }
        for i in (0..n).rev() {
            if let (Some(a), Some(b)) = (kids[i].first(), kids[i].last()) {
                places[i].row = (places[*a].row + places[*b].row) / 2.0;
            }
        }
        places
    }
}

/// Fold every folder that only leads to one other such folder into it: the map shows
/// `crates/core/tests/` as one box instead of three in a row. The top is never folded.
fn fold_ways(nodes: Vec<CastNode>) -> Vec<CastNode> {
    let way = |n: &CastNode| n.cast.is_none() && !n.current;
    let kids = |i: usize| nodes.iter().enumerate().filter(move |(_, n)| n.parent == Some(i)).map(|(k, _)| k);
    // a folded node's only child, which takes its name in front of its own
    let into: Vec<Option<usize>> = (0..nodes.len())
        .map(|i| {
            let only = kids(i).next().filter(|_| kids(i).count() == 1)?;
            (nodes[i].parent.is_some() && way(&nodes[i]) && way(&nodes[only])).then_some(only)
        })
        .collect();
    // where each kept node lands once the folded ones are gone
    let mut new_idx = vec![0; nodes.len()];
    let mut kept = 0;
    for i in 0..nodes.len() {
        new_idx[i] = kept;
        if into[i].is_none() {
            kept += 1;
        }
    }
    let mut out = Vec::with_capacity(kept);
    for (i, n) in nodes.iter().enumerate() {
        if into[i].is_some() {
            continue;
        }
        // walk up through the folded folders: their names go in front, their parent is ours
        let mut n = n.clone();
        let mut up = n.parent;
        while let Some(p) = up.filter(|p| into[*p].is_some()) {
            n.name = format!("{}/{}", nodes[p].name, n.name);
            up = nodes[p].parent;
        }
        n.parent = up.map(|p| new_idx[p]);
        out.push(n);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cast(title: &str) -> Option<CastInfo> {
        Some(CastInfo { title: title.into(), ..Default::default() })
    }

    #[test]
    fn build_adds_the_folders_in_between() {
        let m = CastMap::build("/w", "w", vec![("".into(), cast("All")), ("crates/core".into(), cast("Core")), ("crates/ui".into(), cast("Ui"))], "crates/core");
        let paths: Vec<&str> = m.nodes.iter().map(|n| n.path.as_str()).collect();
        assert_eq!(paths, ["", "crates", "crates/core", "crates/ui"]);
        assert_eq!(m.nodes[0].name, "w");
        assert_eq!(m.nodes[1].cast, None);
        assert_eq!(m.nodes[2].parent, Some(1));
        assert!(m.nodes[2].current && !m.nodes[3].current);
        assert_eq!(m.casts(), 3);
    }

    #[test]
    fn current_folder_without_a_cast_is_on_the_map() {
        let m = CastMap::build("/w", "w", vec![("".into(), cast("All"))], "tools/x");
        assert_eq!(m.nodes.len(), 3);
        assert!(m.nodes[2].current && m.nodes[2].cast.is_none());
    }

    #[test]
    fn a_chain_of_empty_folders_is_one_node() {
        let m = CastMap::build("/w", "w", vec![("".into(), cast("All")), ("a/b/c/x".into(), cast("X")), ("a/b/c/y".into(), cast("Y"))], "");
        let names: Vec<&str> = m.nodes.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, ["w", "a/b/c", "x", "y"]);
        assert_eq!(m.nodes[1].path, "a/b/c");
        assert_eq!(m.nodes.iter().map(|n| n.parent).collect::<Vec<_>>(), [None, Some(0), Some(1), Some(1)]);
    }

    #[test]
    fn topics_are_nodes_under_their_folder() {
        let topic = |dir: &str| Topic { dir: dir.into(), label: topic_label(dir), info: CastInfo::default() };
        let m = CastMap::build_with_topics("/w", "w", vec![("".into(), cast("All")), ("a/b".into(), None), ("a/b/c".into(), cast("C"))], vec![("".into(), vec![topic("01-error_handling"), topic("io")]), ("a/b".into(), vec![topic("only")])], "", "io");
        let names: Vec<&str> = m.nodes.iter().map(|n| n.name.as_str()).collect();
        // `a` is folded into `b`; `b` has a topic, so it is not folded into `c`
        assert_eq!(names, ["w", "error handling", "io", "a/b", "only", "c"]);
        assert_eq!(m.nodes.iter().map(|n| n.parent).collect::<Vec<_>>(), [None, Some(0), Some(0), Some(0), Some(3), Some(3)]);
        assert_eq!(m.nodes[2].topic.as_deref(), Some("io"));
        assert!(m.nodes[0].current && m.nodes[2].current && !m.nodes[1].current && !m.nodes[4].current);
        assert!(m.topic_plays());
        assert_eq!(m.nodes[4].path, "a/b");
    }

    #[test]
    fn topic_labels_come_from_folder_names() {
        assert_eq!(topic_label("error_handling"), "error handling");
        assert_eq!(topic_label("02-the_parser"), "the parser");
        assert_eq!(topic_label("x-ray"), "x-ray");
        assert_eq!(topic_label("2024"), "2024");
    }

    #[test]
    fn layout_centers_parents_over_children() {
        let m = CastMap::build("/w", "w", vec![("a/x".into(), cast("X")), ("a/y".into(), cast("Y")), ("b".into(), cast("B"))], "");
        // "", a, a/x, a/y, b
        let p = m.layout();
        assert_eq!(p.iter().map(|p| p.col).collect::<Vec<_>>(), [0, 1, 2, 2, 1]);
        assert_eq!(p[2].row, 0.0);
        assert_eq!(p[3].row, 1.0);
        assert_eq!(p[4].row, 2.0);
        assert_eq!(p[1].row, 0.5);
        assert_eq!(p[0].row, 1.25);
    }
}

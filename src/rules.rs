//! Window rules: settings for windows of certain programs, applied when a window first shows
//! its picture. In the configuration file:
//!
//! ```toml
//!     [[rules]]
//!     app_id = "pavucontrol"        # the program's app id (X11: its class); * and ? match anything
//!     float = true                  # stays out of the tiling layout
//!     size = [700, 500]             # the program's own size (without the frame)
//!     center = true                 # in the middle of the monitor
//!
//!     [[rules]]
//!     title = "*Picture-in-Picture*"   # the title when the window first shows
//!     on_top = true
//!     workspace = 3                    # 1 to 32
//! ```
//!
//! A rule needs `app_id` or `title` (or both: then both must match). All rules that match are
//! applied, in order; for the same setting the later one wins.

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct WindowRule {
    pub app_id: String,
    pub title: String,
    /// stays out of the tiling layout (or, with false, is always tiled)
    pub float: Option<bool>,
    /// opens on this workspace (1-based)
    pub workspace: Option<usize>,
    pub maximize: Option<bool>,
    pub on_top: Option<bool>,
    /// the program's size in pixels (width, height), without the frame
    pub size: Option<[i32; 2]>,
    /// the top left corner of the window with its frame, on the first monitor
    pub position: Option<[i32; 2]>,
    /// in the middle of the monitor it is on
    pub center: Option<bool>,
}

/// Does `text` match `pattern`? `*` is any run of characters, `?` any one; case does not matter.
pub fn glob(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut mark) = (None, 0usize);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

impl WindowRule {
    pub fn matches(&self, app_id: &str, title: &str) -> bool {
        if self.app_id.is_empty() && self.title.is_empty() {
            return false;
        }
        (self.app_id.is_empty() || glob(&self.app_id, app_id)) && (self.title.is_empty() || glob(&self.title, title))
    }

    /// What is wrong with this rule, if anything.
    pub fn check(&self) -> Result<(), String> {
        if self.app_id.is_empty() && self.title.is_empty() {
            return Err("needs an app_id or a title to match".into());
        }
        if self.app_id.len() > 200 || self.title.len() > 200 {
            return Err("app_id and title are at most 200 characters".into());
        }
        if self.workspace.is_some_and(|n| !(1..=32).contains(&n)) {
            return Err("workspace is a number from 1 to 32".into());
        }
        if self.size.is_some_and(|[w, h]| !(50..=16384).contains(&w) || !(50..=16384).contains(&h)) {
            return Err("size is [width, height], each from 50 to 16384".into());
        }
        if self.position.is_some_and(|[x, y]| x.abs() > 100_000 || y.abs() > 100_000) {
            return Err("position is [x, y] on the monitor".into());
        }
        Ok(())
    }
}

/// The rules of `rules` that apply to a window, in order.
pub fn matching<'a>(rules: &'a [WindowRule], app_id: &str, title: &str) -> Vec<&'a WindowRule> {
    rules.iter().filter(|r| r.matches(app_id, title)).collect()
}

/// What all the matching rules say together (the later rule wins for a setting).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Outcome {
    pub float: Option<bool>,
    pub workspace: Option<usize>,
    pub maximize: Option<bool>,
    pub on_top: Option<bool>,
    pub size: Option<[i32; 2]>,
    pub position: Option<[i32; 2]>,
    pub center: Option<bool>,
}

pub fn combine(rules: &[&WindowRule]) -> Outcome {
    let mut o = Outcome::default();
    for r in rules {
        o.float = r.float.or(o.float);
        o.workspace = r.workspace.or(o.workspace);
        o.maximize = r.maximize.or(o.maximize);
        o.on_top = r.on_top.or(o.on_top);
        o.size = r.size.or(o.size);
        // a position and centring cancel each other: the later one counts
        if r.position.is_some() {
            o.position = r.position;
            o.center = None;
        }
        if r.center.is_some() {
            o.center = r.center;
            if r.center == Some(true) {
                o.position = None;
            }
        }
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(app: &str, title: &str) -> WindowRule {
        WindowRule { app_id: app.into(), title: title.into(), ..Default::default() }
    }

    #[test]
    fn patterns_match_with_stars_and_question_marks_and_ignore_case() {
        assert!(glob("firefox", "Firefox"));
        assert!(glob("org.gnome.*", "org.gnome.Nautilus"));
        assert!(glob("*Picture-in-Picture*", "Firefox — Picture-in-Picture"));
        assert!(glob("a?c", "abc") && !glob("a?c", "ac") && !glob("a?c", "abbc"));
        assert!(glob("*", "") && glob("*", "anything"));
        assert!(glob("a*b*c", "a--b--c") && !glob("a*b*c", "a--c"));
        assert!(!glob("firefox", "firefox-esr"));
        assert!(glob("**x", "x"));
        assert!(!glob("", "x") && glob("", ""));
    }

    #[test]
    fn a_rule_needs_something_to_match_and_both_when_it_has_both() {
        assert!(rule("pavucontrol", "").matches("pavucontrol", "Volume Control"));
        assert!(!rule("pavucontrol", "").matches("other", "Volume Control"));
        assert!(rule("", "Volume*").matches("anything", "Volume Control"));
        assert!(rule("pavu*", "Volume*").matches("pavucontrol", "Volume Control"));
        assert!(!rule("pavu*", "Mixer*").matches("pavucontrol", "Volume Control"), "both must match");
        assert!(!rule("", "").matches("x", "y"), "a rule that names nothing matches nothing");
    }

    #[test]
    fn bad_rules_are_described() {
        assert!(rule("", "").check().unwrap_err().contains("app_id"));
        assert!(WindowRule { workspace: Some(0), ..rule("a", "") }.check().unwrap_err().contains("workspace"));
        assert!(WindowRule { workspace: Some(33), ..rule("a", "") }.check().is_err());
        assert!(WindowRule { size: Some([10, 400]), ..rule("a", "") }.check().unwrap_err().contains("size"));
        assert!(WindowRule { size: Some([800, 99999]), ..rule("a", "") }.check().is_err());
        assert!(WindowRule { size: Some([800, 600]), workspace: Some(3), ..rule("a", "") }.check().is_ok());
        assert!(rule(&"a".repeat(300), "").check().is_err());
    }

    #[test]
    fn matching_rules_combine_and_the_later_one_wins() {
        let rules = vec![
            WindowRule { float: Some(true), size: Some([500, 400]), center: Some(true), ..rule("app*", "") },
            WindowRule { size: Some([700, 500]), workspace: Some(2), ..rule("app1", "") },
            WindowRule { float: Some(false), position: Some([10, 20]), ..rule("app1", "Main*") },
            rule("unrelated", ""),
        ];
        let m = matching(&rules, "app1", "Main window");
        assert_eq!(m.len(), 3);
        let o = combine(&m);
        assert_eq!(o.float, Some(false), "the later rule says tiled");
        assert_eq!(o.size, Some([700, 500]));
        assert_eq!(o.workspace, Some(2));
        assert_eq!((o.position, o.center), (Some([10, 20]), None), "a later position replaces centring");
        let only = combine(&matching(&rules, "app2", "x"));
        assert_eq!((only.float, only.size, only.center, only.workspace), (Some(true), Some([500, 400]), Some(true), None));
        assert_eq!(combine(&matching(&rules, "zzz", "x")), Outcome::default());
    }

    #[test]
    fn rules_are_read_from_toml() {
        let text = "[[rules]]\napp_id = \"pavucontrol\"\nfloat = true\nsize = [700, 500]\n\n[[rules]]\ntitle = \"*PiP*\"\non_top = true\nworkspace = 3\n";
        #[derive(Deserialize)]
        struct File {
            rules: Vec<WindowRule>,
        }
        let f: File = toml::from_str(text).unwrap();
        assert_eq!(f.rules.len(), 2);
        assert_eq!(f.rules[0].size, Some([700, 500]));
        assert_eq!((f.rules[1].on_top, f.rules[1].workspace), (Some(true), Some(3)));
    }
}

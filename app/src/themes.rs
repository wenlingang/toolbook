//! Themes: the 13 built in, the user's in `~/Toolbook/.themes/`, and the
//! syntax colors each one gives the Markdown view.

use std::cell::RefCell;

use nsh_base::Theme;
use nsh_core::element::{AdaptiveColor, set_tokens};
use nsh_core::prelude::*;
use toolbook_core::engine::RunOptions;

/// The built-in themes: id, name, dark.
pub const BUILTIN: &[(&str, &str, bool)] = &[
    ("toolbook-deep", "Toolbook 深空", true),
    ("toolbook-lab", "Toolbook 实验室", false),
    ("one-dark", "One Dark", true),
    ("dracula", "Dracula", true),
    ("catppuccin-mocha", "Catppuccin Mocha", true),
    ("catppuccin-latte", "Catppuccin Latte", false),
    ("tokyo-night", "Tokyo Night", true),
    ("nord", "Nord", true),
    ("gruvbox-dark", "Gruvbox Dark", true),
    ("rose-pine", "Rosé Pine", true),
    ("github-dark", "GitHub Dark", true),
    ("github-light", "GitHub Light", false),
    ("solarized-light", "Solarized Light", false),
];

fn builtin_json(id: &str) -> Option<&'static str> {
    Some(match id {
        "toolbook-deep" => include_str!("../themes/toolbook-deep.json"),
        "toolbook-lab" => include_str!("../themes/toolbook-lab.json"),
        "one-dark" => include_str!("../themes/one-dark.json"),
        "dracula" => include_str!("../themes/dracula.json"),
        "catppuccin-mocha" => include_str!("../themes/catppuccin-mocha.json"),
        "catppuccin-latte" => include_str!("../themes/catppuccin-latte.json"),
        "tokyo-night" => include_str!("../themes/tokyo-night.json"),
        "nord" => include_str!("../themes/nord.json"),
        "gruvbox-dark" => include_str!("../themes/gruvbox-dark.json"),
        "rose-pine" => include_str!("../themes/rose-pine.json"),
        "github-dark" => include_str!("../themes/github-dark.json"),
        "github-light" => include_str!("../themes/github-light.json"),
        "solarized-light" => include_str!("../themes/solarized-light.json"),
        _ => return None,
    })
}

/// A built-in theme by id.
pub fn builtin_theme(id: &str) -> Result<Theme, String> {
    let json = builtin_json(id).ok_or_else(|| format!("没有主题 {id}"))?;
    Theme::from_json(json).map_err(|e| e.0)
}

/// What the Markdown view colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxKind {
    /// Headings and Markdown marks.
    Keyword,
    /// Literal values.
    String,
    /// Numbers.
    Number,
    /// `$name` and `{{ name }}`.
    Variable,
    /// Fences and quiet parts.
    Comment,
    /// A tool's name after the fence.
    Tool,
    /// `key=` in a fence.
    Attr,
}

impl SyntaxKind {
    /// Every kind.
    pub const ALL: &'static [SyntaxKind] = &[
        Self::Keyword,
        Self::String,
        Self::Number,
        Self::Variable,
        Self::Comment,
        Self::Tool,
        Self::Attr,
    ];

    fn token(self) -> &'static str {
        match self {
            Self::Keyword => "syntax-keyword",
            Self::String => "syntax-string",
            Self::Number => "syntax-number",
            Self::Variable => "syntax-variable",
            Self::Comment => "syntax-comment",
            Self::Tool => "syntax-tool",
            Self::Attr => "syntax-attr",
        }
    }
}

/// The color of `kind` in `theme`; a theme without syntax colors gets
/// them from its main colors.
pub fn syntax(theme: &Theme, kind: SyntaxKind) -> AdaptiveColor {
    theme.color(kind.token()).unwrap_or(match kind {
        SyntaxKind::Keyword | SyntaxKind::Tool => theme.primary,
        SyntaxKind::String => theme.chart_2,
        SyntaxKind::Number => theme.chart_3,
        SyntaxKind::Variable => theme.chart_1,
        SyntaxKind::Comment | SyntaxKind::Attr => theme.muted_foreground,
    })
}

/// [`syntax`] as `#rrggbb`, in the dark or the light appearance.
pub fn syntax_hex(theme: &Theme, kind: SyntaxKind, dark: bool) -> String {
    let c = syntax(theme, kind);
    let rgba = Rgba::from(if dark { c.dark } else { c.light });
    let to = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", to(rgba.r), to(rgba.g), to(rgba.b))
}

thread_local! {
    static CURRENT: RefCell<String> = RefCell::new("toolbook-deep".into());
}

/// A theme as the theme menu lists it.
#[derive(Debug, Clone, PartialEq, JsObject)]
pub struct ThemeInfo {
    /// Its id (`dracula`, or `custom:<file>` for the user's).
    pub id: String,
    /// Its name.
    pub name: String,
    /// Whether it is dark.
    pub dark: bool,
}

fn custom_dir() -> std::path::PathBuf {
    RunOptions::from_process(false).library.join(".themes")
}

/// The app's themes.
#[module]
pub struct Themes;

#[module]
impl Themes {
    /// The built-in themes, then the user's from `~/Toolbook/.themes/`.
    pub fn list() -> Vec<ThemeInfo> {
        let mut out: Vec<ThemeInfo> = BUILTIN
            .iter()
            .map(|(id, name, dark)| ThemeInfo {
                id: (*id).into(),
                name: (*name).into(),
                dark: *dark,
            })
            .collect();
        if let Ok(entries) = std::fs::read_dir(custom_dir()) {
            let mut files: Vec<_> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "json"))
                .collect();
            files.sort();
            for p in files {
                let stem = p
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                out.push(ThemeInfo {
                    id: format!("custom:{stem}"),
                    name: stem,
                    dark: true,
                });
            }
        }
        out
    }

    /// Draws the app with theme `id`; why not, when it cannot.
    pub async fn apply(id: String) -> Result<(), String> {
        let theme = match id.strip_prefix("custom:") {
            Some(stem) => {
                let path = custom_dir().join(format!("{stem}.json"));
                let json = std::fs::read_to_string(&path)
                    .map_err(|e| format!("读取 {} 失败：{e}", path.display()))?;
                Theme::from_json(&json).map_err(|e| e.0)?
            }
            None => builtin_theme(&id)?,
        };
        set_tokens(theme.into_tokens());
        CURRENT.with_borrow_mut(|c| *c = id);
        Ok(())
    }

    /// The theme in use.
    pub fn current() -> String {
        CURRENT.with_borrow(|c| c.clone())
    }
}

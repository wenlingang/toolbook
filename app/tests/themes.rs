use toolbook_app::themes::{BUILTIN, SyntaxKind, builtin_theme, syntax_hex};

#[test]
fn all_builtin_themes_parse() {
    assert_eq!(BUILTIN.len(), 13);
    for (id, _, _) in BUILTIN {
        let theme = builtin_theme(id).unwrap_or_else(|e| panic!("{id}: {e}"));
        for kind in SyntaxKind::ALL {
            assert!(
                syntax_hex(&theme, *kind, false).starts_with('#'),
                "{id} {kind:?}"
            );
        }
    }
}

#[test]
fn syntax_colors_follow_theme() {
    let dracula = builtin_theme("dracula").unwrap();
    assert_eq!(syntax_hex(&dracula, SyntaxKind::Variable, true), "#ffb86c");
    let deep = builtin_theme("toolbook-deep").unwrap();
    assert_eq!(syntax_hex(&deep, SyntaxKind::Variable, true), "#ffb547");
    assert_eq!(syntax_hex(&deep, SyntaxKind::Tool, true), "#0fd3c3");
}

#[test]
fn a_custom_theme_without_syntax_colors_gets_them_as_tokens() {
    use toolbook_app::themes::with_syntax_colors;
    // A plain shadcn theme: no syntax-* entries.
    let json = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/themes/dracula.json"))
        .unwrap()
        .lines()
        .filter(|l| !l.contains("syntax-"))
        .collect::<Vec<_>>()
        .join("\n");
    let plain = nsh_base::Theme::from_json(&json).unwrap();
    assert!(plain.color("syntax-variable").is_none());
    let filled = with_syntax_colors(plain.clone());
    for kind in SyntaxKind::ALL {
        let token = format!("syntax-{kind:?}").to_lowercase();
        assert!(filled.color(&token).is_some(), "{token}");
        assert_eq!(syntax_hex(&filled, *kind, true), syntax_hex(&plain, *kind, true), "{token}");
    }
}

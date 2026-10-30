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

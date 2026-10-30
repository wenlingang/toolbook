use toolbook_app::highlight::{Span, spans};

fn find<'a>(all: &'a [Span], class: &str) -> Vec<&'a Span> {
    all.iter().filter(|s| s.class == class).collect()
}

#[test]
fn highlight_marks_vars_and_tools() {
    let src = "# 标题\n\n说明 $x 不算\n\n```pick name=ts input=$jwt.payload.exp path=\"a b\"\n内容\n```\n";
    let all = spans(src);
    let heading = find(&all, "text-syntax-keyword");
    assert_eq!(
        (heading[0].line, heading[0].start, heading[0].end),
        (0, 0, 4)
    );
    let tools = find(&all, "text-syntax-tool");
    assert_eq!((tools[0].line, tools[0].start, tools[0].end), (4, 3, 7));
    let vars = find(&all, "text-syntax-variable");
    // `$jwt.payload.exp` on the fence line only, not `$x` in prose.
    assert_eq!(vars.len(), 1);
    assert_eq!(vars[0].line, 4);
    let attrs = find(&all, "text-syntax-attr");
    assert_eq!(attrs.len(), 3, "name= input= path=");
    assert!(find(&all, "text-syntax-string").iter().any(|s| s.line == 4));
    // Fences are quiet.
    assert!(
        find(&all, "text-syntax-comment")
            .iter()
            .any(|s| s.line == 6 && s.start == 0 && s.end == 3)
    );
}

#[test]
fn template_refs_in_http_content() {
    let src = "```http name=r\nGET https://x/{{ id }}\n```\n";
    let vars: Vec<_> = spans(src)
        .into_iter()
        .filter(|s| s.class == "text-syntax-variable")
        .collect();
    assert_eq!((vars[0].line, vars[0].start, vars[0].end), (1, 14, 22));
}

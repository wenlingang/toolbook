use toolbook_app::bridge::{block_is_tool, schema, to_editor_json, to_markdown};
use toolbook_core::doc::Document;

fn examples() -> Vec<String> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples");
    std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
        .map(|e| std::fs::read_to_string(e.path()).unwrap())
        .collect()
}

#[test]
fn roundtrip_examples_through_editor() {
    let schema = schema();
    for src in examples() {
        let doc = Document::parse(&src);
        let node = schema
            .node_from_json(&to_editor_json(&doc, &schema))
            .unwrap();
        let md = to_markdown(doc.front_matter.as_deref(), &node);
        let back = Document::parse(&md);
        assert_eq!(back.is_toolbook, doc.is_toolbook);
        let a: Vec<_> = doc
            .blocks()
            .map(|(_, b)| (b.info.clone(), b.content.clone()))
            .collect();
        let b: Vec<_> = back
            .blocks()
            .map(|(_, b)| (b.info.clone(), b.content.clone()))
            .collect();
        assert_eq!(a, b, "{md}");
        assert_eq!(back.description(), doc.description());
        // Written once more, it stays the same.
        let node2 = schema
            .node_from_json(&to_editor_json(&back, &schema))
            .unwrap();
        assert_eq!(to_markdown(back.front_matter.as_deref(), &node2), md);
    }
}

#[test]
fn plain_markdown_keeps_code_blocks_static() {
    let schema = schema();
    let doc = Document::parse("# T\n\n```json\n{}\n```\n");
    let json = to_editor_json(&doc, &schema);
    let lang = json["content"][1]["attrs"]["language"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(lang, "json");
    assert!(!block_is_tool(&lang, false));
    assert!(block_is_tool(&lang, true));
    assert!(block_is_tool("json name=x", false));
}

const MIXED: &str = "---\ntoolbook: 1\n---\n\n# 标题\n\n普通段落。\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n<div>html</div>\n\nfoot[^1]\n\n[^1]: note\n\n```base64 name=b\nSGk=\n```\n\n- 列表一\n- 列表二\n";

#[test]
fn unsupported_markdown_is_kept_verbatim() {
    let schema = schema();
    let doc = Document::parse(MIXED);
    let node = schema
        .node_from_json(&to_editor_json(&doc, &schema))
        .unwrap();
    let md = to_markdown(doc.front_matter.as_deref(), &node);
    for part in [
        "| a | b |\n|---|---|\n| 1 | 2 |",
        "<div>html</div>",
        "foot[^1]",
        "[^1]: note",
        "# 标题",
        "普通段落。",
        "- 列表一\n- 列表二",
    ] {
        assert!(md.contains(part), "lost {part:?} in:\n{md}");
    }
    assert!(md.contains("```base64 name=b\nSGk=\n```"));
}

#[test]
fn new_note_template_roundtrips() {
    let src = include_str!("../src/new_note.md").replace("{{name}}", "新笔记");
    let schema = schema();
    let doc = Document::parse(&src);
    let node = schema
        .node_from_json(&to_editor_json(&doc, &schema))
        .unwrap();
    let md = to_markdown(doc.front_matter.as_deref(), &node);
    assert_eq!(md.trim(), src.trim(), "a new note must save back as it was");
}

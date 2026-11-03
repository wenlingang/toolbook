use toolbook_core::doc::Document;
use toolbook_core::fence::{Attr, PathSeg, parse_info};

const SAMPLE: &str = "---\ntoolbook: 1\n---\n\n# 标题\n\n说明文字。\n\n```text name=header param\nAuthorization: Bearer x.y.z\n```\n\n```jwt-decode name=jwt input=$header\n```\n\n结尾。\n";

#[test]
fn roundtrip_is_byte_identical() {
    let mut samples = vec![
        SAMPLE.to_owned(),
        SAMPLE.replace('\n', "\r\n"),
        "no newline at end ```".to_owned(),
        String::new(),
    ];
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    if let Ok(dir) = std::fs::read_dir(root) {
        for entry in dir.flatten() {
            if entry.path().extension().is_some_and(|e| e == "md") {
                samples.push(std::fs::read_to_string(entry.path()).unwrap());
            }
        }
    }
    for s in samples {
        assert_eq!(Document::parse(&s).serialize(), s);
    }
}

#[test]
fn crlf_sample_still_finds_blocks() {
    let doc = Document::parse(&SAMPLE.replace('\n', "\r\n"));
    assert_eq!(doc.blocks().count(), 2);
    let (_, b) = doc.blocks().next().unwrap();
    assert_eq!(b.content, "Authorization: Bearer x.y.z");
}

#[test]
fn parses_fence_info() {
    let info = parse_info(r#"pick name=ts input=$jwt.payload.exp path="a b" param"#);
    assert_eq!(info.tool, "pick");
    assert_eq!(info.name.as_deref(), Some("ts"));
    assert_eq!(info.flags, vec!["param".to_owned()]);
    match &info.attrs[0] {
        (k, Attr::Ref(r)) => {
            assert_eq!(k, "input");
            assert_eq!(r.name, "jwt");
            assert_eq!(r.path, vec![PathSeg::Key("payload".into()), PathSeg::Key("exp".into())]);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(info.attrs[1], ("path".to_owned(), Attr::Lit("a b".into())));
}

#[test]
fn plain_md_json_fence_is_not_a_tool() {
    assert_eq!(Document::parse("```json\n{}\n```\n").blocks().count(), 0);
    let marked = "---\ntoolbook: 1\n---\n```json\n{}\n```\n";
    assert_eq!(Document::parse(marked).blocks().count(), 1);
    assert_eq!(Document::parse("```json name=x\n{}\n```\n").blocks().count(), 1);
    let static_block = "---\ntoolbook: 1\n---\n```json static\n{}\n```\n";
    assert_eq!(Document::parse(static_block).blocks().count(), 0);
}

#[test]
fn nested_and_unclosed_fences_roundtrip() {
    let nested = "---\ntoolbook: 1\n---\n````md\n```json\n{}\n```\n````\n";
    let doc = Document::parse(nested);
    assert_eq!(doc.blocks().count(), 0);
    assert_eq!(doc.serialize(), nested);
    let unclosed = "```json name=a\n{";
    let doc = Document::parse(unclosed);
    assert_eq!(doc.blocks().count(), 0);
    assert_eq!(doc.serialize(), unclosed);
    let tilde = "~~~json name=a\n{}\n~~~\n";
    assert_eq!(Document::parse(tilde).blocks().count(), 1);
}

#[test]
fn set_block_content_then_reserialize_is_stable() {
    let mut doc = Document::parse(SAMPLE);
    doc.set_block_content(0, "Authorization: Bearer a.b.c");
    let once = doc.serialize();
    assert!(once.contains("Bearer a.b.c\n```"));
    assert!(once.contains("```jwt-decode name=jwt input=$header\n```"));
    let twice = Document::parse(&once).serialize();
    assert_eq!(once, twice);
}

#[test]
fn description_is_first_paragraph() {
    assert_eq!(Document::parse(SAMPLE).description().as_deref(), Some("说明文字。"));
}

#[test]
fn front_matter_marker_tolerates_spacing_quotes_and_comments() {
    for line in ["toolbook: 1", "toolbook:1", "toolbook:  1  # Toolbook 笔记", "toolbook: true", "toolbook: \"1\"", "  toolbook: 1"] {
        let doc = Document::parse(&format!("---\ntitle: x\n{line}\n---\n\n正文\n"));
        assert!(doc.is_toolbook, "{line:?}");
    }
    for line in ["toolbook: 0", "toolbook: false", "toolbooks: 1", "# toolbook: 1", "toolbook: 12"] {
        let doc = Document::parse(&format!("---\n{line}\n---\n\n正文\n"));
        assert!(!doc.is_toolbook, "{line:?}");
    }
}

#[test]
fn block_in_an_indented_list_item_drops_the_fence_indent() {
    let src = "---\ntoolbook: 1\n---\n\n1. 第一步\n\n   ```text name=a\n   hello\n     world\n\n   ```\n2. 下一步\n";
    let mut doc = Document::parse(src);
    assert_eq!(doc.serialize(), src);
    let (_, b) = doc.blocks().next().unwrap();
    assert_eq!(b.content, "hello\n  world\n");
    doc.set_block_content(0, "x\n\n  y\n```");
    assert_eq!(
        doc.serialize(),
        "---\ntoolbook: 1\n---\n\n1. 第一步\n\n   ````text name=a\n   x\n\n     y\n   ```\n   ````\n2. 下一步\n"
    );
    let reparsed = Document::parse(&doc.serialize());
    assert_eq!(reparsed.blocks().next().unwrap().1.content, "x\n\n  y\n```");
}

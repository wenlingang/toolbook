use toolbook_app::library::Library;

#[test]
fn first_launch_seeds_library_then_lists_and_creates() {
    let home = std::env::temp_dir().join(format!("toolbook-lib-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    // SAFETY: the only test in this binary; nothing else reads the environment meanwhile.
    unsafe { std::env::set_var("TOOLBOOK_HOME", &home) };
    assert_eq!(Library::root(), home.to_string_lossy());
    assert!(Library::ensure_seeded());
    assert!(!Library::ensure_seeded(), "seeds only once");
    let names: Vec<String> = Library::list().into_iter().map(|n| n.name).collect();
    assert!(names.contains(&"排查登录-401".to_owned()), "{names:?}");
    let items = Library::list();
    let csv = items.iter().find(|n| n.name == "csv-to-sql").unwrap();
    assert_eq!(csv.dir, "数据转换");
    let made = Library::create("草稿".into(), "新笔记".into()).unwrap();
    let text = std::fs::read_to_string(&made).unwrap();
    assert!(text.starts_with("---\ntoolbook: 1\n---\n"));
    assert!(text.contains("# 新笔记"));
    // A new note shows the basics: blocks passing values along, all running.
    let report = toolbook_core::run_file(
        std::path::Path::new(&made),
        &toolbook_core::engine::RunOptions::default(),
    )
    .unwrap();
    assert!(report.blocks.len() >= 3, "{text}");
    for b in &report.blocks {
        assert!(
            matches!(b.status, toolbook_core::engine::BlockStatus::Ok(_)),
            "{:?} in\n{text}",
            b.status
        );
    }
    assert!(
        Library::create("草稿".into(), "新笔记".into()).is_err(),
        "no overwrite"
    );
    assert!(Library::create("".into(), "../x".into()).is_err());

    // Categories: folders, empty ones included, made by name.
    let made_dir = Library::create_dir("接口调试".into()).unwrap();
    assert!(std::path::Path::new(&made_dir).is_dir());
    assert!(
        Library::dirs().contains(&"接口调试".to_owned()),
        "{:?}",
        Library::dirs()
    );
    assert!(
        Library::create_dir("接口调试".into()).is_err(),
        "no duplicate"
    );
    assert!(Library::create_dir(".hidden".into()).is_err());
    assert!(
        Library::create_dir("a/b".into()).is_err(),
        "one level, no slashes"
    );
    assert!(Library::create_dir("  ".into()).is_err());
    let in_dir = Library::create("接口调试".into(), "未命名".into()).unwrap();
    assert!(in_dir.contains("接口调试"));

    // Renaming and moving notes, renaming categories.
    let renamed = Library::rename(in_dir.clone(), "登录接口".into()).unwrap();
    assert!(renamed.ends_with("接口调试/登录接口.md") && !std::path::Path::new(&in_dir).exists());
    assert!(Library::rename(renamed.clone(), "a/b".into()).is_err());
    let moved = Library::move_to(renamed.clone(), "草稿".into()).unwrap();
    assert!(moved.ends_with("草稿/登录接口.md") && !std::path::Path::new(&renamed).exists());
    let other = Library::create("草稿".into(), "另一本".into()).unwrap();
    assert!(
        Library::rename(other.clone(), "登录接口".into()).is_err(),
        "no overwrite on rename"
    );
    std::fs::create_dir_all(home.join("接口调试")).unwrap();
    let copy = Library::create("接口调试".into(), "登录接口".into()).unwrap();
    assert!(
        Library::move_to(copy, "草稿".into()).is_err(),
        "no overwrite on move"
    );
    let dir2 = Library::rename_dir("接口调试".into(), "API".into()).unwrap();
    assert!(dir2.ends_with("API") && Library::dirs().contains(&"API".to_owned()));
    assert!(
        Library::rename_dir("API".into(), "草稿".into()).is_err(),
        "no merge into another category"
    );

    // Categories keep the order they were dragged into; new ones go last.
    let before = Library::dirs();
    let mut order = before.clone();
    order.reverse();
    Library::set_dir_order(order.clone()).unwrap();
    assert_eq!(Library::dirs(), order);
    Library::create_dir("最新".into()).unwrap();
    assert_eq!(Library::dirs().last().map(String::as_str), Some("最新"));
    assert!(
        Library::list()
            .iter()
            .all(|n| !n.path.contains(".toolbook.json"))
    );
    // A renamed category keeps its place.
    let pos = Library::dirs().iter().position(|d| d == "API").unwrap();
    Library::rename_dir("API".into(), "接口".into()).unwrap();
    assert_eq!(Library::dirs().iter().position(|d| d == "接口"), Some(pos));

    // A library folder with no notes in it (made by hand, or emptied) is seeded too.
    std::fs::remove_dir_all(&home).unwrap();
    std::fs::create_dir_all(home.join(".themes")).unwrap();
    assert!(Library::ensure_seeded());
    assert!(!Library::list().is_empty());
}

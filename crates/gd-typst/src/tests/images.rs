//! Image export exercises real layout, confinement and dependency observation.

use super::*;

#[test]
fn image_targets_write_one_page_and_honour_output_reservation() {
    let dir = TempDir::new("images");
    for (target, name, header) in [
        (Target::Svg, "circle.svg", b"<svg".as_slice()),
        (Target::Png, "circle.png", b"\x89PNG\r\n\x1a\n".as_slice()),
    ] {
        let options = Options { target, ..offline() };
        let out = dir.0.join(name);
        let source = "#set page(width: auto, height: auto, margin: 0pt)\n#circle(radius: 12pt)";
        compile_text(&Backend::Embedded, source, &dir.0, &out, &options)
            .expect("valid image fixture");
        let bytes = std::fs::read(&out).expect("valid image fixture");
        assert!(bytes.windows(header.len()).any(|w| w == header));
        let refuse = |_| false;
        let options = Options { reserve: Some(&refuse), ..options };
        assert!(matches!(
            compile_text(&Backend::Embedded, source, &dir.0, &out, &options),
            Err(Error::OutputTooLarge { .. })
        ));
    }
}

#[test]
fn multiple_pages_and_oversize_rasters_are_rejected_before_export() {
    let dir = TempDir::new("image-limits");
    for (target, source, message) in [
        (Target::Svg, "a #pagebreak() b", "exactly one page"),
        (Target::Png, "#set page(width: 10000pt, height: 10000pt)\nx", "pixel limit"),
    ] {
        let result = compile_text(
            &Backend::Embedded,
            source,
            &dir.0,
            &dir.0.join("out"),
            &Options { target, ..offline() },
        );
        assert!(result.expect_err("the source must fail").message().contains(message));
        assert!(!dir.0.join("out").exists());
    }
}

#[test]
fn generated_text_resolves_and_observes_nested_imports_and_missing_data() {
    use std::sync::Mutex;
    let dir = TempDir::new("image-imports");
    std::fs::create_dir_all(dir.0.join("sub")).expect("valid image fixture");
    std::fs::write(dir.0.join("sub/helper.typ"), "#let draw = circle(radius: 12pt)")
        .expect("valid image fixture");
    let files = Mutex::new(Vec::new());
    let observe = |path: &Path| {
        files.lock().expect("valid image fixture").push(path.to_path_buf());
        true
    };
    let options = Options {
        target: Target::Svg,
        source_path: Some(dir.0.join("sub/page.typ")),
        observe_file: Some(&observe),
        read_roots: vec![dir.0.clone()],
        ..offline()
    };
    let out = dir.0.join("out.svg");
    let source = "#set page(width: auto, height: auto, margin: 0pt)\n\
                  #import \"helper.typ\": draw\n#draw";
    compile_text(&Backend::Embedded, source, &dir.0, &out, &options).expect("valid image fixture");
    assert!(files.lock().expect("valid image fixture").contains(&dir.0.join("sub/helper.typ")));
    let error = compile_text(&Backend::Embedded, "#read(\"missing.txt\")", &dir.0, &out, &options)
        .expect_err("the source must fail");
    assert!(error.message().contains("not found"));
    assert!(files.lock().expect("valid image fixture").contains(&dir.0.join("sub/missing.txt")));
}

#[test]
fn image_packages_work_offline_and_missing_packages_fail_without_downloads() {
    let dir = TempDir::new("image-packages");
    let cache = dir.0.join("packages");
    let package = cache.join("preview/guidedog-diagram-test/0.0.0");
    std::fs::create_dir_all(&package).expect("package cache");
    std::fs::write(package.join("typst.toml"), concat!(
        "[package]\nname = 'guidedog-diagram-test'\nversion = '0.0.0'\n",
        "entrypoint = 'lib.typ'\nauthors = ['Test']\nlicense = 'MIT'\ndescription = 'Fixture'\n",
    )).expect("package manifest");
    std::fs::write(package.join("lib.typ"), "#let draw = circle(radius: 12pt)")
        .expect("package source");
    let options = Options { target: Target::Svg, package_cache: Some(cache), ..offline() };
    let source = concat!(
        "#set page(width: auto, height: auto, margin: 0pt)\n",
        "#import \"@preview/guidedog-diagram-test:0.0.0\": draw\n#draw",
    );
    compile_text(&Backend::Embedded, source, &dir.0, &dir.0.join("out.svg"), &options)
        .expect("an installed package works offline");
    let missing = source.replace(":0.0.0", ":0.0.1");
    let result =
        compile_text(&Backend::Embedded, &missing, &dir.0, &dir.0.join("missing.svg"), &options);
    assert!(result.expect_err("missing offline package").message().contains("package"));
    assert!(!package.with_file_name("0.0.1").exists());
}

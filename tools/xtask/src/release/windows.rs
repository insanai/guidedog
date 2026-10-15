//! Windows archives keep Graphviz's runtime and notices beside the programs.

use std::path::Path;
use std::process::Command;

use crate::fsutil::{copy, copy_tree, sorted_entries};
use crate::{Result, fail, process};

/// Copies the verified Graphviz distribution's runtime and notices into the archive.
pub(super) fn copy_runtime(folder: &Path) -> Result<()> {
    let graphviz = std::env::var_os("GUIDEDOG_GRAPHVIZ_WINDOWS").ok_or_else(|| {
        fail(
            "windows-graphviz",
            "NO GRAPHVIZ RUNTIME",
            "GUIDEDOG_GRAPHVIZ_WINDOWS does not name the extracted Graphviz distribution.",
            "Extract the pinned Windows Graphviz archive and set its installation folder.",
        )
    })?;
    let graphviz = Path::new(&graphviz);
    for file in sorted_entries(&graphviz.join("bin"))? {
        let name = file.file_name().unwrap_or_default();
        if file.extension().is_some_and(|ext| ext == "dll") || name == "config8" {
            copy(&file, &folder.join(name))?;
        }
    }
    // Keep upstream notices and sources together, including the third-party licenses.
    for directory in ["share", "etc"] {
        let source = graphviz.join(directory);
        if source.is_dir() {
            copy_tree(&source, &folder.join(directory))?;
        }
    }
    for file in sorted_entries(graphviz)? {
        if file.is_file() {
            copy(&file, &folder.join(file.file_name().unwrap_or_default()))?;
        }
    }
    Ok(())
}

/// Writes a ZIP using the PowerShell supplied by Windows.
pub(super) fn zip(stage: &Path, folder: &str, out: &Path) -> Result<()> {
    let target = out.join(format!("{folder}.zip"));
    let mut command = Command::new("powershell");
    command.current_dir(stage).args(["-NoProfile", "-Command"]);
    command.arg(
        "Compress-Archive -LiteralPath $env:PACKAGE_FOLDER \
        -DestinationPath $env:PACKAGE_ARCHIVE -Force",
    );
    command.env("PACKAGE_FOLDER", folder).env("PACKAGE_ARCHIVE", &target);
    process::run(&mut command)?;
    println!("{}", target.display());
    Ok(())
}

/// Records the exact source and bundled Graphviz distribution for Windows.
pub(super) fn source_text(version: &str, commit: &str, rustc: &str, command: &str) -> String {
    format!(
        "Guidedog {version} for Windows x86-64\nCommit: {commit}\n\
         Source: https://github.com/insanai/guidedog/tree/v{version}\n\
         Toolchain: {rustc}\nBuild: {command}\n\
         Typst 0.15.1 is embedded. Graphviz 16.1.0 DLLs are bundled.\n\
         Graphviz source: https://gitlab.com/graphviz/graphviz/-/tree/16.1.0\n\
         Official runtime: https://graphviz.org/download/\n\
         Archive: windows_10_cmake_Release_Graphviz-16.1.0-win64.zip\n\
         SHA256: 733e49626c492242eb8dca30ea627b6ead20710e207998c7933b4909d92d6abc\n\
         Keep the extracted folder together, including DLLs, config8 and notices.\n"
    )
}

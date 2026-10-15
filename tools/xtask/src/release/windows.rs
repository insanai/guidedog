//! Windows archives keep Graphviz's runtime and notices beside the programs.

use std::path::Path;
use std::process::Command;

use crate::fsutil::{copy, copy_tree};
use crate::{Result, fail, process};

// The dependency closure of gvc, cgraph, core, dot_layout and neato_layout in the
// pinned upstream archive. Other renderers and their image/font libraries are omitted.
const RUNTIME_DLLS: [&str; 18] = [
    "cdt.dll",
    "cgraph.dll",
    "glib-2.0-0.dll",
    "gts.dll",
    "gvc.dll",
    "gvplugin_core.dll",
    "gvplugin_dot_layout.dll",
    "gvplugin_neato_layout.dll",
    "iconv-2.dll",
    "intl-8.dll",
    "libexpat.dll",
    "msvcp140.dll",
    "pathplan.dll",
    "pcre2-8.dll",
    "vcruntime140.dll",
    "vcruntime140_1.dll",
    "xdot.dll",
    "zlib1.dll",
];

/// Copies the verified Graphviz distribution's runtime and notices into the archive.
pub(super) fn copy_runtime(folder: &Path) -> Result<()> {
    let graphviz = std::env::var_os("GUIDEDOG_GRAPHVIZ_WINDOWS").ok_or_else(|| {
        fail(
            "windows-graphviz",
            "NO GRAPHVIZ RUNTIME",
            "GUIDEDOG_GRAPHVIZ_WINDOWS does not name the extracted Graphviz distribution.",
            "Set GUIDEDOG_GRAPHVIZ_WINDOWS to the extracted Graphviz installation folder.",
        )
    })?;
    let graphviz = Path::new(&graphviz);
    for name in RUNTIME_DLLS {
        copy(&graphviz.join("bin").join(name), &folder.join(name))?;
    }
    let notices = crate::repository_root().join("tools/release/windows-notices");
    copy_tree(&notices, &folder.join("notices"))?;
    // Generate a config for the plugins we ship, so no optional system plugins are used.
    let mut dot = Command::new(graphviz.join("bin/dot.exe"));
    dot.arg("-c").env("GVBINDIR", folder);
    process::run(&mut dot)?;
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

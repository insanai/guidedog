//! `cargo xtask graphviz`: builds the static Graphviz libraries that release binaries link,
//! so `guidedog` and `graphdog` need no Graphviz installation at run time.
//!
//! It downloads the pinned Graphviz and Expat release tarballs (each checked against its
//! SHA-256), builds Expat as a static library (Graphviz parses HTML-like labels with it),
//! then builds Graphviz with no optional dependencies: no cairo, pango, gd, X11, Tcl or
//! ltdl, and no zlib (graphdog writes no compressed formats). Only the libraries and the
//! `core`, `dot_layout` and `neato_layout` plugins are built; gd-graphdog hands the
//! plugins to Graphviz itself, as Graphviz's own `dot_static` does. The archives land in
//! `<install>/lib`, where gd-graphdog's build script finds them through
//! `GUIDEDOG_GRAPHVIZ_STATIC=<install>` or its `static-graphviz` feature.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::args::{self, Spec};
use crate::fsutil::{absolute, copy, create_dir, read_text, remove, rename, sha256_file, write};
use crate::{Result, fail, process, repository_root, usage};

const HELP: &str = "\
Builds the static Graphviz libraries that release binaries link.

Usage: cargo xtask graphviz [--out DIR] [--jobs N]

  --out DIR   where the libraries are installed (default: target/graphviz/16.1.0)
  --jobs N    parallel compile jobs for make (default: 4)

Downloads Graphviz 16.1.0 and Expat 2.7.1 (checked against their pinned SHA-256),
builds them as static libraries with only what Guidedog needs, and prints the install
folder. An install folder that is already complete is kept. Sources and logs stay in
target/graphviz/work. Needs a C and C++ compiler, make, curl, tar and xz.

Then build with the libraries linked in:

  GUIDEDOG_GRAPHVIZ_STATIC=target/graphviz/16.1.0 cargo build --release -p guidedog-cmd
";

/// The Graphviz release that static builds pin. gd-graphdog's build script and its
/// text-layout plugin (which reads Graphviz 16.1's `textspan_t`) expect this version.
pub(crate) const GRAPHVIZ_VERSION: &str = "16.1.0";

/// Where the Graphviz release tarball comes from, and its SHA-256.
const GRAPHVIZ: Source = Source {
    name: "graphviz-16.1.0",
    url: "https://gitlab.com/api/v4/projects/4207231/packages/generic/graphviz-releases/\
          16.1.0/graphviz-16.1.0.tar.xz",
    sha256: "0f661718f3e5268dc3bad0fb53ab646605d26db70f4080799aff1dc2f61783c3",
};

/// The Expat release Graphviz is built with (MIT licence).
const EXPAT: Source = Source {
    name: "expat-2.7.1",
    url: "https://github.com/libexpat/libexpat/releases/download/R_2_7_1/expat-2.7.1.tar.xz",
    sha256: "354552544b8f99012e5062f7d570ec77f14b412a3ff5c7d8d0dae62c0d217c30",
};

/// The Graphviz folders built, in order: the libraries, then the three plugins.
const MAKE_DIRS: [&str; 4] = ["lib", "plugin/core", "plugin/dot_layout", "plugin/neato_layout"];

/// The convenience archives Graphviz's own `dot_static` links (cmd/dot/Makefile.am),
/// in link order; each is installed without its `_C` suffix.
const ARCHIVES: [&str; 10] = [
    "plugin/dot_layout/.libs/libgvplugin_dot_layout_C.a",
    "plugin/neato_layout/.libs/libgvplugin_neato_layout_C.a",
    "plugin/core/.libs/libgvplugin_core_C.a",
    "lib/gvc/.libs/libgvc_C.a",
    "lib/pathplan/.libs/libpathplan_C.a",
    "lib/xdot/.libs/libxdot_C.a",
    "lib/cgraph/.libs/libcgraph_C.a",
    "lib/cdt/.libs/libcdt_C.a",
    "lib/util/.libs/libutil_C.a",
    "lib/vpsc/.libs/libvpsc_C.a",
];

/// Graphviz's configure options: static libraries only, every optional dependency off.
const CONFIGURE: [&str; 44] = [
    "--disable-shared",
    "--enable-static",
    "--with-pic",
    "--disable-ltdl",
    "--without-x",
    "--disable-swig",
    "--disable-man-pdfs",
    "--with-expat=yes",
    "--without-pangocairo",
    "--without-gdk",
    "--without-gdk-pixbuf",
    "--without-gtk",
    "--without-gtkgl",
    "--without-gtkglext",
    "--without-rsvg",
    "--without-webp",
    "--without-poppler",
    "--without-ghostscript",
    "--without-lasi",
    "--without-gts",
    "--without-ann",
    "--without-qt",
    "--without-libgd",
    "--without-freetype2",
    "--without-glut",
    "--without-devil",
    "--without-glade",
    "--without-quartz",
    "--disable-tcl",
    "--disable-python",
    "--disable-perl",
    "--disable-ruby",
    "--disable-lua",
    "--disable-php",
    "--disable-guile",
    "--disable-java",
    "--disable-go",
    "--disable-sharp",
    "--disable-ocaml",
    "--disable-r",
    "--disable-d",
    // zlib only compresses svgz and friends, which graphdog never writes; pretending the
    // header is missing keeps a system zlib out of the archives.
    "ac_cv_header_zlib_h=no",
    "ac_cv_lib_z_main=no",
    "ac_cv_lib_z_deflateBound=no",
];

/// A pinned source tarball.
#[derive(Debug, Clone, Copy)]
struct Source {
    /// The tarball's top folder, and its file name without `.tar.xz`.
    name: &'static str,
    url: &'static str,
    sha256: &'static str,
}

/// The folders one build uses.
#[derive(Debug)]
struct Layout {
    /// Downloads, sources, logs and the Expat prefix.
    work: PathBuf,
    /// Where the libraries are installed (`lib/`, the licences and the `VERSION` stamp).
    install: PathBuf,
    /// Parallel make jobs.
    jobs: u32,
}

/// Runs `cargo xtask graphviz`.
pub(crate) fn main(args: &[String]) -> Result<()> {
    let spec = Spec { command: "graphviz", flags: &[], options: &["--out", "--jobs"], help: HELP };
    let Some(parsed) = args::parse(args, &spec)? else { return Ok(()) };
    parsed.at_most(0, &spec)?;
    let jobs = match parsed.option("--jobs") {
        None => 4,
        Some(text) => text.parse().ok().filter(|&n| n > 0).ok_or_else(|| {
            usage(&format!("`--jobs {text}` is not a positive number."), "Pass `--jobs 4`.")
        })?,
    };
    let install = match parsed.option("--out") {
        Some(dir) => absolute(Path::new(dir)),
        None => default_install(),
    };
    let layout = Layout { work: repository_root().join("target/graphviz/work"), install, jobs };
    ensure_built(&layout)?;
    println!("{}", layout.install.display());
    Ok(())
}

/// The default install folder: `target/graphviz/<version>` in the repository.
pub(crate) fn default_install() -> PathBuf {
    repository_root().join("target/graphviz").join(GRAPHVIZ_VERSION)
}

/// The install folder release steps use: GUIDEDOG_GRAPHVIZ_STATIC when set, otherwise the
/// default one.
pub(crate) fn install_dir() -> PathBuf {
    match std::env::var_os("GUIDEDOG_GRAPHVIZ_STATIC").filter(|dir| !dir.is_empty()) {
        Some(dir) => absolute(Path::new(&dir)),
        None => default_install(),
    }
}

/// The lines of a release's SOURCE.txt that say which Graphviz and Expat sources the
/// programs link, as the licences ask.
pub(crate) fn source_text() -> String {
    let mut text = String::from("Statically linked, built by `cargo xtask graphviz` from:\n");
    for source in [GRAPHVIZ, EXPAT] {
        text.push_str(&format!("    {}\n      SHA-256 {}\n", source.url, source.sha256));
    }
    text
}

/// Downloads the pinned Graphviz and Expat source tarballs into `dir` (unless they are
/// there with the right sums) and returns their paths, for the release's source files.
pub(crate) fn fetch_sources(dir: &Path) -> Result<Vec<PathBuf>> {
    Ok(vec![fetch(dir, GRAPHVIZ)?, fetch(dir, EXPAT)?])
}

/// Downloads a tarball into `dir` unless it is there with the right sum; returns its path.
fn fetch(dir: &Path, source: Source) -> Result<PathBuf> {
    let tarball = dir.join(format!("{}.tar.xz", source.name));
    if !(tarball.is_file() && sha256_file(&tarball)? == source.sha256) {
        download(source, &tarball)?;
    }
    Ok(tarball)
}

/// Builds the static libraries into the default folder unless they are there already,
/// and returns that folder. The release build calls this.
pub(crate) fn ensure_default(jobs: u32) -> Result<PathBuf> {
    let layout = Layout {
        work: repository_root().join("target/graphviz/work"),
        install: default_install(),
        jobs,
    };
    ensure_built(&layout)?;
    Ok(layout.install)
}

/// Whether `install` holds a finished build of the pinned version: the `VERSION` stamp is
/// written last.
fn is_installed(install: &Path) -> bool {
    let stamp = install.join("VERSION");
    stamp.is_file() && read_text(&stamp).is_ok_and(|text| text.trim() == GRAPHVIZ_VERSION)
}

fn ensure_built(layout: &Layout) -> Result<()> {
    if is_installed(&layout.install) {
        eprintln!("Graphviz {GRAPHVIZ_VERSION} is already built.");
        return Ok(());
    }
    create_dir(&layout.work)?;
    let expat = unpack(&layout.work, EXPAT)?;
    let graphviz = unpack(&layout.work, GRAPHVIZ)?;
    let expat_prefix = layout.work.join("expat-prefix");
    build_expat(layout, &expat, &expat_prefix)?;
    build_graphviz(layout, &graphviz, &expat_prefix)?;
    install(layout, &graphviz, &expat, &expat_prefix)
}

/// Fetches and extracts a tarball into a fresh source folder, which it returns.
fn unpack(work: &Path, source: Source) -> Result<PathBuf> {
    let tarball = fetch(work, source)?;
    let dir = work.join(source.name);
    remove(&dir)?;
    eprintln!("extracting {}", source.name);
    process::run(Command::new("tar").arg("-xJf").arg(&tarball).arg("-C").arg(work))?;
    Ok(dir)
}

fn download(source: Source, tarball: &Path) -> Result<()> {
    eprintln!("fetching {}", source.url);
    let part = tarball.with_extension("xz.part");
    let mut curl = Command::new("curl");
    curl.args(["-sSfL", "--retry", "3", "-o"]).arg(&part).arg(source.url);
    process::run(&mut curl)?;
    let got = sha256_file(&part)?;
    if got != source.sha256 {
        remove(&part)?;
        return Err(fail(
            "graphviz-sum",
            "WRONG CHECKSUM",
            &format!("{}.tar.xz has SHA-256 {got}, expected {}.", source.name, source.sha256),
            "The download was changed or cut short. Run the task again; if the sum stays \
             wrong, check the release page before changing the pin in \
             tools/xtask/src/graphviz.rs.",
        ));
    }
    rename(&part, tarball)
}

/// Builds Expat as a static, position-independent library into `prefix`.
fn build_expat(layout: &Layout, src: &Path, prefix: &Path) -> Result<()> {
    eprintln!("building {}", EXPAT.name);
    let log = layout.work.join("expat.log");
    let mut configure = clean_env(Command::new(src.join("configure")));
    configure.current_dir(src).arg(format!("--prefix={}", prefix.display()));
    configure.args([
        "--disable-shared",
        "--enable-static",
        "--with-pic",
        "--without-xmlwf",
        "--without-examples",
        "--without-tests",
        "--without-docbook",
    ]);
    run_logged(&mut configure, &log)?;
    let mut make = clean_env(Command::new("make"));
    make.current_dir(src).arg(format!("-j{}", layout.jobs)).arg("install");
    run_logged(&mut make, &log)
}

/// Configures Graphviz against the static Expat and builds the libraries and plugins.
fn build_graphviz(layout: &Layout, src: &Path, expat: &Path) -> Result<()> {
    eprintln!("building {} (a few minutes)", GRAPHVIZ.name);
    let log = layout.work.join("graphviz.log");
    let mut configure = clean_env(Command::new(src.join("configure")));
    configure.current_dir(src);
    configure.arg(format!("--prefix={}", layout.work.join("graphviz-prefix").display()));
    configure.args(CONFIGURE);
    configure.env("EXPAT_CFLAGS", format!("-I{}", expat.join("include").display()));
    configure.env("EXPAT_LIBS", format!("-L{} -lexpat", expat.join("lib").display()));
    // pkg-config would find a system expat first; the static one must win.
    configure.env("PKG_CONFIG_PATH", expat.join("lib/pkgconfig"));
    configure.env("PKG_CONFIG_LIBDIR", expat.join("lib/pkgconfig"));
    run_logged(&mut configure, &log)?;
    for dir in MAKE_DIRS {
        let mut make = clean_env(Command::new("make"));
        make.current_dir(src.join(dir)).arg(format!("-j{}", layout.jobs));
        run_logged(&mut make, &log)?;
    }
    Ok(())
}

/// Copies the archives, the licences and finally the version stamp into the install
/// folder.
fn install(layout: &Layout, graphviz: &Path, expat: &Path, expat_prefix: &Path) -> Result<()> {
    let lib = layout.install.join("lib");
    // Only what this task wrote is replaced: `--out` may name a folder with other files.
    remove(&lib)?;
    create_dir(&lib)?;
    for archive in ARCHIVES {
        let name = Path::new(archive).file_name().unwrap_or_default().to_string_lossy();
        copy(&graphviz.join(archive), &lib.join(name.replace("_C.a", ".a")))?;
    }
    copy(&expat_prefix.join("lib/libexpat.a"), &lib.join("libexpat.a"))?;
    if cfg!(target_os = "linux") {
        // Graphviz's vpsc is C++; linking libstdc++ statically keeps the program free of
        // a libstdc++ version requirement (the GCC runtime library exception allows it).
        copy(&compiler_file("c++", "libstdc++.a")?, &lib.join("libstdc++.a"))?;
    }
    copy(&graphviz.join("COPYING"), &layout.install.join("COPYING-graphviz"))?;
    copy(&expat.join("COPYING"), &layout.install.join("COPYING-expat"))?;
    write(&layout.install.join("VERSION"), format!("{GRAPHVIZ_VERSION}\n"))
}

/// The path of a file the compiler links with, such as `libstdc++.a`.
fn compiler_file(compiler: &str, name: &str) -> Result<PathBuf> {
    let mut command = Command::new(compiler);
    command.arg(format!("-print-file-name={name}"));
    let path = PathBuf::from(process::output(&mut command)?.trim());
    if path.is_absolute() && path.is_file() {
        return Ok(path);
    }
    Err(fail(
        "missing-library",
        "LIBRARY NOT FOUND",
        &format!("The C++ compiler has no {name}."),
        "Install the static C++ library (Debian/Ubuntu: apt install libstdc++-14-dev, or \
         the version that matches g++).",
    ))
}

/// A command without the compiler flags of the environment (a Homebrew CPPFLAGS, say),
/// which must not leak into the build; only `-O2` is set.
fn clean_env(mut command: Command) -> Command {
    for name in ["CPPFLAGS", "LDFLAGS", "CFLAGS", "CXXFLAGS", "MAKEFLAGS"] {
        command.env_remove(name);
    }
    command.env("CFLAGS", "-O2").env("CXXFLAGS", "-O2");
    // Release programs run on macOS 12 and later, so the libraries they link must too.
    if cfg!(target_os = "macos") && std::env::var_os("MACOSX_DEPLOYMENT_TARGET").is_none() {
        command.env("MACOSX_DEPLOYMENT_TARGET", "12.0");
    }
    command
}

/// Runs a build step with its output appended to `log`; on failure, shows the log's last
/// lines.
fn run_logged(command: &mut Command, log: &Path) -> Result<()> {
    let open = || {
        File::options()
            .create(true)
            .append(true)
            .open(log)
            .map_err(|err| crate::fsutil::io_failure("open", log, &err))
    };
    command.stdin(Stdio::null()).stdout(open()?).stderr(open()?);
    let status = command.status().map_err(|err| process::spawn_failure(command, &err))?;
    if status.success() {
        return Ok(());
    }
    let text = read_text(log).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    for line in &lines[lines.len().saturating_sub(30)..] {
        eprintln!("{line}");
    }
    Err(fail(
        "graphviz-build",
        "BUILD FAILED",
        &format!("`{}` failed; its output is in {}.", process::describe(command), log.display()),
        "Read the lines above. The build needs a C and C++ compiler and make; install them \
         with the system's package manager and run `cargo xtask graphviz` again.",
    ))
}

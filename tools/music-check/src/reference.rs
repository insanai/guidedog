//! An external oracle produces review artifacts; no reference implementation is linked.

use std::fmt::Write;
use std::path::Path;
use std::process::Command;

pub(crate) fn check(root: &Path, program: &str) -> Result<(), String> {
    let version = Command::new(program).arg("--version").output().map_err(|e| e.to_string())?;
    if !String::from_utf8_lossy(&version.stdout).starts_with("GNU LilyPond 2.24.") {
        return Err("The visual reference must be LilyPond 2.24.x.".into());
    }
    let directory =
        root.join("tests/integration/data/music").canonicalize().map_err(|e| e.to_string())?;
    let output = root.join("target/music-reference");
    std::fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    let mut sources: Vec<_> = std::fs::read_dir(&directory)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "ly"))
        .collect();
    sources.sort();
    let mut logs = String::new();
    let mut failures = 0;
    for source in &sources {
        let stem =
            source.file_stem().and_then(|s| s.to_str()).ok_or("Invalid corpus filename.")?;
        let mut text = std::fs::read_to_string(source).map_err(|e| e.to_string())?;
        if matches!(stem, "36-lyrics" | "37-verses") {
            text = format!("<< {} >>", text.replace("\\lyricsto", "\\new Lyrics \\lyricsto"));
        }
        if stem == "22-measure-rests" {
            text = format!("\\compressMMRests {text}");
        }
        let wrapper = format!(
            concat!(
                "\\version \"2.24.4\"\n#(set-global-staff-size 18)\n",
                "\\paper {{ indent = 0\\mm line-width = 123.4722\\mm ragged-right = ##t }}\n",
                "\\layout {{ }}\n{}"
            ),
            text
        );
        let input = output.join(format!("{stem}.ly"));
        std::fs::write(&input, wrapper).map_err(|e| e.to_string())?;
        let result = Command::new(program)
            .args(["-dbackend=svg", "-dcrop", "-dno-point-and-click"])
            .arg("-o")
            .arg(output.join(stem))
            .arg(&input)
            .output()
            .map_err(|e| e.to_string())?;
        writeln!(logs, "{stem}: {}\n{}", result.status, String::from_utf8_lossy(&result.stderr))
            .map_err(|e| e.to_string())?;
        failures += usize::from(!result.status.success());
    }
    std::fs::write(output.join("reference.log"), logs).map_err(|e| e.to_string())?;
    atlas(&directory, &output, &sources)?;
    if failures > 0 {
        return Err(format!("{failures} reference scores failed; see target/music-reference."));
    }
    let pages = sources.len().div_ceil(12);
    println!("{} LilyPond references and {pages} review pages generated.", sources.len());
    Ok(())
}

fn atlas(directory: &Path, output: &Path, sources: &[std::path::PathBuf]) -> Result<(), String> {
    for (page, chunk) in sources.chunks(12).enumerate() {
        let mut html = String::from(concat!(
            "<!doctype html><meta charset=utf-8>",
            "<style>body{font:14px sans-serif;margin:20px;background:white;color:black}",
            "table{border-collapse:collapse;width:100%}td{border:1px solid #bbb;padding:10px;",
            "width:50%;vertical-align:top}img{max-width:100%}h2{font-size:14px}</style>",
            "<table><tr><th>Guidedog</th><th>LilyPond 2.24</th></tr>"
        ));
        for source in chunk {
            let stem =
                source.file_stem().and_then(|s| s.to_str()).ok_or("Invalid corpus name.")?;
            let cropped = format!("{stem}.cropped.svg");
            let reference =
                if output.join(&cropped).is_file() { cropped } else { format!("{stem}.svg") };
            write!(
                html,
                "<tr><td><h2>{stem}</h2><img src=\"file://{}/{stem}.svg\"></td>\
                <td><h2>{stem}</h2><img src=\"{reference}\"></td></tr>",
                directory.display()
            )
            .map_err(|e| e.to_string())?;
        }
        html.push_str("</table>");
        std::fs::write(output.join(format!("review-{}.html", page + 1)), html)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

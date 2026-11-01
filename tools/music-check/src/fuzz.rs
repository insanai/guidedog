const SEEDS: &[&str] = &[
    r"\relative c' { c8( d e f) g4~ g }",
    r"<< { cis'2 c'2 } \\ { c'2 s2 } >>",
    r"{ \tuplet 3/2 { c'8 d' e' } c'2. }",
    "{ c'4^\"音楽 संगीत ♫\" d' e' f' }",
    r#"{ c'4^\markup { \bold "Allegro" \box \circle "A" \flat } d'2. }"#,
    r"{\repeat volta 3 { c'1 } \alternative { \volta 1,2 { d'1 } \volta 3 { e'1 } }}",
    r"<< { c''4 d'' } \\ { c'2 } \\ { g'8 a' b' c'' } >>",
    r"<< { c'4 } \\ { c'4 } \\ { cis'4 } \\ { d'4 } >>",
    r"x = { \x } \x",
    r"{ \repeat unfold 999999999 { c'128 } }",
];

pub(crate) fn check(count: usize) -> Result<(), String> {
    if count == 0 || count > 10_000_000 {
        return Err("Use 1..10000000 fuzz cases.".into());
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let data = crate::fonts::load(&root)?;
    let fonts = data.fonts()?;
    let mut engraver = gd_engrave::Engraver::default();
    let alphabet = b"{}<>[]()\\#$_ abcdefgrs012348.,'~|%\"*/=-\n";
    let mut seed = 0x7b41fcu64;
    let (mut read, mut drawn) = (0, 0);
    for index in 0..count {
        let mut text = String::new();
        for _ in 0..index % 512 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            text.push(char::from(alphabet[(seed >> 32) as usize % alphabet.len()]));
        }
        if index % 3 == 0 {
            text = structured(index, &mut seed);
        } else if index % 3 == 1 {
            text = SEEDS[index % SEEDS.len()].to_string();
            if index % 2 == 0 {
                let cut = text
                    .char_indices()
                    .nth(index % text.chars().count())
                    .map(|p| p.0)
                    .unwrap_or(0);
                text.truncate(cut);
            }
        }
        let mut options = gd_lilypond::Options {
            limits: gd_lilypond::Limits {
                work: 4096,
                depth: 16,
                events: 128,
                source_bytes: 512,
                files: 2,
                memory_bytes: 256 * 1024,
            },
            ..Default::default()
        };
        if let Ok(loaded) = gd_lilypond::load_score(&text, "fuzz.ly", &mut options) {
            read += 1;
            let mut options = gd_engrave::Options {
                limits: gd_engrave::Limits { work: 200_000, ..Default::default() },
                ..Default::default()
            };
            if let Ok(drawing) = engraver.engrave_score(&loaded.score, &fonts, &mut options) {
                drawn += 1;
                crate::corpus::audit(&drawing).map_err(|e| format!("fuzz #{index}: {e}"))?;
                let _ = gd_engrave::write_svg(
                    &mut String::new(),
                    &drawing,
                    &gd_engrave::SvgOptions { max_bytes: 65536, ..Default::default() },
                    None,
                );
            }
        }
    }
    println!("{count} deterministic cases: {read} parsed, {drawn} engraved.");
    Ok(())
}

fn structured(index: usize, seed: &mut u64) -> String {
    let mut text = String::from("\\relative c' { \\time 4/4 ");
    for _ in 0..index % 12 + 1 {
        *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        text.push_str(
            [
                "c8 d e f g a b c | ",
                "c4( e g c) | ",
                "<c e g>2 r2 | ",
                "\\tuplet 3/2 { c8 d e } f4 g2 | ",
            ][(*seed >> 32) as usize % 4],
        );
    }
    text.push('}');
    text
}

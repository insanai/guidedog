//! Localized musical key names for generated score descriptions.

use gd_env::UiText;
use gd_score::{Alter, Key, Mode};

const GERMAN_TONICS: [[&str; 5]; 7] = [
    ["Ceses", "Ces", "C", "Cis", "Cisis"],
    ["Deses", "Des", "D", "Dis", "Disis"],
    ["Eses", "Es", "E", "Eis", "Eisis"],
    ["Feses", "Fes", "F", "Fis", "Fisis"],
    ["Geses", "Ges", "G", "Gis", "Gisis"],
    ["Asas", "As", "A", "Ais", "Aisis"],
    ["Heses", "B", "H", "His", "Hisis"],
];

pub(super) fn key_name(language: &str, ui: &dyn UiText, key: Key) -> String {
    let base = language.split(['-', '_']).next().unwrap_or(language);
    let step = key.tonic.step.index() as usize;
    let alter_idx = alter_index(key.tonic.alter);
    let accidental = accidental_msgid(key.tonic.alter);
    let mode_id = mode_msgid(key.mode);
    let mode = ui.gettext(mode_id);
    match base {
        "de" => {
            let raw = GERMAN_TONICS[step][alter_idx];
            let tonic =
                if key.mode == Mode::Minor { raw.to_ascii_lowercase() } else { raw.to_string() };
            format!("{tonic}-{mode}")
        }
        "ja" => {
            let note = ["ハ", "ニ", "ホ", "ヘ", "ト", "イ", "ロ"][step];
            let acc = ["重変", "変", "", "嬰", "重嬰"][alter_idx];
            format!("{acc}{note}{mode}")
        }
        "zh" => {
            let note = ["C", "D", "E", "F", "G", "A", "B"][step];
            let acc = if accidental.is_empty() { "" } else { &ui.gettext(accidental) };
            format!("{acc}{note}{mode}")
        }
        "ko" => {
            let note = ["다", "라", "마", "바", "사", "가", "나"][step];
            let acc = ["겹내림 ", "내림 ", "", "올림 ", "겹올림 "][alter_idx];
            let join = if matches!(key.mode, Mode::Major | Mode::Minor) { "" } else { " " };
            format!("{acc}{note}{join}{mode}")
        }
        "es" => {
            solfege_key(["do", "re", "mi", "fa", "sol", "la", "si"][step], accidental, &mode, ui)
        }
        "fr" => {
            solfege_key(["do", "ré", "mi", "fa", "sol", "la", "si"][step], accidental, &mode, ui)
        }
        "pt" => {
            solfege_key(["dó", "ré", "mi", "fá", "sol", "lá", "si"][step], accidental, &mode, ui)
        }
        "ru" => {
            let note = ["до", "ре", "ми", "фа", "соль", "ля", "си"][step];
            let acc = if accidental.is_empty() {
                String::new()
            } else {
                format!("-{}", ui.gettext(accidental))
            };
            format!("{note}{acc} {mode}")
        }
        _ => {
            let note = ["C", "D", "E", "F", "G", "A", "B"][step];
            solfege_key(note, accidental, &mode, ui)
        }
    }
}

fn solfege_key(note: &str, accidental: &str, mode: &str, ui: &dyn UiText) -> String {
    let mut out = note.to_string();
    if !accidental.is_empty() {
        out.push(' ');
        out.push_str(&ui.gettext(accidental));
    }
    out.push(' ');
    out.push_str(mode);
    out
}

fn alter_index(alter: Alter) -> usize {
    match alter {
        Alter::DoubleFlat => 0,
        Alter::Flat => 1,
        Alter::Natural => 2,
        Alter::Sharp => 3,
        Alter::DoubleSharp => 4,
    }
}

fn accidental_msgid(alter: Alter) -> &'static str {
    match alter {
        Alter::DoubleFlat => "double flat",
        Alter::Flat => "flat",
        Alter::Natural => "",
        Alter::Sharp => "sharp",
        Alter::DoubleSharp => "double sharp",
    }
}

fn mode_msgid(mode: Mode) -> &'static str {
    match mode {
        Mode::Major => "major",
        Mode::Minor => "minor",
        Mode::Dorian => "Dorian",
        Mode::Phrygian => "Phrygian",
        Mode::Lydian => "Lydian",
        Mode::Mixolydian => "Mixolydian",
        Mode::Locrian => "Locrian",
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use gd_env::{EnglishUi, UiText};
    use gd_score::{Alter, Key, Mode, Pitch, Step};

    use super::key_name;

    struct MapUi(&'static [(&'static str, &'static str)]);

    impl UiText for MapUi {
        fn gettext<'a>(&'a self, msgid: &'a str) -> Cow<'a, str> {
            self.0
                .iter()
                .find_map(|&(k, v)| (k == msgid).then_some(Cow::Borrowed(v)))
                .unwrap_or(Cow::Borrowed(msgid))
        }

        fn ngettext<'a>(&'a self, singular: &'a str, plural: &'a str, n: u64) -> Cow<'a, str> {
            Cow::Borrowed(if n == 1 { singular } else { plural })
        }
    }

    fn key(step: Step, alter: Alter, mode: Mode) -> Key {
        Key { tonic: Pitch { step, alter, octave: 0 }, mode }
    }

    #[test]
    fn key_names_follow_each_languages_musical_orthography() {
        let g_maj = key(Step::G, Alter::Natural, Mode::Major);
        let fis_min = key(Step::F, Alter::Sharp, Mode::Minor);
        let b_maj = key(Step::B, Alter::Flat, Mode::Major);
        let h_min = key(Step::B, Alter::Natural, Mode::Minor);
        let d_dor = key(Step::D, Alter::Natural, Mode::Dorian);

        assert_eq!(key_name("en", &EnglishUi, fis_min), "F sharp minor");
        assert_eq!(key_name("en", &EnglishUi, b_maj), "B flat major");

        let de = MapUi(&[("major", "Dur"), ("minor", "Moll"), ("Dorian", "Dorisch")]);
        assert_eq!(key_name("de", &de, g_maj), "G-Dur");
        assert_eq!(key_name("de", &de, fis_min), "fis-Moll");
        assert_eq!(key_name("de", &de, b_maj), "B-Dur");
        assert_eq!(key_name("de", &de, h_min), "h-Moll");
        assert_eq!(key_name("de", &de, d_dor), "D-Dorisch");

        let ja = MapUi(&[("major", "長調"), ("minor", "短調"), ("Dorian", "ドリア旋法")]);
        assert_eq!(key_name("ja", &ja, g_maj), "ト長調");
        assert_eq!(key_name("ja", &ja, fis_min), "嬰ヘ短調");
        assert_eq!(key_name("ja", &ja, b_maj), "変ロ長調");
        assert_eq!(key_name("ja", &ja, d_dor), "ニドリア旋法");

        let zh = MapUi(&[("sharp", "升"), ("flat", "降"), ("major", "大调"), ("minor", "小调")]);
        assert_eq!(key_name("zh_CN", &zh, fis_min), "升F小调");
        assert_eq!(key_name("zh_CN", &zh, b_maj), "降B大调");

        let ko = MapUi(&[("major", "장조"), ("minor", "단조"), ("Dorian", "도리아 선법")]);
        assert_eq!(key_name("ko", &ko, g_maj), "사장조");
        assert_eq!(key_name("ko", &ko, fis_min), "올림 바단조");
        assert_eq!(key_name("ko", &ko, b_maj), "내림 나장조");
        assert_eq!(key_name("ko", &ko, d_dor), "라 도리아 선법");

        let fr = MapUi(&[("sharp", "dièse"), ("major", "majeur"), ("minor", "mineur")]);
        assert_eq!(key_name("fr", &fr, g_maj), "sol majeur");
        assert_eq!(key_name("fr", &fr, fis_min), "fa dièse mineur");

        let es = MapUi(&[("sharp", "sostenido"), ("major", "mayor"), ("minor", "menor")]);
        assert_eq!(key_name("es", &es, fis_min), "fa sostenido menor");

        let pt = MapUi(&[("sharp", "sustenido"), ("major", "maior"), ("minor", "menor")]);
        assert_eq!(key_name("pt_BR", &pt, fis_min), "fá sustenido menor");

        let ru = MapUi(&[
            ("sharp", "диез"),
            ("flat", "бемоль"),
            ("major", "мажор"),
            ("minor", "минор"),
        ]);
        assert_eq!(key_name("ru", &ru, fis_min), "фа-диез минор");
        assert_eq!(key_name("ru", &ru, b_maj), "си-бемоль мажор");
        assert_eq!(key_name("ru", &ru, g_maj), "соль мажор");
    }
}

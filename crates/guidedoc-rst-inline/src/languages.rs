//! Language data for bibliographic fields, generated titles, and quotation marks. The
//! field names are the ones writers of each language use for these document parts; the
//! quotation marks follow the Unicode CLDR delimiters of each language.

/// A bibliographic field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Bibliographic {
    Author,
    Authors,
    Organization,
    Address,
    Contact,
    Version,
    Revision,
    Status,
    Date,
    Copyright,
    Dedication,
    Abstract,
}

impl Bibliographic {
    /// Every field, in table order.
    pub const ALL: [Bibliographic; 12] = [
        Bibliographic::Author,
        Bibliographic::Authors,
        Bibliographic::Organization,
        Bibliographic::Address,
        Bibliographic::Contact,
        Bibliographic::Version,
        Bibliographic::Revision,
        Bibliographic::Status,
        Bibliographic::Date,
        Bibliographic::Copyright,
        Bibliographic::Dedication,
        Bibliographic::Abstract,
    ];

    /// The English key, used as the docinfo attribute value and the topic class.
    pub fn key(self) -> &'static str {
        ENGLISH_KEYS[self as usize]
    }
}

const ENGLISH_KEYS: [&str; 12] = [
    "author",
    "authors",
    "organization",
    "address",
    "contact",
    "version",
    "revision",
    "status",
    "date",
    "copyright",
    "dedication",
    "abstract",
];

/// One document language. `names` lists the lowercase field name of each
/// bibliographic field (in `Bibliographic::ALL` order). `quotes` holds the primary
/// opening and closing marks, then the secondary ones; `alt` the alternative set.
#[derive(Debug)]
pub(crate) struct Language {
    pub tag: &'static str,
    pub names: [&'static str; 12],
    pub dedication: &'static str,
    pub abstract_: &'static str,
    /// The title of a table of contents without one.
    pub contents: &'static str,
    /// Author separators, tried in order.
    pub separators: &'static str,
    pub quotes: [&'static str; 4],
    pub alt: [&'static str; 4],
}

impl Language {
    /// Finds the field a normalized (case-folded) field name names.
    pub fn bibliographic(&self, name: &str) -> Option<Bibliographic> {
        if name.is_empty() {
            return None;
        }
        let k = self.names.iter().position(|n| *n == name)?;
        Some(Bibliographic::ALL[k])
    }
}

/// Finds a language by BCP 47 tag: the whole tag, then its primary subtag, compared
/// without case and with "_" as "-"; English otherwise.
pub(crate) fn language_of(tag: &str) -> &'static Language {
    if tag.is_empty() {
        return &LANGUAGES[0];
    }
    if let Some(l) = LANGUAGES.iter().find(|l| tag_equal(l.tag, tag)) {
        return l;
    }
    let primary = tag.split(['-', '_']).next().unwrap_or(tag);
    LANGUAGES.iter().find(|l| tag_equal(l.tag, primary)).unwrap_or(&LANGUAGES[0])
}

/// Compares language tags without case, "_" as "-".
pub(crate) fn tag_equal(a: &str, b: &str) -> bool {
    let norm = |c: u8| if c == b'_' { b'-' } else { c.to_ascii_lowercase() };
    a.len() == b.len() && a.bytes().zip(b.bytes()).all(|(x, y)| norm(x) == norm(y))
}

static LANGUAGES: [Language; 15] = [
    Language {
        tag: "en",
        names: [
            "author",
            "authors",
            "organization",
            "address",
            "contact",
            "version",
            "revision",
            "status",
            "date",
            "copyright",
            "dedication",
            "abstract",
        ],
        dedication: "Dedication",
        abstract_: "Abstract",
        contents: "Contents",
        separators: ";,",
        quotes: ["“", "”", "‘", "’"],
        alt: ["‘", "’", "“", "”"],
    },
    Language {
        tag: "de",
        names: [
            "autor",
            "autoren",
            "organisation",
            "adresse",
            "kontakt",
            "version",
            "revision",
            "status",
            "datum",
            "copyright",
            "widmung",
            "zusammenfassung",
        ],
        dedication: "Widmung",
        abstract_: "Zusammenfassung",
        contents: "Inhalt",
        separators: ";,",
        quotes: ["„", "“", "‚", "‘"],
        alt: ["»", "«", "›", "‹"],
    },
    Language {
        tag: "fr",
        names: [
            "auteur",
            "auteurs",
            "organisation",
            "adresse",
            "contact",
            "version",
            "révision",
            "statut",
            "date",
            "copyright",
            "dédicace",
            "résumé",
        ],
        dedication: "Dédicace",
        abstract_: "Résumé",
        contents: "Sommaire",
        separators: ";,",
        quotes: ["« ", " »", "“", "”"],
        alt: ["« ", " »", "“", "”"],
    },
    Language {
        tag: "es",
        names: [
            "autor",
            "autores",
            "organización",
            "dirección",
            "contacto",
            "versión",
            "revisión",
            "estado",
            "fecha",
            "copyright",
            "dedicatoria",
            "resumen",
        ],
        dedication: "Dedicatoria",
        abstract_: "Resumen",
        contents: "Contenido",
        separators: ";,",
        quotes: ["«", "»", "“", "”"],
        alt: ["“", "”", "‘", "’"],
    },
    Language {
        tag: "it",
        names: [
            "autore",
            "autori",
            "organizzazione",
            "indirizzo",
            "contatti",
            "versione",
            "revisione",
            "status",
            "data",
            "copyright",
            "dedica",
            "riassunto",
        ],
        dedication: "Dedica",
        abstract_: "Riassunto",
        contents: "Indice",
        separators: ";,",
        quotes: ["«", "»", "“", "”"],
        alt: ["“", "”", "‘", "’"],
    },
    Language {
        tag: "nl",
        names: [
            "auteur",
            "auteurs",
            "organisatie",
            "adres",
            "contact",
            "versie",
            "revisie",
            "status",
            "datum",
            "copyright",
            "toewijding",
            "samenvatting",
        ],
        dedication: "Toewijding",
        abstract_: "Samenvatting",
        contents: "Inhoud",
        separators: ";,",
        quotes: ["“", "”", "‘", "’"],
        alt: ["„", "”", "‚", "’"],
    },
    Language {
        tag: "sv",
        names: [
            "författare",
            "författarna",
            "organisation",
            "adress",
            "kontakt",
            "version",
            "revision",
            "status",
            "datum",
            "copyright",
            "dedikation",
            "sammanfattning",
        ],
        dedication: "Dedikation",
        abstract_: "Sammanfattning",
        contents: "Innehåll",
        separators: ";,",
        quotes: ["”", "”", "’", "’"],
        alt: ["»", "»", "›", "›"],
    },
    Language {
        tag: "fi",
        names: [
            "tekijä",
            "tekijät",
            "yhteisö",
            "osoite",
            "yhteystiedot",
            "versio",
            "revisio",
            "tila",
            "päiväys",
            "tekijänoikeudet",
            "omistuskirjoitus",
            "tiivistelmä",
        ],
        dedication: "Omistuskirjoitus",
        abstract_: "Tiivistelmä",
        contents: "Sisällys",
        separators: ";,",
        quotes: ["”", "”", "’", "’"],
        alt: ["»", "»", "›", "›"],
    },
    Language {
        tag: "da",
        names: [
            "forfatter",
            "forfattere",
            "organisation",
            "adresse",
            "kontakt",
            "version",
            "revision",
            "status",
            "dato",
            "copyright",
            "dedikation",
            "resumé",
        ],
        dedication: "Dedikation",
        abstract_: "Resumé",
        contents: "Indhold",
        separators: ";,",
        quotes: ["“", "”", "‘", "’"],
        alt: ["»", "«", "›", "‹"],
    },
    Language {
        tag: "pl",
        names: [
            "autor",
            "autorzy",
            "organizacja",
            "adres",
            "kontakt",
            "wersja",
            "korekta",
            "status",
            "data",
            "prawa autorskie",
            "dedykacja",
            "streszczenie",
        ],
        dedication: "Dedykacja",
        abstract_: "Streszczenie",
        contents: "Spis treści",
        separators: ";,",
        quotes: ["„", "”", "«", "»"],
        alt: ["«", "»", "„", "”"],
    },
    Language {
        tag: "cs",
        names: [
            "autor",
            "autoři",
            "organizace",
            "adresa",
            "kontakt",
            "verze",
            "revize",
            "stav",
            "datum",
            "copyright",
            "věnování",
            "abstrakt",
        ],
        dedication: "Věnování",
        abstract_: "Abstrakt",
        contents: "Obsah",
        separators: ";,",
        quotes: ["„", "“", "‚", "‘"],
        alt: ["»", "«", "›", "‹"],
    },
    Language {
        tag: "ru",
        names: [
            "автор",
            "авторы",
            "организация",
            "адрес",
            "контакт",
            "версия",
            "редакция",
            "статус",
            "дата",
            "авторские права",
            "посвящение",
            "аннотация",
        ],
        dedication: "Посвящение",
        abstract_: "Аннотация",
        contents: "Содержание",
        separators: ";,",
        quotes: ["«", "»", "„", "“"],
        alt: ["„", "“", "‚", "‘"],
    },
    Language {
        tag: "ja",
        names: [
            "著者",
            "著者一覧",
            "組織",
            "住所",
            "連絡先",
            "バージョン",
            "リビジョン",
            "ステータス",
            "日付",
            "著作権",
            "献辞",
            "概要",
        ],
        dedication: "献辞",
        abstract_: "概要",
        contents: "目次",
        separators: "；;，,、",
        quotes: ["「", "」", "『", "』"],
        alt: ["“", "”", "‘", "’"],
    },
    Language {
        tag: "zh",
        names: [
            "作者",
            "作者们",
            "组织",
            "地址",
            "联系",
            "版本",
            "修订",
            "状态",
            "日期",
            "版权",
            "献辞",
            "摘要",
        ],
        dedication: "献辞",
        abstract_: "摘要",
        contents: "目录",
        separators: "；;，,、",
        quotes: ["“", "”", "‘", "’"],
        alt: ["「", "」", "『", "』"],
    },
    Language {
        tag: "zh-tw",
        names: [
            "作者",
            "作者們",
            "組織",
            "地址",
            "聯絡",
            "版本",
            "修訂",
            "狀態",
            "日期",
            "版權",
            "獻辭",
            "摘要",
        ],
        dedication: "獻辭",
        abstract_: "摘要",
        contents: "目錄",
        separators: "；;，,、",
        quotes: ["「", "」", "『", "』"],
        alt: ["“", "”", "‘", "’"],
    },
];

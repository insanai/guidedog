use gd_score::{ColumnAlign, Enclosure, Markup, StyleChange};

use crate::cursor::Cursor;
use crate::interpret::{Reader, unsupported};
use crate::limits::{limit, push};
use crate::parser::syntax_error;
use crate::{Error, ExprKind, Span, SyntaxId, TokenKind};

const REFUSED_MARKUP: &[&str] = &[
    "score",
    "score-lines",
    "note",
    "note-by-number",
    "rhythm",
    "override",
    "override-lines",
    "fromproperty",
    "on-the-fly",
    "postscript",
    "epsfile",
    "path",
    "draw-line",
    "draw-circle",
    "filled-box",
    "arrow-head",
    "with-url",
    "with-color",
    "rotate",
    "scale",
    "magnify",
    "fill-line",
    "wordwrap",
    "justify",
    "semisharp",
    "semiflat",
    "sesquisharp",
    "sesquiflat",
    "figured-bass",
];

/// Markup commands that take exactly one markup argument (a brace list counts as one).
const ONE_ARGUMENT: &[&str] = &[
    "markup",
    "bold",
    "italic",
    "upright",
    "medium",
    "normal-text",
    "roman",
    "serif",
    "sans",
    "typewriter",
    "number",
    "finger",
    "left-align",
    "center-align",
    "right-align",
    "caps",
    "smallCaps",
    "line",
    "concat",
    "column",
    "left-column",
    "dir-column",
    "center-column",
    "right-column",
    "box",
    "rounded-box",
    "circle",
    "ellipse",
    "underline",
    "parenthesize",
    "bracket",
    "hbracket",
    "sub",
    "super",
    "teeny",
    "tiny",
    "small",
    "smaller",
    "large",
    "larger",
    "huge",
];

impl Reader<'_> {
    pub(crate) fn read_markup(
        &self,
        cursor: &mut Cursor<'_>,
        depth: usize,
    ) -> Result<Markup, Error> {
        self.markup_expr(cursor, depth)
    }

    fn markup_expr(&self, cursor: &mut Cursor<'_>, depth: usize) -> Result<Markup, Error> {
        let span = cursor.span();
        self.work.take(1, span)?;
        self.memory.take(128, span)?;
        if depth > self.limits.depth {
            return Err(limit("markup depth", span));
        }
        let node = cursor.next()?;
        match &node.kind {
            ExprKind::Sequence(ids) => self.markup_list(ids, depth + 1, false),
            ExprKind::Atom(
                TokenKind::Text(text) | TokenKind::Word(text) | TokenKind::Number(text),
            ) => Ok(Markup::Text(self.copy_text(text, node.span)?)),
            ExprKind::Atom(TokenKind::Symbol('#')) => {
                if let Some(ExprKind::Atom(TokenKind::Text(text))) = cursor.peek().map(|n| &n.kind)
                {
                    cursor.at += 1;
                    Ok(Markup::Text(self.copy_text(text, node.span)?))
                } else {
                    Err(unsupported(node.span, "Scheme"))
                }
            }
            ExprKind::Atom(TokenKind::Symbol('$')) => Err(unsupported(node.span, "Scheme")),
            ExprKind::Atom(TokenKind::Command(name)) => {
                self.markup_command(name, cursor, node.span, depth + 1)
            }
            _ => Err(syntax_error(node.span, "Expected text or a supported markup command.")),
        }
    }

    fn markup_list(&self, ids: &[SyntaxId], depth: usize, concat: bool) -> Result<Markup, Error> {
        let mut inner = Cursor::new(self.syntax, ids);
        let mut items = Vec::new();
        while inner.peek().is_some() {
            let span = inner.span();
            let item = self.markup_expr(&mut inner, depth)?;
            push(&mut items, item, span)?;
        }
        if items.len() == 1 {
            return Ok(items.pop().unwrap_or_else(|| Markup::Text(String::new())));
        }
        Ok(if concat { Markup::Concat(items) } else { Markup::Line(items) })
    }

    fn markup_items(&self, cursor: &mut Cursor<'_>, depth: usize) -> Result<Vec<Markup>, Error> {
        let ids = cursor.sequence()?;
        let mut inner = Cursor::new(self.syntax, ids);
        let mut items = Vec::new();
        while inner.peek().is_some() {
            let span = inner.span();
            let item = self.markup_expr(&mut inner, depth)?;
            push(&mut items, item, span)?;
        }
        Ok(items)
    }

    fn markup_command(
        &self,
        name: &str,
        cursor: &mut Cursor<'_>,
        span: Span,
        depth: usize,
    ) -> Result<Markup, Error> {
        if let Some(ids) = self.variables.get(name) {
            return self.expand_markup_var(ids, span, depth);
        }
        if REFUSED_MARKUP.contains(&name) {
            return Err(unsupported(span, &format!("markup \\{name}")));
        }
        match name {
            "markup" => self.markup_expr(cursor, depth),
            "bold" | "italic" | "upright" | "medium" | "normal-text" => {
                let change = match name {
                    "bold" => StyleChange::Bold,
                    "italic" => StyleChange::Italic,
                    "upright" => StyleChange::Upright,
                    "medium" => StyleChange::Medium,
                    _ => StyleChange::Normal,
                };
                Ok(Markup::Style(change, Box::new(self.markup_expr(cursor, depth)?)))
            }
            // Markup is drawn in the project's text faces, so family and alignment
            // commands keep their argument's weight and slant rather than resetting them.
            "roman" | "serif" | "sans" | "typewriter" | "number" | "finger" | "left-align"
            | "center-align" | "right-align" => self.markup_expr(cursor, depth),
            "caps" | "smallCaps" => Ok(uppercase_markup(self.markup_expr(cursor, depth)?)),
            "line" => {
                let ids = cursor.sequence()?;
                self.markup_list(ids, depth, false)
            }
            "concat" => {
                let ids = cursor.sequence()?;
                self.markup_list(ids, depth, true)
            }
            "column" | "left-column" | "dir-column" => {
                Ok(Markup::Column(ColumnAlign::Left, self.markup_items(cursor, depth)?))
            }
            "center-column" => {
                Ok(Markup::Column(ColumnAlign::Center, self.markup_items(cursor, depth)?))
            }
            "right-column" => {
                Ok(Markup::Column(ColumnAlign::Right, self.markup_items(cursor, depth)?))
            }
            "box" | "rounded-box" => {
                Ok(Markup::Framed(Enclosure::Box, Box::new(self.markup_expr(cursor, depth)?)))
            }
            "circle" | "ellipse" => {
                Ok(Markup::Framed(Enclosure::Circle, Box::new(self.markup_expr(cursor, depth)?)))
            }
            "underline" => Ok(Markup::Framed(
                Enclosure::Underline,
                Box::new(self.markup_expr(cursor, depth)?),
            )),
            "parenthesize" => Ok(Markup::Concat(vec![
                Markup::Text("(".into()),
                self.markup_expr(cursor, depth)?,
                Markup::Text(")".into()),
            ])),
            "bracket" | "hbracket" => Ok(Markup::Concat(vec![
                Markup::Text("[".into()),
                self.markup_expr(cursor, depth)?,
                Markup::Text("]".into()),
            ])),
            _ => self.markup_symbol_or_metric(name, cursor, span, depth),
        }
    }

    fn markup_symbol_or_metric(
        &self,
        name: &str,
        cursor: &mut Cursor<'_>,
        span: Span,
        depth: usize,
    ) -> Result<Markup, Error> {
        match name {
            "sub" => Ok(Markup::Scaled {
                scale_percent: 72,
                shift_percent: -30,
                body: Box::new(self.markup_expr(cursor, depth)?),
            }),
            "super" => Ok(Markup::Scaled {
                scale_percent: 72,
                shift_percent: 35,
                body: Box::new(self.markup_expr(cursor, depth)?),
            }),
            "teeny" | "tiny" | "small" | "smaller" | "large" | "larger" | "huge" => {
                let scale_percent = match name {
                    "teeny" => 65,
                    "tiny" => 75,
                    "small" => 85,
                    "smaller" => 88,
                    "larger" => 112,
                    "large" => 118,
                    _ => 132,
                };
                Ok(Markup::Scaled {
                    scale_percent,
                    shift_percent: 0,
                    body: Box::new(self.markup_expr(cursor, depth)?),
                })
            }
            "fontsize" => {
                let step = markup_number(cursor, span)?.clamp(-12.0, 12.0);
                let scale = (2.0f64.powf(step / 6.0) * 100.0).round() as u16;
                Ok(Markup::Scaled {
                    scale_percent: scale.clamp(25, 400),
                    shift_percent: 0,
                    body: Box::new(self.markup_expr(cursor, depth)?),
                })
            }
            "abs-fontsize" => {
                let pt = markup_number(cursor, span)?.clamp(4.0, 72.0);
                let scale = ((pt / 10.8) * 100.0).round() as u16;
                Ok(Markup::Scaled {
                    scale_percent: scale.clamp(25, 600),
                    shift_percent: 0,
                    body: Box::new(self.markup_expr(cursor, depth)?),
                })
            }
            "raise" | "lower" => {
                let amount = markup_number(cursor, span)?.clamp(-8.0, 8.0);
                let shift =
                    (amount * 40.0 * if name == "lower" { -1.0 } else { 1.0 }).round() as i16;
                Ok(Markup::Scaled {
                    scale_percent: 100,
                    shift_percent: shift,
                    body: Box::new(self.markup_expr(cursor, depth)?),
                })
            }
            "halign" => {
                let _ = markup_number(cursor, span)?;
                self.markup_expr(cursor, depth)
            }
            "hspace" | "vspace" => {
                let value =
                    (markup_number(cursor, span)?.clamp(-20.0, 20.0) * 100.0).round() as i16;
                Ok(if name == "hspace" {
                    Markup::Space { horizontal: value, vertical: 0 }
                } else {
                    Markup::Space { horizontal: 0, vertical: value }
                })
            }
            _ => self.markup_music_symbol(name, cursor, span),
        }
    }

    fn markup_music_symbol(
        &self,
        name: &str,
        cursor: &mut Cursor<'_>,
        span: Span,
    ) -> Result<Markup, Error> {
        match name {
            "null" | "strut" => Ok(Markup::Text(String::new())),
            "sharp" => Ok(Markup::MusicGlyph("accidentalSharp".into())),
            "flat" => Ok(Markup::MusicGlyph("accidentalFlat".into())),
            "natural" => Ok(Markup::MusicGlyph("accidentalNatural".into())),
            "doublesharp" => Ok(Markup::MusicGlyph("accidentalDoubleSharp".into())),
            "doubleflat" => Ok(Markup::MusicGlyph("accidentalDoubleFlat".into())),
            "fermata" => Ok(Markup::MusicGlyph("fermataAbove".into())),
            "segno" => Ok(Markup::MusicGlyph("segno".into())),
            "coda" => Ok(Markup::MusicGlyph("coda".into())),
            "varcoda" => Ok(Markup::MusicGlyph("codaSquare".into())),
            "dynamic" => {
                cursor.symbol('#');
                let text = self.copy_text(cursor.text()?, span)?;
                if text.is_empty() || !text.chars().all(|c| "pmfrsz".contains(c)) {
                    return Err(unsupported(span, &format!("dynamic `{text}`")));
                }
                Ok(Markup::Dynamic(text))
            }
            "musicglyph" => {
                cursor.symbol('#');
                let raw = cursor.text()?;
                let mapped = map_musicglyph(raw)
                    .ok_or_else(|| unsupported(span, &format!("musicglyph `{raw}`")))?;
                Ok(Markup::MusicGlyph(mapped.into()))
            }
            _ => Err(unsupported(span, &format!("markup \\{name}"))),
        }
    }

    pub(crate) fn expand_markup_var(
        &self,
        ids: &[SyntaxId],
        span: Span,
        depth: usize,
    ) -> Result<Markup, Error> {
        if depth > self.limits.depth {
            return Err(limit("markup variable expansion", span));
        }
        let mut inner = Cursor::new(self.syntax, ids);
        inner.command("markup");
        let markup = self.markup_expr(&mut inner, depth + 1)?;
        if inner.peek().is_some() {
            return Err(syntax_error(span, "The variable is not a valid text or markup value."));
        }
        Ok(markup)
    }

    pub(crate) fn capture_markup(
        &self,
        cursor: &mut Cursor<'_>,
        depth: usize,
    ) -> Result<(), Error> {
        if depth > self.limits.depth {
            return Err(limit("markup binding syntax", cursor.span()));
        }
        let node = cursor.next()?;
        match &node.kind {
            ExprKind::Sequence(_)
            | ExprKind::Atom(TokenKind::Text(_) | TokenKind::Word(_) | TokenKind::Number(_)) => {}
            ExprKind::Atom(TokenKind::Symbol('#')) => {
                // Only a quoted string may follow `#`; anything else is Scheme.
                if !matches!(
                    cursor.peek().map(|n| &n.kind),
                    Some(ExprKind::Atom(TokenKind::Text(_)))
                ) {
                    return Err(unsupported(node.span, "Scheme"));
                }
                let _ = cursor.next()?;
            }
            ExprKind::Atom(TokenKind::Command(name))
                if REFUSED_MARKUP.contains(&name.as_str()) =>
            {
                return Err(unsupported(node.span, &format!("markup \\{name}")));
            }
            ExprKind::Atom(TokenKind::Command(name)) => match name.as_str() {
                "null" | "strut" | "sharp" | "flat" | "natural" | "doublesharp" | "doubleflat"
                | "fermata" | "segno" | "coda" | "varcoda" => {}
                "hspace" | "vspace" => {
                    let _ = markup_number(cursor, node.span)?;
                }
                "fontsize" | "abs-fontsize" | "raise" | "lower" | "halign" => {
                    let _ = markup_number(cursor, node.span)?;
                    self.capture_markup(cursor, depth + 1)?;
                }
                "dynamic" | "musicglyph" => {
                    cursor.symbol('#');
                    let _ = cursor.text()?;
                }
                _ if self.variables.contains_key(name) => {}
                _ if ONE_ARGUMENT.contains(&name.as_str()) => {
                    self.capture_markup(cursor, depth + 1)?;
                }
                // An unknown command's arguments are unknown too; refuse it here, where
                // its span is known, rather than let them leak into the music.
                _ => return Err(unsupported(node.span, &format!("markup \\{name}"))),
            },
            _ => return Err(syntax_error(node.span, "Invalid markup expression in binding.")),
        }
        Ok(())
    }
}

fn markup_number(cursor: &mut Cursor<'_>, span: Span) -> Result<f64, Error> {
    cursor.symbol('#');
    let negative = cursor.symbol('-');
    let integer = cursor.integer()? as f64;
    let fraction = if cursor.symbol('.') {
        let node = cursor.next()?;
        let ExprKind::Atom(TokenKind::Number(digits)) = &node.kind else {
            return Err(syntax_error(
                node.span,
                "Expected fractional digits after decimal point.",
            ));
        };
        digits
            .parse::<f64>()
            .ok()
            .map(|v| v / 10f64.powi(digits.len() as i32))
            .ok_or_else(|| syntax_error(span, "Invalid markup number."))?
    } else {
        0.0
    };
    let value = integer + fraction;
    Ok(if negative { -value } else { value })
}

fn uppercase_markup(markup: Markup) -> Markup {
    match markup {
        Markup::Text(s) => Markup::Text(s.to_uppercase()),
        Markup::Line(items) => Markup::Line(items.into_iter().map(uppercase_markup).collect()),
        Markup::Concat(items) => Markup::Concat(items.into_iter().map(uppercase_markup).collect()),
        Markup::Column(align, items) => {
            Markup::Column(align, items.into_iter().map(uppercase_markup).collect())
        }
        Markup::Style(style, body) => Markup::Style(style, Box::new(uppercase_markup(*body))),
        Markup::Scaled { scale_percent, shift_percent, body } => Markup::Scaled {
            scale_percent,
            shift_percent,
            body: Box::new(uppercase_markup(*body)),
        },
        Markup::Framed(frame, body) => Markup::Framed(frame, Box::new(uppercase_markup(*body))),
        other => other,
    }
}

fn map_musicglyph(name: &str) -> Option<&'static str> {
    Some(match name {
        "scripts.segno" | "segno" => "segno",
        "scripts.coda" | "coda" => "coda",
        "scripts.varcoda" | "codaSquare" => "codaSquare",
        "scripts.fermata" | "scripts.ufermata" | "fermataAbove" => "fermataAbove",
        "scripts.dfermata" | "fermataBelow" => "fermataBelow",
        "scripts.trill" | "ornamentTrill" => "ornamentTrill",
        "scripts.turn" | "ornamentTurn" => "ornamentTurn",
        "scripts.mordent" | "ornamentMordent" => "ornamentMordent",
        "scripts.prall" | "ornamentShortTrill" => "ornamentShortTrill",
        "scripts.staccato" | "articStaccatoAbove" => "articStaccatoAbove",
        "scripts.sforzato" | "articAccentAbove" => "articAccentAbove",
        "scripts.umarcato" | "articMarcatoAbove" => "articMarcatoAbove",
        "scripts.tenuto" | "articTenutoAbove" => "articTenutoAbove",
        "scripts.caesura.curved" | "scripts.caesura.straight" | "caesura" => "caesura",
        "clefs.G" | "gClef" => "gClef",
        "clefs.F" | "fClef" => "fClef",
        "clefs.C" | "cClef" => "cClef",
        "accidentals.sharp" | "accidentalSharp" => "accidentalSharp",
        "accidentals.flat" | "accidentalFlat" => "accidentalFlat",
        "accidentals.natural" | "accidentalNatural" => "accidentalNatural",
        "accidentals.doublesharp" | "accidentalDoubleSharp" => "accidentalDoubleSharp",
        "accidentals.flatflat" | "accidentalDoubleFlat" => "accidentalDoubleFlat",
        "pedal.Ped" | "keyboardPedalPed" => "keyboardPedalPed",
        "pedal.*" | "keyboardPedalUp" => "keyboardPedalUp",
        "timesig.C44" | "timeSigCommon" => "timeSigCommon",
        "timesig.C22" | "timeSigCutCommon" => "timeSigCutCommon",
        _ => return None,
    })
}

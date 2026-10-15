//! The document and value grammar.

use crate::table::Origin;
use crate::{Data, Error, ErrorKind, Table, Value, datetime, number};

/// The deepest nesting of arrays and inline tables [`parse`] accepts.
pub const MAX_DEPTH: usize = 64;

/// The hint of a parser state that cannot happen: a bug to report.
const LOST_TABLE: &str =
    "Report this bug with the TOML file and the output of guidedog --version.";

pub(crate) type Result<T> = std::result::Result<T, Error>;

/// One key of a dotted key path, with where it was written.
pub(crate) struct Key {
    pub(crate) name: String,
    pub(crate) offset: usize,
    pub(crate) end: usize,
}

pub(crate) struct Parser<'a> {
    pub(crate) src: &'a str,
    pub(crate) b: &'a [u8],
    pub(crate) at: usize,
    depth: usize,
    line_starts: Vec<usize>,
}

/// Reads `source` as a TOML 1.0 document into its root table.
///
/// The result owns its strings and does not borrow `source`. Fails with an [`Error`]
/// at the first mistake: a syntax error, a key or table defined twice, a bad string,
/// number or date-time, or nesting deeper than [`MAX_DEPTH`].
pub fn parse(source: &str) -> Result<Table> {
    let mut p = Parser {
        src: source,
        b: source.as_bytes(),
        at: 0,
        depth: 0,
        line_starts: std::iter::once(0)
            .chain(source.match_indices('\n').map(|(i, _)| i + 1))
            .collect(),
    };
    let mut root = Table::new();
    let mut section: Vec<String> = Vec::new();
    loop {
        p.skip_blank()?;
        if p.at >= p.b.len() {
            return Ok(root);
        }
        if p.peek("[[") {
            section = p.array_header(&mut root)?;
        } else if p.peek("[") {
            section = p.table_header(&mut root)?;
        } else {
            let at = p.at;
            let table = section_table(&mut root, &section).ok_or_else(|| {
                p.fail(ErrorKind::Syntax, at, at, "Lost the current table.", LOST_TABLE)
            })?;
            p.key_value(table)?;
        }
        p.end_line()?;
    }
}

/// The table `path` names from the root, entering the last table of arrays of tables.
fn section_table<'t>(root: &'t mut Table, path: &[String]) -> Option<&'t mut Table> {
    let mut t = root;
    for key in path {
        let i = t.position(key)?;
        t = table_in(&mut t.entry_mut(i).value)?;
    }
    Some(t)
}

/// A table value itself, or the last table of an array of tables.
fn table_in(value: &mut Value) -> Option<&mut Table> {
    match &mut value.data {
        Data::Table(t) => Some(t),
        Data::Array(items) => items.last_mut().and_then(Value::as_table_mut),
        _ => None,
    }
}

fn dotted(path: &[Key]) -> String {
    let names: Vec<&str> = path.iter().map(|k| k.name.as_str()).collect();
    names.join(".")
}

impl<'a> Parser<'a> {
    pub(crate) fn line_of(&self, offset: usize) -> usize {
        self.line_starts.partition_point(|&s| s <= offset)
    }

    /// Builds an error at bytes `start..end`.
    pub(crate) fn fail(
        &self,
        kind: ErrorKind,
        start: usize,
        end: usize,
        message: impl Into<String>,
        hint: &str,
    ) -> Error {
        let start = gd_report::floor_boundary(self.src, start);
        let line = self.line_of(start);
        let line_start = self.line_starts[line - 1];
        let column = self.src[line_start..start].chars().count() + 1;
        let end = gd_report::floor_boundary(self.src, end.max(start));
        Error {
            kind,
            message: message.into(),
            hint: hint.to_string(),
            offset: start,
            end,
            line,
            column,
        }
    }

    pub(crate) fn peek(&self, s: &str) -> bool {
        self.b[self.at..].starts_with(s.as_bytes())
    }

    pub(crate) fn byte(&self) -> Option<u8> {
        self.b.get(self.at).copied()
    }

    fn value_at(&self, data: Data, offset: usize) -> Value {
        Value { data, line: self.line_of(offset), offset }
    }

    fn skip_space(&mut self) {
        while matches!(self.byte(), Some(b' ' | b'\t')) {
            self.at += 1;
        }
    }

    /// Skips white space, line breaks and comments between statements and array items.
    fn skip_blank(&mut self) -> Result<()> {
        loop {
            match self.byte() {
                Some(b' ' | b'\t' | b'\n') => self.at += 1,
                Some(b'\r') if self.b.get(self.at + 1) == Some(&b'\n') => self.at += 2,
                Some(b'#') => self.skip_comment()?,
                _ => return Ok(()),
            }
        }
    }

    fn skip_comment(&mut self) -> Result<()> {
        while let Some(c) = self.byte() {
            if c == b'\n' || (c == b'\r' && self.b.get(self.at + 1) == Some(&b'\n')) {
                break;
            }
            if (c < 0x20 && c != b'\t') || c == 0x7f {
                return Err(self.fail(
                    ErrorKind::Syntax,
                    self.at,
                    self.at + 1,
                    "This comment holds a control character.",
                    "Remove it; comments may hold tabs but no other control characters.",
                ));
            }
            self.at += 1;
        }
        Ok(())
    }

    fn end_line(&mut self) -> Result<()> {
        self.skip_space();
        if self.byte() == Some(b'#') {
            self.skip_comment()?;
        }
        match self.byte() {
            None | Some(b'\n') => Ok(()),
            Some(b'\r') if self.b.get(self.at + 1) == Some(&b'\n') => Ok(()),
            _ => Err(self.fail(
                ErrorKind::Syntax,
                self.at,
                self.at + 1,
                "Expected the end of the line here.",
                "Put each `key = value` pair and each [table] header on its own line, and \
                 start comments with #.",
            )),
        }
    }

    fn expect(&mut self, close: &str, message: &str, hint: &str) -> Result<()> {
        self.skip_space();
        if !self.peek(close) {
            return Err(self.fail(ErrorKind::Syntax, self.at, self.at + 1, message, hint));
        }
        self.at += close.len();
        Ok(())
    }

    /// Reads `[a.b]`, defines the table and returns its path.
    fn table_header(&mut self, root: &mut Table) -> Result<Vec<String>> {
        let start = self.at;
        self.at += 1;
        let path = self.key_path()?;
        self.expect("]", "A table header ends with ].", "Close the header, as in [server].")?;
        let (last, parents) = path.split_last().ok_or_else(|| self.no_key())?;
        let mut t = root;
        for key in parents {
            t = self.header_step(t, key)?;
        }
        let header = self.value_at(Data::Table(Table::new()), start);
        match t.position(&last.name) {
            None => {
                t.push(last.name.clone(), header, Origin::Header);
            }
            Some(i) if t.entry(i).origin == Origin::Implicit => {
                let entry = t.entry_mut(i);
                entry.origin = Origin::Header;
                (entry.value.line, entry.value.offset) = (header.line, header.offset);
            }
            Some(i) => {
                let first = t.entry(i).value.line;
                return Err(self.fail(
                    ErrorKind::Duplicate,
                    start,
                    self.at,
                    format!("The table `{}` is already defined on line {first}.", dotted(&path)),
                    "Merge the two tables into one; TOML allows each table once.",
                ));
            }
        }
        Ok(path.into_iter().map(|k| k.name).collect())
    }

    /// Reads `[[a.b]]`, appends a table to the array and returns its path.
    fn array_header(&mut self, root: &mut Table) -> Result<Vec<String>> {
        let start = self.at;
        self.at += 2;
        let path = self.key_path()?;
        self.expect(
            "]]",
            "An array-of-tables header ends with ]].",
            "Close the header, as in [[products]].",
        )?;
        let (last, parents) = path.split_last().ok_or_else(|| self.no_key())?;
        let mut t = root;
        for key in parents {
            t = self.header_step(t, key)?;
        }
        let item = self.value_at(Data::Table(Table::new()), start);
        match t.position(&last.name) {
            None => {
                let array = self.value_at(Data::Array(vec![item]), start);
                t.push(last.name.clone(), array, Origin::ArrayOfTables);
            }
            Some(i) if t.entry(i).origin == Origin::ArrayOfTables => {
                if let Data::Array(items) = &mut t.entry_mut(i).value.data {
                    items.push(item);
                }
            }
            Some(i) => {
                let first = t.entry(i).value.line;
                return Err(self.fail(
                    ErrorKind::Duplicate,
                    start,
                    self.at,
                    format!(
                        "`{}` is already defined on line {first} as something other than an \
                         array of tables.",
                        dotted(&path)
                    ),
                    "Use [[name]] for every table of the array, or rename one of them.",
                ));
            }
        }
        Ok(path.into_iter().map(|k| k.name).collect())
    }

    fn no_key(&self) -> Error {
        self.fail(ErrorKind::Syntax, self.at, self.at, "Expected a key.", "Name the table.")
    }

    /// Enters (creating when absent) the table `key` names on the way to a header.
    fn header_step<'t>(&self, t: &'t mut Table, key: &Key) -> Result<&'t mut Table> {
        let i = match t.position(&key.name) {
            Some(i) => i,
            None => {
                let value = self.value_at(Data::Table(Table::new()), key.offset);
                t.push(key.name.clone(), value, Origin::Implicit)
            }
        };
        let entry = t.entry_mut(i);
        let line = entry.value.line;
        let frozen = entry.origin == Origin::Frozen;
        match table_in(&mut entry.value) {
            Some(table) if !frozen => Ok(table),
            _ => Err(self.fail(
                ErrorKind::Duplicate,
                key.offset,
                key.end,
                format!(
                    "`{}` is already set on line {line} to a value that is not a table.",
                    key.name
                ),
                "A header can only name tables; rename the table or the key.",
            )),
        }
    }

    /// Enters (creating when absent) the table `key` names in a dotted key.
    fn dotted_step<'t>(&self, t: &'t mut Table, key: &Key) -> Result<&'t mut Table> {
        let i = match t.position(&key.name) {
            Some(i) => i,
            None => {
                let value = self.value_at(Data::Table(Table::new()), key.offset);
                t.push(key.name.clone(), value, Origin::Dotted)
            }
        };
        let entry = t.entry_mut(i);
        let (line, origin) = (entry.value.line, entry.origin);
        match &mut entry.value.data {
            Data::Table(table) if origin == Origin::Dotted => Ok(table),
            Data::Table(_) => Err(self.fail(
                ErrorKind::Duplicate,
                key.offset,
                key.end,
                format!("The table `{}` is already defined on line {line}.", key.name),
                "Dotted keys cannot add to a table defined elsewhere; write the key inside \
                 that table instead.",
            )),
            _ => Err(self.fail(
                ErrorKind::Duplicate,
                key.offset,
                key.end,
                format!(
                    "`{}` is already set on line {line} to a value that is not a table.",
                    key.name
                ),
                "Rename one of the keys; a key cannot be both a value and a table.",
            )),
        }
    }

    /// Reads `key = value` into table `t`.
    fn key_value(&mut self, t: &mut Table) -> Result<()> {
        let path = self.key_path()?;
        self.expect("=", "Expected = after the key.", "Write a setting as `key = value`.")?;
        self.skip_space();
        let value = self.value()?;
        let (last, parents) = path.split_last().ok_or_else(|| self.no_key())?;
        let mut t = t;
        for key in parents {
            t = self.dotted_step(t, key)?;
        }
        if let Some(i) = t.position(&last.name) {
            let first = t.entry(i).value.line;
            return Err(self.fail(
                ErrorKind::Duplicate,
                path[0].offset,
                last.end,
                format!("`{}` is already set on line {first}.", dotted(&path)),
                "Remove one of the two settings; TOML allows each key once.",
            ));
        }
        t.push(last.name.clone(), value, Origin::Frozen);
        Ok(())
    }

    /// Reads a key path such as `a."b c".d`.
    fn key_path(&mut self) -> Result<Vec<Key>> {
        let mut path = Vec::with_capacity(2);
        loop {
            self.skip_space();
            let offset = self.at;
            let name = self.single_key()?;
            path.push(Key { name, offset, end: self.at });
            self.skip_space();
            if self.byte() != Some(b'.') {
                return Ok(path);
            }
            self.at += 1;
        }
    }

    fn single_key(&mut self) -> Result<String> {
        match self.byte() {
            Some(b'"') if !self.peek("\"\"\"") => return self.basic_string(),
            Some(b'\'') if !self.peek("'''") => return self.literal_string().map(str::to_string),
            _ => {}
        }
        let start = self.at;
        while self.byte().is_some_and(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-') {
            self.at += 1;
        }
        if self.at == start {
            return Err(self.fail(
                ErrorKind::Syntax,
                start,
                start + 1,
                "Expected a key here.",
                "Bare keys use letters, digits, _ and -; quote any other key, as in \"my key\".",
            ));
        }
        Ok(self.src[start..self.at].to_string())
    }

    /// Reads one value; arrays and inline tables recurse through here, bounded by depth.
    fn value(&mut self) -> Result<Value> {
        let start = self.at;
        if self.depth >= MAX_DEPTH {
            return Err(self.fail(
                ErrorKind::Depth,
                start,
                start + 1,
                format!("Values are nested more than {MAX_DEPTH} levels deep."),
                "Flatten the arrays or inline tables, or use [table] headers.",
            ));
        }
        self.depth += 1;
        let data = self.value_data();
        self.depth -= 1;
        Ok(self.value_at(data?, start))
    }

    fn value_data(&mut self) -> Result<Data> {
        Ok(match self.byte() {
            Some(b'"') if self.peek("\"\"\"") => Data::String(self.multiline_basic()?),
            Some(b'\'') if self.peek("'''") => Data::String(self.multiline_literal()?.to_string()),
            Some(b'"') => Data::String(self.basic_string()?),
            Some(b'\'') => Data::String(self.literal_string()?.to_string()),
            Some(b'[') => Data::Array(self.array()?),
            Some(b'{') => Data::Table(self.inline_table()?),
            _ if self.peek("true") => {
                self.at += 4;
                Data::Boolean(true)
            }
            _ if self.peek("false") => {
                self.at += 5;
                Data::Boolean(false)
            }
            _ if datetime::looks_like(&self.b[self.at..]) => self.datetime()?,
            _ => self.number()?,
        })
    }

    fn datetime(&mut self) -> Result<Data> {
        let start = self.at;
        match datetime::scan(&self.b[start..]) {
            Ok((d, used)) => {
                self.at += used;
                Ok(Data::Datetime(d))
            }
            Err(message) => Err(self.fail(
                ErrorKind::Datetime,
                start,
                start + 10,
                message,
                "Write dates as 1979-05-27, times as 07:32:00, and both as \
                 1979-05-27T07:32:00Z or with an offset such as -07:00.",
            )),
        }
    }

    fn number(&mut self) -> Result<Data> {
        let start = self.at;
        while self.byte().is_some_and(|c| c.is_ascii_alphanumeric() || b"_+-.".contains(&c)) {
            self.at += 1;
        }
        if self.at == start {
            return Err(self.fail(
                ErrorKind::Syntax,
                start,
                start + 1,
                "Expected a value here.",
                "Write a string in quotes, a number, true or false, an array [...] or an \
                 inline table {...}.",
            ));
        }
        number::parse(&self.src[start..self.at])
            .map_err(|(message, hint)| self.fail(ErrorKind::Number, start, self.at, message, hint))
    }

    fn array(&mut self) -> Result<Vec<Value>> {
        let open = self.at;
        self.at += 1;
        let mut items = Vec::new();
        loop {
            self.skip_blank()?;
            if self.byte() == Some(b']') {
                self.at += 1;
                return Ok(items);
            }
            items.push(self.value()?);
            self.skip_blank()?;
            match self.byte() {
                Some(b',') => self.at += 1,
                Some(b']') => {
                    self.at += 1;
                    return Ok(items);
                }
                _ => {
                    let message = format!(
                        "The array opened on line {} is missing a comma or its ].",
                        self.line_of(open)
                    );
                    let hint = "Separate items with commas and close the array with ].";
                    return Err(self.fail(ErrorKind::Syntax, self.at, self.at + 1, message, hint));
                }
            }
        }
    }

    fn inline_table(&mut self) -> Result<Table> {
        self.at += 1;
        let mut table = Table::new();
        self.skip_space();
        if self.byte() == Some(b'}') {
            self.at += 1;
            return Ok(table);
        }
        loop {
            self.key_value(&mut table)?;
            self.skip_space();
            match self.byte() {
                Some(b',') => self.at += 1,
                Some(b'}') => {
                    self.at += 1;
                    return Ok(table);
                }
                _ => {
                    return Err(self.fail(
                        ErrorKind::Syntax,
                        self.at,
                        self.at + 1,
                        "Expected , or } in this inline table.",
                        "An inline table stays on one line, as in { x = 1, y = 2 }, without a \
                         trailing comma; use a [table] header for longer tables.",
                    ));
                }
            }
        }
    }
}

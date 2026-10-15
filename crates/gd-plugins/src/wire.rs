//! The JSON Lines wire format shared by both process protocols: bounded lines, JSON
//! objects, handshakes, versions and problems.
//!
//! A host starts a plugin program with [`crate::PROTOCOL_FLAG`]; the two exchange JSON
//! objects, one a line, over the child's standard input and output, and the child's
//! standard error is its progress for people. The child speaks first, with its
//! handshake; the host answers with the version it selects, then makes requests, each
//! answered by a response with the request's `id` and a boolean `ok`. Lines are
//! bounded, nesting is shallow, keys are unique, and unknown optional fields are
//! ignored.

use std::io::BufRead;

use gd_json::{Object, Value};

use crate::problem::{self, EXIT_UNKNOWN, Problem};
use crate::{
    BUILD_PROTOCOL_NAME, BUILD_PROTOCOL_VERSION, PROTOCOL_NAME, PROTOCOL_VERSION, Version,
};

/// The deepest nesting of objects and arrays a message may have.
const MAX_NESTING: usize = 16;

/// The first line a plugin program writes: the protocol, the highest minor version of
/// each major version it speaks, the interface version it was built against, what it
/// is, its capabilities (publishers), the snapshot schemas it reads (publishers), and
/// its hooks (build plugins).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Handshake {
    /// [`PROTOCOL_NAME`] or [`BUILD_PROTOCOL_NAME`].
    pub protocol: String,
    /// The protocol versions spoken.
    pub versions: Vec<Version>,
    /// The interface version built against.
    pub api: Version,
    /// The plugin's ID.
    pub id: String,
    /// Its display name.
    pub name: String,
    /// Its product version.
    pub version: String,
    /// A publisher's optional operations.
    pub capabilities: Vec<String>,
    /// The snapshot schemas a publisher reads.
    pub snapshot_schemas: Vec<i64>,
    /// A build plugin's hooks.
    pub hooks: Vec<String>,
}

impl Handshake {
    /// Whether this is a build plugin's handshake.
    pub fn is_build(&self) -> bool {
        self.protocol == BUILD_PROTOCOL_NAME
    }

    pub(crate) fn to_json(&self) -> Value {
        let strings =
            |list: &[String]| Value::Array(list.iter().map(|s| s.as_str().into()).collect());
        object([
            ("protocol", self.protocol.as_str().into()),
            ("versions", Value::Array(self.versions.iter().map(|v| version_json(*v)).collect())),
            ("api", version_json(self.api)),
            ("id", self.id.as_str().into()),
            ("name", self.name.as_str().into()),
            ("version", self.version.as_str().into()),
            ("capabilities", strings(&self.capabilities)),
            (
                "snapshot_schemas",
                Value::Array(self.snapshot_schemas.iter().map(|&n| n.into()).collect()),
            ),
            ("hooks", strings(&self.hooks)),
        ])
    }
}

/// The protocol version a host and the program whose handshake is `h` share: the
/// same major version, at the lower minor version; `None` when they share none.
pub fn select_version(h: &Handshake) -> Option<Version> {
    let host = if h.is_build() { BUILD_PROTOCOL_VERSION } else { PROTOCOL_VERSION };
    h.versions
        .iter()
        .find(|v| v.major == host.major)
        .map(|v| Version::new(v.major, v.minor.min(host.minor)))
}

/// Reads and checks a handshake line.
pub(crate) fn parse_handshake(line: &str) -> Result<Handshake, Problem> {
    let o = parse_message(line)?;
    let protocol = str_field(&o, "protocol").unwrap_or_default();
    if protocol != PROTOCOL_NAME && protocol != BUILD_PROTOCOL_NAME {
        return Err(problem::protocol("The program is not a Guidedog plugin."));
    }
    let mut h = Handshake {
        protocol: protocol.to_string(),
        id: str_field(&o, "id").unwrap_or_default().to_string(),
        name: str_field(&o, "name").unwrap_or_default().to_string(),
        version: str_field(&o, "version").unwrap_or_default().to_string(),
        api: o.get("api").and_then(version_of).unwrap_or_default(),
        ..Handshake::default()
    };
    if !crate::is_valid_id(&h.id) || h.name.is_empty() || h.version.is_empty() {
        return Err(problem::protocol("The handshake has no valid id, name, or version."));
    }
    let Some(versions) = arr_field(&o, "versions") else {
        return Err(problem::protocol("The handshake lists no protocol versions."));
    };
    for item in versions {
        let v = version_of(item)
            .ok_or_else(|| problem::protocol("The handshake lists a malformed version."))?;
        h.versions.push(v);
    }
    let items = arr_field(&o, "capabilities").unwrap_or_default();
    h.capabilities = items.iter().filter_map(Value::as_str).map(str::to_string).collect();
    let items = arr_field(&o, "snapshot_schemas").unwrap_or_default();
    h.snapshot_schemas = items.iter().filter_map(Value::as_i64).collect();
    if h.is_build() {
        h.hooks = build_hooks(&o, &h.id)?;
    }
    Ok(h)
}

fn build_hooks(o: &Object, id: &str) -> Result<Vec<String>, Problem> {
    let Some(items) = arr_field(o, "hooks") else {
        return Err(problem::contract(id, "List the build hooks in the handshake."));
    };
    items
        .iter()
        .map(|item| {
            item.as_str().map(str::to_string).ok_or_else(|| {
                problem::contract(id, "Write hook names as strings: source, html, or assets.")
            })
        })
        .collect()
}

/// The host's `select` request, naming the protocol version it chose.
pub(crate) fn select_request(id: i64, version: Version) -> Value {
    object([("id", id.into()), ("operation", "select".into()), ("version", version_json(version))])
}

pub(crate) fn version_json(v: Version) -> Value {
    object([("major", i64::from(v.major).into()), ("minor", i64::from(v.minor).into())])
}

pub(crate) fn version_of(v: &Value) -> Option<Version> {
    let o = v.as_object()?;
    let major = u16::try_from(int_field(o, "major")?).ok()?;
    let minor = u16::try_from(int_field(o, "minor")?).ok()?;
    Some(Version::new(major, minor))
}

/// A problem as the protocol writes it; no problem is written with empty fields.
pub(crate) fn problem_json(p: Option<&Problem>) -> Value {
    let empty = Problem::default();
    let p = p.unwrap_or(&empty);
    object([
        ("code", p.code.as_str().into()),
        ("title", p.title.as_str().into()),
        ("path", p.path.as_str().into()),
        ("message", p.message.as_str().into()),
        ("hint", p.hint.as_str().into()),
        ("exit", i64::from(p.exit).into()),
    ])
}

/// The problem a protocol object holds; `None` when it is empty. An exit status outside
/// 1 to 125 becomes 1, and a missing code `plugins.remote`.
pub(crate) fn problem_of(o: &Object) -> Option<Problem> {
    let text = |key| str_field(o, key).unwrap_or_default().to_string();
    let mut p = crate::ProblemData {
        code: text("code"),
        title: text("title"),
        path: text("path"),
        message: text("message"),
        hint: text("hint"),
        exit: 0,
    };
    let exit = int_field(o, "exit").unwrap_or(0);
    if exit == 0 && p.code.is_empty() && p.message.is_empty() {
        return None;
    }
    p.exit = if (1..=125).contains(&exit) { exit as i32 } else { 1 };
    if p.code.is_empty() {
        p.code = "plugins.remote".to_string();
    }
    Some(p.into())
}

/// Makes a JSON object of `fields`, in order.
pub(crate) fn object<const N: usize>(fields: [(&str, Value); N]) -> Value {
    let mut o = Object::with_capacity(N);
    for (key, value) in fields {
        o.insert(key.to_string(), value);
    }
    Value::Object(o)
}

/// A response `{"id": id, "ok": true}` with `fields` after it.
pub(crate) fn ok_reply<const N: usize>(id: i64, fields: [(&str, Value); N]) -> Value {
    let mut o = Object::with_capacity(N + 2);
    o.insert("id".to_string(), id.into());
    o.insert("ok".to_string(), true.into());
    for (key, value) in fields {
        o.insert(key.to_string(), value);
    }
    Value::Object(o)
}

/// A refusal `{"id": id, "ok": false, "problem": {...}}`.
pub(crate) fn refusal(id: i64, p: &Problem) -> Value {
    object([("id", id.into()), ("ok", false.into()), ("problem", problem_json(Some(p)))])
}

/// `value` as one line of JSON, with its line break.
pub(crate) fn message_line(value: &Value) -> String {
    let mut line = gd_json::to_string(value);
    line.push('\n');
    line
}

/// Parses one line as a JSON object, refusing nesting deeper than 16 and repeated keys.
pub(crate) fn parse_message(line: &str) -> Result<Object, Problem> {
    let limits = gd_json::Limits { max_depth: MAX_NESTING, ..gd_json::Limits::default() };
    match gd_json::parse_with(line, &limits) {
        Ok(Value::Object(o)) => Ok(o),
        Ok(_) => Err(problem::protocol("A line is not a JSON object.")),
        Err(e) => Err(problem::protocol(&format!("The text is not JSON ({}).", e.message))),
    }
}

/// Reads the next line without its line break (`\n` or `\r\n`). Fails at the end of
/// the input (`plugins.protocol-closed`), on a read error, on text that is not UTF-8,
/// and when the line is over `limit` bytes; the limit holds before anything is kept.
pub(crate) fn read_line(reader: &mut impl BufRead, limit: usize) -> Result<String, Problem> {
    let mut line = Vec::new();
    loop {
        let chunk = reader
            .fill_buf()
            .map_err(|_| problem::protocol("The plugin's output cannot be read."))?;
        if chunk.is_empty() {
            return Err(closed());
        }
        let (take, done) = match chunk.iter().position(|&c| c == b'\n') {
            Some(at) => (at, true),
            None => (chunk.len(), false),
        };
        if take > limit - line.len() {
            return Err(problem::protocol("A line is too long."));
        }
        line.extend_from_slice(&chunk[..take]);
        reader.consume(if done { take + 1 } else { take });
        if done {
            break;
        }
    }
    if line.last() == Some(&b'\r') {
        line.pop();
    }
    String::from_utf8(line).map_err(|_| problem::protocol("The text is not UTF-8."))
}

/// `plugins.protocol-closed`: the program ended its output before it answered.
pub(crate) fn closed() -> Problem {
    Problem::new(
        "plugins.protocol-closed",
        "The plugin program stopped",
        "The plugin program ended its output before it answered.",
    )
    .hint("Run the plugin's own commands to see why it stopped, then try again.")
    .exit(EXIT_UNKNOWN)
}

pub(crate) fn str_field<'a>(o: &'a Object, key: &str) -> Option<&'a str> {
    o.get(key)?.as_str()
}

pub(crate) fn int_field(o: &Object, key: &str) -> Option<i64> {
    o.get(key)?.as_i64()
}

pub(crate) fn bool_field(o: &Object, key: &str) -> Option<bool> {
    o.get(key)?.as_bool()
}

pub(crate) fn obj_field<'a>(o: &'a Object, key: &str) -> Option<&'a Object> {
    o.get(key)?.as_object()
}

pub(crate) fn arr_field<'a>(o: &'a Object, key: &str) -> Option<&'a [Value]> {
    o.get(key)?.as_array().map(Vec::as_slice)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The limit applies even when a newline arrives in the same read, and extra lines
    // remain in the reader.
    #[test]
    fn lines_are_bounded_and_keep_what_follows() {
        let mut input: &[u8] = b"12345\nnext\r\n";
        assert_eq!(read_line(&mut input, 4).unwrap_err().code, "plugins.protocol");
        let mut input: &[u8] = b"12345\nnext\r\n";
        assert_eq!(read_line(&mut input, 5).unwrap(), "12345");
        assert_eq!(read_line(&mut input, 5).unwrap(), "next");
        assert_eq!(read_line(&mut input, 5).unwrap_err().code, "plugins.protocol-closed");
        let mut bad: &[u8] = b"\xff\n";
        assert_eq!(read_line(&mut bad, 5).unwrap_err().code, "plugins.protocol");
    }

    #[test]
    fn protocol_versions_preserve_optional_minor_changes() {
        for versions in [
            vec![Version::new(1, 0)],
            vec![Version::new(1, 7)],
            vec![Version::new(2, 0), Version::new(1, 3)],
        ] {
            let h = Handshake { versions, ..Handshake::default() };
            assert_eq!(select_version(&h), Some(PROTOCOL_VERSION));
        }
        let h = Handshake { versions: vec![Version::new(2, 0)], ..Handshake::default() };
        assert_eq!(select_version(&h), None);
    }

    #[test]
    fn handshakes_round_trip_and_are_checked() {
        let h = Handshake {
            protocol: BUILD_PROTOCOL_NAME.into(),
            versions: vec![BUILD_PROTOCOL_VERSION],
            api: Version::new(1, 0),
            id: "badge".into(),
            name: "Badge".into(),
            version: "1.0.0".into(),
            hooks: vec!["source".into()],
            ..Handshake::default()
        };
        let line = message_line(&h.to_json());
        assert_eq!(parse_handshake(line.trim_end()).unwrap(), h);
        assert!(parse_handshake(r#"{"protocol":"other"}"#).is_err());
        let deep = format!("{}{}", "[".repeat(20), "]".repeat(20));
        assert!(parse_message(&deep).is_err());
        assert!(parse_message(r#"{"a":1,"a":2}"#).is_err());
    }

    #[test]
    fn problems_cross_the_wire() {
        assert_eq!(problem_of(problem_json(None).as_object().unwrap()), None);
        let p = Problem::new("x.y", "T", "M").hint("H").exit(200);
        let back = problem_of(problem_json(Some(&p)).as_object().unwrap()).unwrap();
        assert_eq!(back.exit, 1);
        assert_eq!(back.hint, "H");
    }
}

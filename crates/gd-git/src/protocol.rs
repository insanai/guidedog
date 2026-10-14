//! The messages of a receive-pack exchange: the reference advertisement, the update
//! request and the report.

use std::fs::File;
use std::io::{BufReader, Write};
use std::path::Path;

use crate::pack::{CHUNK, create_private};
use crate::{Error, ErrorKind, Id, digit, parse_id};

/// A reference and the object it names.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Ref {
    /// The reference's full name, such as `refs/heads/gh-pages`.
    pub name: String,
    /// The object it names.
    pub id: Id,
}

/// What receive-pack offers before an update: its references and capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Advertisement {
    /// The references, in the order the server listed them.
    pub refs: Vec<Ref>,
    /// The capabilities, such as `report-status` or `agent=git/2.47.0`.
    pub capabilities: Vec<String>,
}

impl Advertisement {
    /// Returns the ID of the reference `name`, or `None` when the advertisement does not
    /// list it.
    pub fn find(&self, name: &str) -> Option<Id> {
        self.refs.iter().find(|r| r.name == name).map(|r| r.id)
    }

    /// Tells whether the advertisement offers `capability` (bare, or with a `=value`).
    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|c| {
            c == capability || c.strip_prefix(capability).is_some_and(|rest| rest.starts_with('='))
        })
    }
}

/// How receive-pack answered the update of one reference: applied, or why not.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RefStatus {
    /// The reference's full name.
    pub name: String,
    /// Why the update was refused, such as `fetch first`; `None` when it was applied.
    pub refusal: Option<String>,
}

impl RefStatus {
    /// Tells whether the update was applied.
    pub fn is_ok(&self) -> bool {
        self.refusal.is_none()
    }
}

/// Receive-pack's answer to an update: whether it unpacked the pack, and the status of
/// each reference.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Report {
    /// Why receive-pack could not unpack the pack; `None` when it did (`unpack ok`).
    pub unpack_error: Option<String>,
    /// The status of each reference.
    pub refs: Vec<RefStatus>,
}

impl Report {
    /// Tells whether receive-pack unpacked the pack.
    pub fn is_unpacked(&self) -> bool {
        self.unpack_error.is_none()
    }
}

/// One reference update for [`write_request`]: change `reference` from `old` to `new`
/// ([`Id::ZERO`] `old` creates it), sending the pack at `pack`. `agent` names the client,
/// such as `guidedog-pages/0.7.0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Update<'a> {
    /// The reference's full name, such as `refs/heads/gh-pages`.
    pub reference: &'a str,
    /// The ID the reference must still have; [`Id::ZERO`] when it must not exist.
    pub old: Id,
    /// The ID to set.
    pub new: Id,
    /// The pack holding the objects the server lacks.
    pub pack: &'a Path,
    /// The client's name and version.
    pub agent: &'a str,
}

/// Bounds the advertisement and the report the parsers read.
pub const MAX_ADVERTISEMENT: usize = 16 * 1024 * 1024;

enum Packet<'a> {
    Flush,
    Line(&'a str),
}

/// Parses receive-pack's reference advertisement: as smart HTTP sends it (after a
/// `# service=git-receive-pack` line), or as `receive-pack --advertise-refs` writes it.
///
/// Fails with [`ErrorKind::TooLarge`] above [`MAX_ADVERTISEMENT`] bytes and with
/// [`ErrorKind::Protocol`] when the data is not such an advertisement.
pub fn parse_advertisement(data: &[u8]) -> Result<Advertisement, Error> {
    if data.len() > MAX_ADVERTISEMENT {
        return Err(too_large("advertisement"));
    }
    let format = || Error::protocol("The advertisement is not in Git's packet format.");
    let mut rest = data;
    let mut packet = next_packet(&mut rest).ok_or_else(format)?;
    if let Packet::Line(line) = packet
        && line.starts_with("# service=")
    {
        if line != "# service=git-receive-pack" {
            return Err(Error::protocol("The server offered another service."));
        }
        if !matches!(next_packet(&mut rest), Some(Packet::Flush)) {
            return Err(format());
        }
        packet = next_packet(&mut rest).ok_or_else(format)?;
    }
    let mut a = Advertisement::default();
    let mut first = true;
    while let Packet::Line(line) = packet {
        let mut text = line;
        if first {
            let Some((head, caps)) = line.split_once('\0') else {
                return Err(Error::protocol("The advertisement has no capabilities."));
            };
            text = head;
            a.capabilities = caps.split_ascii_whitespace().map(str::to_string).collect();
            first = false;
        }
        let parsed = text.split_once(' ').and_then(|(id, name)| Some((parse_id(id)?, name)));
        let Some((id, name)) = parsed else {
            return Err(Error::protocol("The advertisement lists a malformed reference."));
        };
        if name != "capabilities^{}" {
            a.refs.push(Ref { name: name.to_string(), id });
        }
        packet = next_packet(&mut rest)
            .ok_or_else(|| Error::protocol("The advertisement ends early."))?;
    }
    Ok(a)
}

/// Parses receive-pack's `report-status` answer.
///
/// Fails with [`ErrorKind::TooLarge`] above [`MAX_ADVERTISEMENT`] bytes and with
/// [`ErrorKind::Protocol`] when the data is not such a report.
pub fn parse_report(data: &[u8]) -> Result<Report, Error> {
    if data.len() > MAX_ADVERTISEMENT {
        return Err(too_large("report"));
    }
    let mut rest = data;
    let unpack = match next_packet(&mut rest) {
        Some(Packet::Line(line)) => line.strip_prefix("unpack "),
        _ => None,
    };
    let Some(unpack) = unpack else {
        return Err(Error::protocol("The answer is not a receive-pack report."));
    };
    let unpack_error = (unpack != "ok").then(|| unpack.to_string());
    let mut r = Report { unpack_error, refs: vec![] };
    loop {
        let line = match next_packet(&mut rest) {
            None => return Err(Error::protocol("The report ends early.")),
            Some(Packet::Flush) => break,
            Some(Packet::Line(line)) => line,
        };
        if let Some(name) = line.strip_prefix("ok ") {
            r.refs.push(RefStatus { name: name.to_string(), refusal: None });
        } else if let Some(text) = line.strip_prefix("ng ") {
            let (name, reason) = text.split_once(' ').unwrap_or((text, ""));
            let refusal = Some(reason.to_string());
            r.refs.push(RefStatus { name: name.to_string(), refusal });
        } else {
            return Err(Error::protocol("The report lists a malformed reference."));
        }
    }
    Ok(r)
}

/// Reads the next packet of `rest`: its text without the final line break, or a flush;
/// `None` when `rest` does not start with a whole packet of UTF-8 text.
fn next_packet<'a>(rest: &mut &'a [u8]) -> Option<Packet<'a>> {
    let head = rest.get(..4)?;
    let mut n = 0usize;
    for &c in head {
        n = n << 4 | usize::from(digit(c)?);
    }
    if n == 0 {
        *rest = &rest[4..];
        return Some(Packet::Flush);
    }
    if n < 4 || n > rest.len() {
        return None;
    }
    let text = std::str::from_utf8(&rest[4..n]).ok()?;
    *rest = &rest[n..];
    Some(Packet::Line(text.strip_suffix('\n').unwrap_or(text)))
}

fn too_large(what: &str) -> Error {
    Error::new(
        ErrorKind::TooLarge,
        format!("The {what} is too large."),
        "Check that the remote URL names a Git repository, not another service.",
    )
}

/// Writes to `path`, which must not exist, the request for `update`: the command, a
/// request for a report, and the pack, streamed from its file.
///
/// An update applies only while the reference still has the ID `old`: a compare-and-swap,
/// so a publisher never overwrites a branch someone else moved. Fails with
/// [`ErrorKind::Protocol`] for an empty or too long reference or agent, or one holding
/// whitespace or NUL, and [`ErrorKind::Io`] when a file cannot be read or written.
pub fn write_request(path: &Path, update: &Update<'_>) -> Result<(), Error> {
    let command = update_command(update)?;
    let mut out = create_private(path).map_err(|e| Error::uncreatable(path, &e))?;
    let unwritable = || Error::unwritable(path, "request");
    out.write_all(&command).map_err(|_| unwritable())?;
    let pack = File::open(update.pack).map_err(|e| Error::unreadable(update.pack, &e))?;
    // Stream the pack: a large site never sits in memory whole.
    let mut pack = BufReader::with_capacity(CHUNK, pack);
    std::io::copy(&mut pack, &mut out).map_err(|_| unwritable())?;
    out.sync_all().map_err(|_| unwritable())
}

/// The update command as one packet, followed by a flush.
fn update_command(u: &Update<'_>) -> Result<Vec<u8>, Error> {
    let bad = |s: &str| s.is_empty() || s.contains([' ', '\t', '\r', '\n', '\0']);
    if u.reference.len() > 65_000 || u.agent.len() > 256 || bad(u.reference) || bad(u.agent) {
        return Err(Error::new(
            ErrorKind::Protocol,
            "The reference or agent is not a valid Git protocol field.",
            "Use a branch name without spaces, such as `refs/heads/gh-pages`.",
        ));
    }
    let mut command = Vec::with_capacity(u.reference.len() + u.agent.len() + 128);
    // Writing into a Vec cannot fail. The length is patched in once it is known.
    let _ = writeln!(
        command,
        "0000{} {} {}\0report-status agent={}",
        u.old, u.new, u.reference, u.agent
    );
    let size = command.len();
    let _ = write!(&mut command[..4], "{size:04x}");
    command.extend_from_slice(b"0000");
    Ok(command)
}

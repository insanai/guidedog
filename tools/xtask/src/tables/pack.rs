//! Packing table entries into lines, as many to a line as fit, the way the committed
//! tables are laid out (they carry `#[rustfmt::skip]`, so rustfmt keeps the layout).

/// The widest line of the HTML entity and style tables, rustfmt's `max_width`.
pub(crate) const WIDTH: usize = 99;

/// Appends `items` (each already ending in its comma) to `out`, each line starting with
/// `indent`, items separated by one space, and a line ending before an item that would
/// pass `width` columns. An item longer than a line gets a line of its own.
pub(crate) fn pack<I>(out: &mut String, indent: &str, width: usize, items: I)
where
    I: IntoIterator<Item = String>,
{
    let mut line = String::from(indent);
    for item in items {
        if line.len() > indent.len() {
            if line.len() + 1 + item.len() > width {
                out.push_str(&line);
                out.push('\n');
                line.clear();
                line.push_str(indent);
            } else {
                line.push(' ');
            }
        }
        line.push_str(&item);
    }
    if line.len() > indent.len() {
        out.push_str(&line);
        out.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breaks_lines_before_the_width() {
        let items = (0..30).map(|i| format!("{i:>9},"));
        let mut out = String::new();
        pack(&mut out, "    ", WIDTH, items);
        assert!(out.lines().all(|line| line.len() <= WIDTH));
        assert_eq!(out.lines().count(), 4);
        assert!(out.lines().all(|line| line.starts_with("     ")));
    }
}

# gd-clock cheat sheet

The build's moment, internal to Guidedog (`publish = false`, std only): the time now or
the one `SOURCE_DATE_EPOCH` fixes, civil dates from Unix seconds, and the local time
zone's offset from UTC. Design notes: `docs/manual/internals/crates.rst`.

```rust
let moment = gd_clock::build_time();         // SOURCE_DATE_EPOCH (UTC), or now
let today = moment.local().date.to_string(); // "2026-10-15"
let utc = moment.utc();                      // DateTime { date, hour, minute, second }
let day = gd_clock::Date::from_days(11_016); // 2000-02-29
```

| Item | Does |
|---|---|
| `now()` | the system clock with the local offset; ignores `SOURCE_DATE_EPOCH` |
| `build_time()` | `SOURCE_DATE_EPOCH` when valid (offset 0), otherwise `now()` |
| `Moment { unix, offset_seconds, is_fixed }`, `.utc()`, `.local()` | a moment and its readings |
| `parse_source_date_epoch(text) -> Result<i64, EpochError>` | a whole number in the years 1 to 9999 |
| `source_date_epoch()` | the environment's value, `None` when unset or refused |
| `Date::from_days`, `Date::days`, `DateTime::from_unix`, `DateTime::unix` | Howard Hinnant's civil algorithms |
| `local_offset(unix)` | seconds east of UTC, or `None` (UTC) where the system cannot tell |

The local offset is the only FFI (`src/offset.rs`): `localtime_r` with `struct tm` declared
for 64-bit Linux, Android, Apple systems and the BSDs (only `tm_gmtoff` is read);
`SystemTimeToTzSpecificLocalTime` on Windows; UTC elsewhere (32-bit targets, illumos).

Redesign notes: this crate replaces the copies of the date arithmetic and the two
`localtime_r` bindings that `gd-typst` and `gd-discussion` carried, and the separate
`SOURCE_DATE_EPOCH` readers of `gd-cli` and `gd-pages`. Every reader now applies one rule:
a whole number (white space ignored) whose moment falls in the years 1 to 9999; any other
value falls back to the clock, so a huge value never wraps into a plausible old date.

//! Byte layouts drawn by a codecast: what a frame, a header or a payload is made of, written in a
//! ```` ```bytes Title ```` fenced block inside a part. The script shows one in the side panel
//! with `| bytes` (or `| bytes:Title`, `| bytes:2`) and glows its fields with `!`: field ids are
//! the refs.
//!
//! ```text
//! title: Header                  (or after the fence: ```bytes Header)
//! row: 8                         bytes per row (default 8)
//! endian: le                     default byte order of the number fields (default be)
//! magic  4  ascii  Magic number  one field per line: id, size, optional format, description
//! len    u16       Body length   a number type is a size and a format at once
//! flags  1         Bit 0: zip
//! _      1         Reserved      `_` fields are padding: not refs, drawn grey
//! body   len       The payload   size from an earlier number field (needs a sample)
//! crc    4  hex    CRC32         formats: u8…u64, i8…i64 (le/be suffix), f32, f64, ascii, hex
//! rest   *         Trailer       `*`: whatever the sample has left
//! sample: 49 57 52 31 00 05 …    example bytes, decoded into each field's value
//! ```
//!
//! Never fails: lines it cannot read go to [`Layout::errors`] (`codecast check` lists them).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Size {
    Fixed(usize),
    /// the value of an earlier number field
    Field(String),
    /// whatever the sample has left
    Rest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    Hex,
    Ascii,
    Uint { le: bool },
    Int { le: bool },
    Float { le: bool },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Field {
    /// the ref (`!` glows it); `_` for padding
    pub id: String,
    pub size: Size,
    pub format: Format,
    pub desc: String,
}

impl Field {
    pub fn is_padding(&self) -> bool {
        self.id == "_"
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Layout {
    /// after the fence (```` ```bytes Header ````) or a `title:` line; `| bytes:<title>` picks it
    pub title: Option<String>,
    /// bytes per row
    pub row: usize,
    pub fields: Vec<Field>,
    pub sample: Vec<u8>,
    /// the block as written (for `script::to_text`)
    pub source: String,
    pub errors: Vec<String>,
}

impl Layout {
    pub fn has(&self, id: &str) -> bool {
        id != "_" && self.fields.iter().any(|f| f.id == id)
    }
    /// Field ids, for error messages.
    pub fn ids(&self) -> Vec<&str> {
        self.fields.iter().filter(|f| !f.is_padding()).map(|f| f.id.as_str()).collect()
    }
}

/// A field placed on the bytes: where it starts, how long it is, what the sample says.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Placed {
    pub field: usize,
    /// `None` after a field whose size is not known (variable size, no sample)
    pub offset: Option<usize>,
    pub len: Option<usize>,
    /// decoded from the sample: `5`, `"IWR1"`, `de ad be ef`
    pub value: Option<String>,
}

fn int_type(t: &str) -> Option<(usize, Format)> {
    let (base, le) = match t.strip_suffix("le") {
        Some(b) => (b, Some(true)),
        None => match t.strip_suffix("be") {
            Some(b) => (b, Some(false)),
            None => (t, None),
        },
    };
    let le = le.unwrap_or(false);
    let (kind, bits) = base.split_at(1.min(base.len()));
    let n: usize = bits.parse().ok()?;
    let f = match kind {
        "u" if matches!(n, 8 | 16 | 32 | 64) => Format::Uint { le },
        "i" if matches!(n, 8 | 16 | 32 | 64) => Format::Int { le },
        "f" if matches!(n, 32 | 64) => Format::Float { le },
        _ => return None,
    };
    Some((n / 8, f))
}

/// A format word: `u16`, `u16le`, `i32be`, `f32`, `ascii`, `hex`. `endian` fills in a missing suffix.
fn format_word(w: &str, le_default: bool) -> Option<(Option<usize>, Format)> {
    match w.to_ascii_lowercase().as_str() {
        "hex" | "bytes" => Some((None, Format::Hex)),
        "ascii" | "text" | "str" | "utf8" => Some((None, Format::Ascii)),
        lw => {
            let explicit = lw.ends_with("le") || lw.ends_with("be");
            let (n, f) = int_type(lw)?;
            let f = match (f, explicit) {
                (Format::Uint { .. }, false) => Format::Uint { le: le_default },
                (Format::Int { .. }, false) => Format::Int { le: le_default },
                (Format::Float { .. }, false) => Format::Float { le: le_default },
                (f, _) => f,
            };
            Some((Some(n), f))
        }
    }
}

fn is_id(w: &str) -> bool {
    !w.is_empty() && w.chars().all(|c| c.is_alphanumeric() || c == '_') && !w.chars().next().unwrap().is_ascii_digit()
}

/// Parse the body of a ```` ```bytes ```` block; `title` is the text after the fence word.
pub fn parse(text: &str, title: Option<&str>) -> Layout {
    let mut l = Layout { title: title.map(str::trim).filter(|t| !t.is_empty()).map(String::from), row: 8, source: text.trim_end().to_string(), ..Default::default() };
    // `endian:` may come after fields that already used the default: read it first
    let le = text.lines().rev().find_map(|x| x.trim().strip_prefix("endian:")).map(|v| v.trim().eq_ignore_ascii_case("le") || v.trim().eq_ignore_ascii_case("little")).unwrap_or(false);
    for (ln, raw) in text.lines().enumerate() {
        let t = raw.trim();
        let at = ln + 1;
        if t.is_empty() || t.starts_with("%%") || t.starts_with("//") || t.starts_with('#') {
            continue;
        }
        if let Some(v) = t.strip_prefix("title:") {
            l.title = Some(v.trim().to_string()).filter(|v| !v.is_empty());
            continue;
        }
        if let Some(v) = t.strip_prefix("row:") {
            match v.trim().parse::<usize>() {
                Ok(n) if (1..=64).contains(&n) => l.row = n,
                _ => l.errors.push(format!("line {}: `row:` takes a number of bytes per row, 1 to 64", at)),
            }
            continue;
        }
        if let Some(v) = t.strip_prefix("endian:") {
            if !matches!(v.trim().to_ascii_lowercase().as_str(), "le" | "be" | "little" | "big") {
                l.errors.push(format!("line {}: `endian:` is `le` or `be`", at));
            }
            continue;
        }
        if let Some(v) = t.strip_prefix("sample:") {
            match parse_hex(v) {
                Ok(b) => l.sample.extend(b),
                Err(e) => l.errors.push(format!("line {}: {}", at, e)),
            }
            continue;
        }
        let mut words = t.split_whitespace();
        let (Some(id), Some(size)) = (words.next(), words.next()) else {
            l.errors.push(format!("line {}: a field is `id size [format] description`", at));
            continue;
        };
        if !is_id(id) {
            l.errors.push(format!("line {}: `{}` is not a field id (letters, digits, `_`)", at, id));
            continue;
        }
        if id != "_" && l.has(id) {
            l.errors.push(format!("line {}: field `{}` twice", at, id));
            continue;
        }
        let rest: Vec<&str> = words.collect();
        let mut format = Format::Hex;
        let size = if let Ok(n) = size.parse::<usize>() {
            if n == 0 {
                l.errors.push(format!("line {}: field `{}` has size 0", at, id));
                continue;
            }
            Size::Fixed(n)
        } else if size == "*" {
            Size::Rest
        } else if let Some((Some(n), f)) = format_word(size, le) {
            format = f;
            Size::Fixed(n)
        } else if l.has(size) {
            Size::Field(size.to_string())
        } else {
            l.errors.push(format!("line {}: size `{}` of `{}` is not a number, a type (u16…), `*`, or an earlier field", at, size, id));
            continue;
        };
        let mut desc_words = rest.as_slice();
        if let Some((n, f)) = desc_words.first().and_then(|w| format_word(w, le)) {
            if let (Some(n), Size::Fixed(s)) = (n, &size) {
                if n != *s {
                    l.errors.push(format!("line {}: `{}` is {} bytes, `{}` says {}", at, id, s, desc_words[0], n));
                }
            }
            format = f;
            desc_words = &desc_words[1..];
        }
        l.fields.push(Field { id: id.to_string(), size, format, desc: desc_words.join(" ") });
    }
    if l.fields.is_empty() {
        l.errors.push("no fields (one per line: `id size [format] description`)".into());
    }
    if let Some(i) = l.fields.iter().position(|f| f.size == Size::Rest) {
        if i + 1 != l.fields.len() {
            l.errors.push(format!("`{}` takes the rest (`*`): it must be the last field", l.fields[i].id));
        }
    }
    if !l.sample.is_empty() {
        let (placed, end) = place_inner(&l);
        for p in &placed {
            if p.len.is_none() {
                l.errors.push(format!("sample: no size for `{}` (its size field cannot be read)", l.fields[p.field].id));
            }
        }
        if let Some(end) = end {
            if end > l.sample.len() {
                l.errors.push(format!("sample: {} bytes, the fields need {}", l.sample.len(), end));
            } else if end < l.sample.len() {
                l.errors.push(format!("sample: {} bytes left over after the last field (end with a `*` field?)", l.sample.len() - end));
            }
        }
    }
    l
}

/// `49 57 52 31`, `0x49 0x57`, `49575231`, `49:57:52`.
fn parse_hex(s: &str) -> Result<Vec<u8>, String> {
    let mut digits = String::new();
    for w in s.split(|c: char| c.is_whitespace() || c == ',' || c == ':') {
        let w = w.trim_start_matches("0x").trim_start_matches("0X");
        if w.is_empty() {
            continue;
        }
        if !w.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(format!("`{}` in `sample:` is not hex", w));
        }
        if w.len() % 2 == 1 {
            return Err(format!("`{}` in `sample:` has an odd number of hex digits", w));
        }
        digits.push_str(w);
    }
    Ok((0..digits.len()).step_by(2).map(|i| u8::from_str_radix(&digits[i..i + 2], 16).unwrap()).collect())
}

/// Place the fields on the sample (or on nothing: offsets and sizes stop at the first field of
/// unknown size). Returns the placed fields and where the last one ends, when known.
fn place_inner(l: &Layout) -> (Vec<Placed>, Option<usize>) {
    let mut out: Vec<Placed> = Vec::new();
    let mut off = Some(0usize);
    for (i, f) in l.fields.iter().enumerate() {
        let len = match &f.size {
            Size::Fixed(n) => Some(*n),
            Size::Rest => off.filter(|_| !l.sample.is_empty()).map(|o| l.sample.len().saturating_sub(o)),
            Size::Field(r) => l.fields.iter().position(|g| &g.id == r).and_then(|j| out[j].value.as_ref()).and_then(|v| v.parse::<usize>().ok()),
        };
        let value = match (off, len) {
            (Some(o), Some(n)) if o + n <= l.sample.len() => Some(decode(&l.sample[o..o + n], f.format)),
            _ => None,
        };
        out.push(Placed { field: i, offset: off, len, value });
        off = match (off, len) {
            (Some(o), Some(n)) => Some(o + n),
            _ => None,
        };
    }
    (out, off)
}

/// Where each field sits and what the sample says in it.
pub fn place(l: &Layout) -> Vec<Placed> {
    place_inner(l).0
}

fn decode(b: &[u8], f: Format) -> String {
    // the bytes in little-endian order
    fn arr<const N: usize>(b: &[u8], le: bool) -> [u8; N] {
        let mut a = [0u8; N];
        a.copy_from_slice(&b[..N]);
        if !le {
            a.reverse();
        }
        a
    }
    let num = |le: bool| -> Option<u64> {
        if b.len() > 8 {
            return None;
        }
        let mut a = [0u8; 8];
        for (i, x) in b.iter().enumerate() {
            a[if le { i } else { b.len() - 1 - i }] = *x;
        }
        Some(u64::from_le_bytes(a))
    };
    match f {
        Format::Uint { le } => num(le).map(|v| v.to_string()).unwrap_or_else(|| hex(b)),
        Format::Int { le } => num(le)
            .map(|v| {
                let bits = b.len() * 8;
                let v = if bits < 64 && v >> (bits - 1) & 1 == 1 { (v as i64) - (1i64 << bits) } else { v as i64 };
                v.to_string()
            })
            .unwrap_or_else(|| hex(b)),
        Format::Float { le } => match b.len() {
            4 => f32::from_le_bytes(arr::<4>(b, le)).to_string(),
            8 => f64::from_le_bytes(arr::<8>(b, le)).to_string(),
            _ => hex(b),
        },
        Format::Ascii => format!("\"{}\"", b.iter().map(|&c| if (0x20..0x7f).contains(&c) { c as char } else { '·' }).collect::<String>()),
        Format::Hex => hex(b),
    }
}

/// `de ad be ef`; long runs are cut: `68 65 6c … (12 bytes)`.
pub fn hex(b: &[u8]) -> String {
    if b.len() > 8 {
        format!("{} … ({} bytes)", b[..4].iter().map(|x| format!("{:02x}", x)).collect::<Vec<_>>().join(" "), b.len())
    } else {
        b.iter().map(|x| format!("{:02x}", x)).collect::<Vec<_>>().join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str = "row: 8\nmagic 4 ascii Magic\nlen u16le Body length\nflags 1 Bit 0: zip\n_ 1 Reserved\nbody len ascii The payload\ncrc 4 hex CRC32\nsample: 49 57 52 31 05 00 01 00 68 65 6c 6c 6f de ad be ef\n";

    #[test]
    fn fields_and_sample() {
        let l = parse(HEADER, Some("Header"));
        assert!(l.errors.is_empty(), "{:?}", l.errors);
        assert_eq!(l.title.as_deref(), Some("Header"));
        assert_eq!(l.ids(), vec!["magic", "len", "flags", "body", "crc"]);
        let p = place(&l);
        let v: Vec<(Option<usize>, Option<usize>, Option<&str>)> = p.iter().map(|x| (x.offset, x.len, x.value.as_deref())).collect();
        assert_eq!(v[0], (Some(0), Some(4), Some("\"IWR1\"")));
        assert_eq!(v[1], (Some(4), Some(2), Some("5")));
        assert_eq!(v[4], (Some(8), Some(5), Some("\"hello\"")));
        assert_eq!(v[5], (Some(13), Some(4), Some("de ad be ef")));
    }

    #[test]
    fn numbers_and_endianness() {
        let l = parse("a u16 BE by default\nb i8\nc u32le\nd f32be\nsample: 01 02 ff 04 00 00 00 3f 80 00 00", None);
        assert!(l.errors.is_empty(), "{:?}", l.errors);
        let vals: Vec<String> = place(&l).into_iter().map(|p| p.value.unwrap()).collect();
        assert_eq!(vals, vec!["258", "-1", "4", "1"]);
        let l = parse("endian: le\na u16\nsample: 01 02", None);
        assert_eq!(place(&l)[0].value.as_deref(), Some("513"));
    }

    #[test]
    fn without_sample_offsets_stop_at_a_variable_field() {
        let l = parse("len 2\nbody len\ncrc 4", None);
        assert!(l.errors.is_empty(), "{:?}", l.errors);
        let p = place(&l);
        assert_eq!((p[1].offset, p[1].len), (Some(2), None));
        assert_eq!(p[2].offset, None);
    }

    #[test]
    fn errors_are_collected() {
        let l = parse("9x 2\nlen\na nope\nb 2 u32\nsample: 0g\n", None);
        assert_eq!(l.errors.len(), 5, "{:?}", l.errors);
        let l = parse("a 2\nsample: 01 02 03", None);
        assert!(l.errors[0].contains("1 bytes left over"), "{:?}", l.errors);
        let l = parse("a 4\nsample: 01", None);
        assert!(l.errors[0].contains("need 4"), "{:?}", l.errors);
        let l = parse("a *\nb 1", None);
        assert!(l.errors[0].contains("last field"), "{:?}", l.errors);
    }
}

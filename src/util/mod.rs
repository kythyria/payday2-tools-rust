pub mod ordered_float;
pub mod read_helpers;
pub mod rc_cell;
pub mod parse_helpers;
pub mod subslice;

pub use subslice::Subslice;

use std::fmt::Write;

pub type BoxResult<T> = Result<T, Box<dyn std::error::Error>>;

// Per RFC 8259:
// All Unicode characters may be placed within the
// quotation marks, except for the characters that MUST be escaped:
// quotation mark, reverse solidus, and the control characters (U+0000
// through U+001F).
//
//    escape = %x5C              ; \
//    char = unescaped /
//        escape (
//            %x22 /          ; "    quotation mark  U+0022
//            %x5C /          ; \    reverse solidus U+005C
//            %x2F /          ; /    solidus         U+002F
//            %x62 /          ; b    backspace       U+0008
//            %x66 /          ; f    form feed       U+000C
//            %x6E /          ; n    line feed       U+000A
//            %x72 /          ; r    carriage return U+000D
//            %x74 /          ; t    tab             U+0009
//            %x75 4HEXDIG )  ; uXXXX                U+XXXX

pub fn escape_json_str(what: &str) -> String {
    let mut buffer = String::with_capacity(what.len()+2);
    buffer.push('"');
    for ch in what.chars() {
        if let Some(i) = "\"\\/\x08\x0C\n\r\t".find(ch) {
            buffer.push('\\');
            buffer.push_str(&"\"\\/bfnrt"[i..i+1]);
        }
        else if ch.is_control() || ('\u{E000}'..='\u{F8FF}').contains(&ch) {
            write!(buffer, "\\u{:04X}", ch as u32).unwrap();
        }
        else {
            buffer.push(ch);
        }
    }
    buffer.push('"');
    buffer
}

#[macro_use]
pub mod timeprint {
    #[macro_export]
    macro_rules! eprintln_time {
        ($fmt:literal, $($args:tt)*) => {
            eprintln!(concat!("[{}] ", $fmt), ::chrono::Utc::now().format("%F %H:%M:%S%.3f"), $($args)*)
        };
        ($fmt:literal) => {
            eprintln!(concat!("[{}] ", $fmt), ::chrono::Utc::now().format("%F %H:%M:%S%.3f"))
        };
    }
}

/// Implement the obvious From for a tuple variant with a single member.
macro_rules! variant_from {
    ($en:ident::$var:ident, $t:ty) => {
        impl From<$t> for $en {
            fn from(src: $t) -> $en {
                $en::$var(src)
            }
        }
    }
}

pub struct AsHex<'a>(pub &'a[u8]);
impl<'a> std::fmt::Display for AsHex<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for i in self.0 {
            write!(f, "{:02x}", i)?;
        };
        Ok(())
    }
}

pub fn write_error_chain<O, E>(output: &mut O, e: E) -> std::fmt::Result
where O: std::fmt::Write, E: std::error::Error
{
    writeln!(output, "{}", e)?;
    if let Some(inner) = e.source() {
        write!(output, "because ")?;
        write_error_chain(output, inner)?;
    }
    Ok(())
}

pub const LIB_VERSION: &str = git_version::git_version!();

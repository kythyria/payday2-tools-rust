pub mod ordered_float;
pub mod read_helpers;
pub mod rc_cell;
pub mod binaryreader;

use std::fmt::{Write, Debug};

pub type BoxResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Eq, PartialEq, Copy, Clone, Hash)]
pub struct InvalidDiscriminant {
    pub discriminant: u32
}

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
        write!(f, "[")?;
        for i in self.0 {
            write!(f, " {:02x}", i)?;
        };
        write!(f, " ]")?;
        Ok(())
    }
}

/// Forward [`std::fmt::Debug`] to [`std::fmt::Display`]
pub struct DbgDisplay<T: std::fmt::Display>(pub T);
impl<T: std::fmt::Display> std::fmt::Debug for DbgDisplay<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

pub struct DbgMatrixF64<'m>(pub &'m vek::Mat4<f64>);
impl<'m> std::fmt::Debug for DbgMatrixF64<'m>
//where
//    T: vek::num_traits::Zero + vek::num_traits::One + std::fmt::Debug + std::cmp::PartialEq
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if *self.0 == vek::Mat4::<f64>::identity() { return f.write_str("Identity"); }

        let strs = self.0.map_cols(|c| c.map(|v| format!("{:.8}", v)));
        let lx = [strs.cols.x.x.len(), strs.cols.x.y.len(), strs.cols.x.z.len(), strs.cols.x.w.len()];
        let ly = [strs.cols.y.x.len(), strs.cols.y.y.len(), strs.cols.y.z.len(), strs.cols.y.w.len()];
        let lz = [strs.cols.z.x.len(), strs.cols.z.y.len(), strs.cols.z.z.len(), strs.cols.z.w.len()];
        let lw = [strs.cols.w.x.len(), strs.cols.w.y.len(), strs.cols.w.z.len(), strs.cols.w.w.len()];

        let maxes = [lx.iter().max().unwrap(), ly.iter().max().unwrap(), lz.iter().max().unwrap(), lw.iter().max().unwrap()];

        f.write_str("Mat4 {")?;

        if f.alternate() { f.write_str("\n    ")? } else { f.write_char(' ')? }
        f.write_fmt(format_args!("({1:>0$}  {3:>2$}  {5:>4$}  {7:>6$})",
            maxes[0], strs.cols.x.x, maxes[1], strs.cols.y.x, maxes[2], strs.cols.z.x, maxes[3], strs.cols.w.x))?;
        
        if f.alternate() { f.write_str("\n    ")? } else { f.write_char(' ')? }
        f.write_fmt(format_args!("({1:>0$}  {3:>2$}  {5:>4$}  {7:>6$})",
            maxes[0], strs.cols.x.y, maxes[1], strs.cols.y.y, maxes[2], strs.cols.z.y, maxes[3], strs.cols.w.y))?;
        
        if f.alternate() { f.write_str("\n    ")? } else { f.write_char(' ')? }
        f.write_fmt(format_args!("({1:>0$}  {3:>2$}  {5:>4$}  {7:>6$})",
            maxes[0], strs.cols.x.z, maxes[1], strs.cols.y.z, maxes[2], strs.cols.z.z, maxes[3], strs.cols.w.z))?;
        
        if f.alternate() { f.write_str("\n    ")? } else { f.write_char(' ')? }
        f.write_fmt(format_args!("({1:>0$}  {3:>2$}  {5:>4$}  {7:>6$})",
            maxes[0], strs.cols.x.w, maxes[1], strs.cols.y.w, maxes[2], strs.cols.z.w, maxes[3], strs.cols.w.w))?;
        
        if f.alternate() { f.write_char('\n')? } else { f.write_char(' ')? }
        f.write_str("}")
    }
}

macro_rules! count_tts {
    () => {0};
    ($i:tt) => {1};
    ($i:tt $($r:tt)+) => {(1+count_tts!($($r)*))};
}


macro_rules! simple_debug_table {
    ($item_t:ty, $item_n:literal, 'indices, [$($field:ident $fmt:literal),*$(,)?], $data:expr) => { simple_debug_table!(@t $item_t, $item_n, true, [$($field $fmt),*], $data) };
    ($item_t:ty, $item_n:literal, [$($field:ident $fmt:literal),*$(,)?], $data:expr) => { simple_debug_table!(@t $item_t, $item_n, false, [$($field $fmt),*], $data) };
    (@t $item_t:ty, $item_n:literal, $indices:expr, [$($field:ident $fmt:literal),*], $data:expr) => {
        SimpleDbgTable::<$item_t, {count_tts!($($field)*)}> {
            name: stringify!($item_t),
            header: Some([$(stringify!($field)),*]),
            with_indices: $indices,
            columns: [ $(
                &|item, width, writer| write!(writer, $fmt, item.$field, width)
            ),*],
            data: $data
        }
    };
}

/*
Table is optional header, optional indices, and data cells. For each column there's an optional header string, and a formatter
that takes a width and that row's item. Invoke once with width=0 to get the width, then again to actually fit it
to the table's actual width.
 */
pub struct SimpleDbgTable<'m, T, const N: usize> {
    pub name: &'m str,
    pub header: Option<[&'m str; N]>,
    pub with_indices: bool,
    pub columns: [&'m dyn Fn(&T, usize, &mut dyn Write) -> std::fmt::Result; N],
    pub data: &'m [T]
}
impl<'m, T, const N: usize> std::fmt::Debug for SimpleDbgTable<'m, T, N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{} [\n", self.name))?;

        let mut len_buf = String::new();
        let index_width = if self.with_indices {
            write!(len_buf, "{}", self.data.len()).unwrap();
            len_buf.len()
        }
        else { 0 };

        let mut col_widths = self.header.map_or([0; N], |h| h.map(|c| c.len()));
        for item in self.data {
            for i in 0..N {
                len_buf.clear();
                self.columns[i](item, 0, &mut len_buf).unwrap();
                col_widths[i] = usize::max(col_widths[i], len_buf.len());
            }
        }

        if let Some(header) = self.header {
            f.write_str("    ")?;
            if self.with_indices {
                write!(f, "{0:1$}  ", ' ', index_width)?;
            }
            let mut sep = " ";
            for i in 0..N {
                write!(f, "{2}{0:^1$}", header[i], col_widths[i], sep)?;
                sep = ", ";
            }
            f.write_char('\n')?;
        }

        for (idx, item) in self.data.iter().enumerate() {
            f.write_str("    ")?;
            if self.with_indices {
                write!(f, "{0:>1$}: ", idx, index_width)?;
            }
            let mut sep = "(";
            for i in 0..N {
                f.write_str(sep)?;
                self.columns[i](item, col_widths[i], f)?;
                sep = ", ";
            }
            f.write_str(")\n")?;
        }

        f.write_char(']')
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

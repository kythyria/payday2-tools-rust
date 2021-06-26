use std::{iter::{Copied, Enumerate}, ops::Range};

use nom::Slice;

/// Slice that remembers where it came from
#[derive(Copy, Clone, Debug)]
pub struct Subslice<'a, T> {
    outer: &'a [T],
    inner: &'a [T]
}
impl<'a, T> Subslice<'a, T> {
    pub fn inner(&self) -> &'a [T] {
        self.inner
    }

    pub fn outer(&self) -> &'a [T] {
        self.outer
    }

    pub fn with_inner(&self, n: &'a [T]) -> Self {
        let Range { start: out_start, end: out_end } = self.outer.as_ptr_range();
        let Range { start: in_start, end: in_end } = n.as_ptr_range();
        if in_start < out_start || in_end > out_end {
            panic!("Slice does not fit inside ostensible outer");
        }
        Subslice {
            outer: self.outer,
            inner: n
        }
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn offset(&self) -> usize {
        let base = self.outer.as_ptr() as usize;
        let start = self.inner.as_ptr() as usize;
        start - base
    }

    pub fn offset_of(&self, slice: &[T]) -> usize {
        let base = self.outer.as_ptr() as usize;
        let start = slice.as_ptr() as usize;
        start - base
    }

    pub fn split(&self, mid: usize) -> (Self, Self) {
        let (l, r) = self.inner.split_at(mid);
        let left = Subslice{ outer: self.outer, inner: l };
        let right = Subslice{ outer: self.outer, inner: r };
        (left, right)
    }
}

impl<'a, T: Copy> Subslice<'a, T> {
    pub fn inner_boxed(&self) -> Box<[T]> {
        Box::from(self.inner)
    }
}

impl<'a, T> From<&'a [T]> for Subslice<'a, T> {
    fn from(src: &'a [T]) -> Self {
        Subslice {
            outer: src,
            inner: src
        }
    }
}

impl<'a, T, I> Slice<I> for Subslice<'a, T>
where 
    &'a [T]: Slice<I>,
{
    fn slice(&self, range: I) -> Self {
        Subslice {
            outer: self.outer,
            inner: &self.inner.slice(range)
        }
    }
}

impl<'a, T> nom::InputLength for Subslice<'a, T> {
    fn input_len(&self) -> usize {
        self.len()
    }
}

impl<'a, T> nom::InputTake for Subslice<'a, T> {
    fn take(&self, count: usize) -> Self {
        self.slice(0..count)
    }

    fn take_split(&self, count: usize) -> (Self, Self) {
        let (prefix, suffix) = self.split(count);
        (suffix, prefix)
    }
}

impl<'a, T: Copy> nom::InputIter for Subslice<'a, T> {
    type Item = T;
    type Iter = Enumerate<Self::IterElem>;
    type IterElem = Copied<std::slice::Iter<'a, T>>;

    #[inline]
    fn iter_indices(&self) -> Self::Iter {
        self.iter_elements().enumerate()
    }

    #[inline]
    fn iter_elements(&self) -> Self::IterElem {
        self.inner.iter().copied()
    }

    #[inline]
    fn position<P>(&self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Item) -> bool,
    {
      self.inner.iter().position(|b| predicate(*b))
    }

    #[inline]
    fn slice_index(&self, count: usize) -> Result<usize, nom::Needed> {
        if self.len() >= count {
            Ok(count)
        } else {
            Err(nom::Needed::new(count - self.len()))
        }
    }
}

impl<'a, 'b> nom::Compare<Subslice<'b, u8>> for Subslice<'a, u8>
where
    &'a [u8]: nom::InputIter + nom::InputLength + nom::InputTake
{
    #[inline(always)]
    fn compare(&self, t: Subslice<'b, u8>) -> nom::CompareResult {
        nom::Compare::compare(&self.inner, t.inner)
    }
  
    #[inline(always)]
    fn compare_no_case(&self, t: Subslice<'b, u8>) -> nom::CompareResult {
        nom::Compare::compare_no_case(&self.inner, t.inner)
    }
}

impl<'a, 'b> nom::Compare<&'b [u8]> for Subslice<'a, u8>
where
    &'a [u8]: nom::InputIter + nom::InputLength + nom::InputTake
{
    #[inline(always)]
    fn compare(&self, t: &'b [u8]) -> nom::CompareResult {
        nom::Compare::compare(&self.inner, t)
    }
  
    #[inline(always)]
    fn compare_no_case(&self, t: &'b [u8]) -> nom::CompareResult {
        nom::Compare::compare_no_case(&self.inner, t)
    }
}

impl<'a, 'b, const C: usize> nom::Compare<&'b [u8; C]> for Subslice<'a, u8>
where
    &'a [u8]: nom::InputIter + nom::InputLength + nom::InputTake
{
    #[inline(always)]
    fn compare(&self, t: &'b [u8; C]) -> nom::CompareResult {
        nom::Compare::compare(&self.inner, &t[..])
    }
  
    #[inline(always)]
    fn compare_no_case(&self, t: &'b [u8; C]) -> nom::CompareResult {
        nom::Compare::compare_no_case(&self.inner, &t[..])
    }
}

fn t() -> nom::IResult<Subslice<'static, u8>, Subslice<'static, u8>> {
    let src = b"parser goes brrr";
    let ss = Subslice::from(&src[..]);
    nom::bytes::complete::tag(b"parse")(ss)
}
use std::cell::RefCell;
use std::cmp::*;
use std::rc::Rc;

/// Refcounted pointer to mutable data, with reference equality/ordering.
pub struct RcCell<T: ?Sized>(pub Rc<RefCell<T>>);
impl<T> RcCell<T> {
    pub fn new(contents: T) -> RcCell<T> {
        RcCell(Rc::new(RefCell::new(contents)))
    }
}

impl<T: Default> Default for RcCell<T> {
    fn default() -> RcCell<T> {
        RcCell(Rc::<RefCell<T>>::default())
    }
}

impl<T> PartialEq for RcCell<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_ptr() == other.0.as_ptr()
    }
}
impl<T> Eq for RcCell<T> { }

impl<T> PartialOrd for RcCell<T> { fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(Ord::cmp(self, other)) } }
impl<T> Ord for RcCell<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        let sp = self.0.as_ptr() as usize;
        let op = other.0.as_ptr() as usize;
        Ord::cmp(&sp, &op)
    }
}

impl<T> Clone for RcCell<T> {
    fn clone(&self) -> Self { RcCell(self.0.clone()) }
}

macro_rules! formatters {
    ($($formatter:ident),* ) => {
        $(impl<T: std::fmt::$formatter> std::fmt::$formatter for RcCell<T> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
                let reference = self.0.borrow();
                let refref = &*reference;
                std::fmt::$formatter::fmt(refref, f)
            }
        })*
    }
}
formatters!(Binary, Debug, Display, LowerExp, LowerHex, Octal, Pointer, UpperExp, UpperHex);

impl<T> std::hash::Hash for RcCell<T> {
    fn hash<H: std::hash::Hasher>(&self, hasher: &mut H) {
        let sp = self.0.as_ptr() as usize;
        hasher.write_usize(sp);
    }
}
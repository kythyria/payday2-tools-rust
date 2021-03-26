use std::cell::{Ref, RefCell, RefMut};
use std::cmp::*;
use std::rc::{Rc, Weak};

/// Refcounted pointer to mutable data, with reference equality/ordering.
pub struct RcCell<T: ?Sized>(pub Rc<RefCell<T>>);
impl<T> RcCell<T> {
    pub fn new(contents: T) -> RcCell<T> {
        RcCell(Rc::new(RefCell::new(contents)))
    }

    pub fn downgrade(&self) -> WeakCell<T> {
        WeakCell(Rc::downgrade(&self.0))
    }

    pub fn borrow(&self) -> Ref<T> {
        self.0.borrow()
    }

    pub fn borrow_mut(&self) -> RefMut<T> {
        self.0.borrow_mut()
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

impl<T> std::hash::Hash for RcCell<T> {
    fn hash<H: std::hash::Hasher>(&self, hasher: &mut H) {
        let sp = self.0.as_ptr() as usize;
        hasher.write_usize(sp);
    }
}

/// Weak counterpart of RcCell
pub struct WeakCell<T: ?Sized>(pub Weak<RefCell<T>>);
impl<T> std::hash::Hash for WeakCell<T> {
    fn hash<H: std::hash::Hasher>(&self, hasher: &mut H) {
        let sp = self.0.as_ptr() as usize;
        hasher.write_usize(sp);
    }
}
impl<T> Clone for WeakCell<T> {
    fn clone(&self) -> Self {
        WeakCell(self.0.clone())
    }
}
impl<T> PartialEq for WeakCell<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_ptr() == other.0.as_ptr()
    }
}
impl<T> Eq for WeakCell<T> { }
impl<T> PartialOrd for WeakCell<T> { fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(Ord::cmp(self, other)) } }
impl<T> Ord for WeakCell<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        let sp = self.0.as_ptr() as usize;
        let op = other.0.as_ptr() as usize;
        Ord::cmp(&sp, &op)
    }
}

macro_rules! formatters {
    (RcCell<T>, $($formatter:ident),* ) => {
        $(impl<T: std::fmt::$formatter> std::fmt::$formatter for RcCell<T> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
                let reference = self.0.borrow();
                let refref = &*reference;
                std::fmt::$formatter::fmt(refref, f)
            }
        })*
    };
    (WeakCell<T>, $($formatter:ident),* ) => {
        $(impl<T: std::fmt::$formatter> std::fmt::$formatter for WeakCell<T> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
                if let Some(strong) = self.0.upgrade() {
                    let reference = strong.borrow();
                    let refref = &*reference;
                    std::fmt::$formatter::fmt(refref, f)
                }
                else {
                    f.write_str("(Dead)")
                }
            }
        })*
    };
}
formatters!(RcCell<T>, Binary, Debug, Display, LowerExp, LowerHex, Octal, Pointer, UpperExp, UpperHex);
formatters!(WeakCell<T>, Binary, Debug, Display, LowerExp, LowerHex, Octal, Pointer, UpperExp, UpperHex);
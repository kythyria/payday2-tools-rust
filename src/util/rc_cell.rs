
#[derive(Default)]
pub struct RcCell<T: ?Sized>(pub Rc<RefCell<T>>);
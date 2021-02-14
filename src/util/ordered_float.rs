#[derive(Copy, Clone)]
pub struct OrderedFloat(pub f32);
impl std::convert::From<OrderedFloat> for f32 { fn from(src: OrderedFloat) -> f32 { src.0 } }
impl std::convert::From<&OrderedFloat> for f32 { fn from(src: &OrderedFloat) -> f32 { src.0 } }
impl std::convert::From<f32> for OrderedFloat { fn from(src: f32) -> OrderedFloat { OrderedFloat(src) }}
impl std::convert::From<&f32> for OrderedFloat { fn from(src: &f32) -> OrderedFloat { OrderedFloat(*src) }}
impl std::cmp::PartialEq for OrderedFloat { fn eq(&self, other: &Self) -> bool { self.0 == other.0 || (f32::is_nan(self.0) && f32::is_nan(other.0) ) } }
impl std::cmp::Eq for OrderedFloat { }
impl std::cmp::PartialOrd for OrderedFloat { fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(other)) } }
impl std::cmp::Ord for OrderedFloat {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match std::cmp::PartialOrd::partial_cmp(&self.0, &other.0) {
            Some(o) => o,
            None => if f32::is_nan(self.0) && f32::is_nan(other.0) { std::cmp::Ordering::Equal }
                else if f32::is_nan(self.0) { std::cmp::Ordering::Less }
                else if f32::is_nan(other.0) { std::cmp::Ordering::Greater }
                else { panic!("Apparently {} and {} are incomparable", self.0, other.0) }
        }
    }
}

impl std::hash::Hash for OrderedFloat {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let to_hash = if self.0 == 0.0 || self.0 == -0.0 {
            0.0
        }
        else if f32::is_nan(self.0) {
            f32::NAN
        }
        else {
            self.0
        };
        state.write_u32(f32::to_bits(to_hash))
    }
}

impl std::fmt::Display for OrderedFloat {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        std::fmt::Display::fmt(&self.0, fmt)
    }
}

impl std::fmt::Debug for OrderedFloat {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        std::fmt::Debug::fmt(&self.0, fmt)
    }
}
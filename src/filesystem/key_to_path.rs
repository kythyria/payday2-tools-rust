//! Convert between paths and package database keys, robustly
//! 
//! This is complicated by two possibilities: One, things which are not in the
//! hashlist, and two, things which contain a dot or slash in an inconvenient
//! place. These probably can't ever happen because Overkill generate all this
//! from a real filesystem, but nonetheless, just in case they completely lose
//! their minds, we have this.


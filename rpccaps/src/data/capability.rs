use std::cmp::PartialEq;
use std::ops::{BitAnd,BitAndAssign};

use serde::{Serialize,Deserialize};

/// A Capability are the allowed operations (and thoses that can be shared)
/// over a reference's Authorization.
///
/// # Operators
/// - `BitAnd`: equivalent to `Capability::subset()`
/// - `BitAndAssign`: inplace equivalent to `Capability::subset_inplace()`
///

#[derive(Serialize,Deserialize,PartialEq,Clone,Debug)]
pub struct Capability {
    /// Allowed actions as a bits field.
    pub actions: u64,
    /// Shareable operations as a bits field.
    pub share: u64,
}


impl Capability {
    /// Create a capability ensuring valid fields.
    pub fn new(actions: u64, share: u64) -> Self {
        let (actions, share) = (actions, (share & actions));
        Self { actions, share }
    }

    /// Create an empty capability.
    pub fn empty() -> Self {
        Self { actions: 0, share: 0 }
    }

    /// Create new capability as subset of `self`.
    pub fn subset(&self, actions: u64, share: u64) -> Self {
        let (actions, share) = (actions, (share & actions));
        Self { actions: self.share & actions, share: self.share & share }
    }

    /// Make `self` as subset of itself.
    pub fn subset_inplace(&mut self, actions: u64, share: u64) {
        self.actions = self.share & actions;
        self.share = self.share & share;
    }

    /// Return true if action is allowed
    pub fn is_allowed(&self, action: u64) -> bool {
        (self.actions & action) != 0
    }

    /// Return true if action can be shared
    pub fn is_shareable(&self, action: u64) -> bool {
        (self.share & action) != 0
    }

    /// Return true if capability is empty
    pub fn is_empty(&self) -> bool {
        self.actions == 0 && self.share == 0
    }

    /// Verify that capability has valid values.
    pub fn is_valid(&self) -> bool {
        self.share == self.share & self.actions
    }

    /// Return true if provided `self` is a subset of `cap`.
    ///
    /// A capability B is a subset of capability A if `B.actions in A.shared` and
    /// `B.shared in A.shared`.
    pub fn is_subset(&self, cap: &Self) -> bool {
        // actions: - cap.share & ~self.actions == 0
        //          - cap.actions & ~self.actions == 0
        // share:   - cap.share & self.share < cap.share
        self.actions - (cap.share & cap.actions & self.actions) == 0 &&
        self.share - (cap.share & self.share) == 0
    }
}


impl BitAnd for Capability {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        self.subset(rhs.actions, rhs.share)
    }
}


impl BitAndAssign for Capability {
    fn bitand_assign(&mut self, rhs: Self) {
        self.subset_inplace(rhs.actions, rhs.share)
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subset() {
        let a = Capability::new(0b0110, 0b0011);
        let b = a.subset(0b1110, 0b1100);

        // masks applied
        assert!(a.is_valid());
        assert!(b.is_valid());

        assert_eq!(a, Capability::new(0b0110, 0b0010));
        assert_eq!(b, Capability::new(0b0010, 0b0000));

        // simple subset
        assert!(b.is_subset(&a));
        assert!(!a.is_subset(&b));
    }

    #[test]
    fn test_not_subset() {
        let a = Capability::new(0b1111, 0b0011);
        let b = Capability::new(0b1111, 0b0000);
        assert!(!b.is_subset(&a));
    }

}


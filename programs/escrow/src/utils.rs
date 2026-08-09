use anchor_lang::prelude::*;

use crate::errors::EscrowError;

/// Computes `amount * percent / 100` using checked arithmetic.
pub fn percent_of(amount: u64, percent: u64) -> Result<u64> {
    (amount as u128)
        .checked_mul(percent as u128)
        .and_then(|v| v.checked_div(100))
        .and_then(|v| u64::try_from(v).ok())
        .ok_or_else(|| error!(EscrowError::MathError))
}

pub fn checked_add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or_else(|| error!(EscrowError::Overflow))
}

pub fn checked_sub(a: u64, b: u64) -> Result<u64> {
    a.checked_sub(b).ok_or_else(|| error!(EscrowError::MathError))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_add_overflows() {
        assert!(checked_add(u64::MAX, 1).is_err());
    }

    #[test]
    fn checked_sub_underflows() {
        assert!(checked_sub(0, 1).is_err());
    }

    #[test]
    fn percent_of_computes_split() {
        assert_eq!(percent_of(1_000_000, 20).unwrap(), 200_000);
        assert_eq!(percent_of(1_000_000, 80).unwrap(), 800_000);
    }

    #[test]
    fn percent_of_does_not_overflow_on_max_amount() {
        assert!(percent_of(u64::MAX, 20).is_ok());
    }
}

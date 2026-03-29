//! Parse integral number with unit support

use std::str::FromStr;

/// Number unit types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NumberUnit {
    /// No unit (base number)
    None,
    /// K (thousand)
    Kilo,
    /// M (million)
    Mega,
    /// G (giga)
    Giga,
    /// T (tera)
    Tera,
    /// P (peta)
    Peta,
}

impl NumberUnit {
    /// Get the multiplier for this unit
    pub fn multiplier(&self) -> u64 {
        match self {
            NumberUnit::None => 1,
            NumberUnit::Kilo => 1_000,
            NumberUnit::Mega => 1_000_000,
            NumberUnit::Giga => 1_000_000_000,
            NumberUnit::Tera => 1_000_000_000_000,
            NumberUnit::Peta => 1_000_000_000_000_000,
        }
    }
}

/// Parse integral number with optional unit
pub fn parse_integral_number_unit(s: &str) -> Result<u64, String> {
    let s = s.trim().to_uppercase();

    // Extract unit suffix
    let (num_str, unit) = if s.ends_with('P') {
        (&s[..s.len() - 1], NumberUnit::Peta)
    } else if s.ends_with('T') {
        (&s[..s.len() - 1], NumberUnit::Tera)
    } else if s.ends_with('G') {
        (&s[..s.len() - 1], NumberUnit::Giga)
    } else if s.ends_with('M') {
        (&s[..s.len() - 1], NumberUnit::Mega)
    } else if s.ends_with('K') {
        (&s[..s.len() - 1], NumberUnit::Kilo)
    } else {
        (s.as_str(), NumberUnit::None)
    };

    let base_num = u64::from_str(num_str.trim())
        .map_err(|e| format!("Invalid number '{}': {}", num_str, e))?;

    Ok(base_num * unit.multiplier())
}

/// Parse number with unit to f64
pub fn parse_number_unit_f64(s: &str) -> Result<f64, String> {
    let s = s.trim().to_uppercase();

    let (num_str, unit) = if s.ends_with('P') {
        (&s[..s.len() - 1], NumberUnit::Peta)
    } else if s.ends_with('T') {
        (&s[..s.len() - 1], NumberUnit::Tera)
    } else if s.ends_with('G') {
        (&s[..s.len() - 1], NumberUnit::Giga)
    } else if s.ends_with('M') {
        (&s[..s.len() - 1], NumberUnit::Mega)
    } else if s.ends_with('K') {
        (&s[..s.len() - 1], NumberUnit::Kilo)
    } else {
        (s.as_str(), NumberUnit::None)
    };

    let base_num = f64::from_str(num_str.trim())
        .map_err(|e| format!("Invalid number '{}': {}", num_str, e))?;

    Ok(base_num * unit.multiplier() as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_number() {
        assert_eq!(parse_integral_number_unit("100").unwrap(), 100);
        assert_eq!(parse_integral_number_unit("1000").unwrap(), 1000);
    }

    #[test]
    fn test_parse_with_units() {
        assert_eq!(parse_integral_number_unit("1K").unwrap(), 1_000);
        assert_eq!(parse_integral_number_unit("1M").unwrap(), 1_000_000);
        assert_eq!(parse_integral_number_unit("1G").unwrap(), 1_000_000_000);
        assert_eq!(parse_integral_number_unit("1T").unwrap(), 1_000_000_000_000);
    }

    #[test]
    fn test_parse_case_insensitive() {
        assert_eq!(parse_integral_number_unit("1k").unwrap(), 1_000);
        assert_eq!(parse_integral_number_unit("1m").unwrap(), 1_000_000);
    }

    #[test]
    fn test_unit_multiplier() {
        assert_eq!(NumberUnit::None.multiplier(), 1);
        assert_eq!(NumberUnit::Kilo.multiplier(), 1_000);
        assert_eq!(NumberUnit::Mega.multiplier(), 1_000_000);
    }
}

// this belongs with bin/add-transaction.rs, but needed to be extracted to run doctests

use std::str::FromStr;

use crate::rational::Rational;

#[derive(Debug, PartialEq, Eq)]
pub enum EntryType {
    Credit,
    Debit,
}

/// # Examples
///
/// ```
/// # use sharebill::{parse_arg::{EntryType, parse_arg}, rational::Rational};
/// assert_eq!(parse_arg("test"), None);
/// assert_eq!(parse_arg("jh+2"), Some((EntryType::Credit, "jh", 2u32.into())));
/// assert_eq!(parse_arg("xyz-5/3"), Some((EntryType::Debit, "xyz", Rational::new(5u32, 3u32))));
/// assert_eq!(parse_arg("xyz--5/3"), None);
/// assert_eq!(parse_arg("xyz-+5/3"), None);
/// assert_eq!(parse_arg("xyz+-5/3"), None);
/// assert_eq!(parse_arg("xyz++5/3"), None);
/// ```
pub fn parse_arg<'a>(arg: &'a str) -> Option<(EntryType, &'a str, Rational)> {
    let result = match (arg.find('+'), arg.find('-')) {
        (Some(index), None) => Some((EntryType::Credit, index)),
        (None, Some(index)) => Some((EntryType::Debit, index)),
        _ => None,
    };
    if let Some((entry_type, index)) = result {
        let (account, rest) = arg.split_at(index);
        let (sign, amount) = rest.split_at(1);
        if amount.find(sign) != None {
            // multiple signs
            return None;
        }
        return Rational::from_str(amount)
            .ok()
            .and_then(|amount| Some((entry_type, account, amount)));
    }
    return None;
}

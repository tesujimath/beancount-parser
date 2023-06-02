// TODO remove suppression for dead code warning
#![allow(dead_code)]

use std::iter::once;

use super::*;
use either::Either;
use nom::{
    branch::alt,
    bytes::complete::{escaped_transform, take_while1},
    character::complete::{anychar, one_of, satisfy, space0},
    combinator::{map, map_res, opt, recognize, value},
    multi::{many0, many0_count},
    sequence::{delimited, tuple},
    IResult,
};
use nom_supreme::{
    error::{BaseErrorKind, ErrorTree},
    tag::complete::tag as sym, // beancount grammar has its own tag
};

/// Matches `Account`.
pub fn account(i: &str) -> IResult<&str, Account, ErrorTree<&str>> {
    map_res(
        tuple((
            account_type,
            sym(":"),
            sub_account,
            many0(tuple((sym(":"), sub_account))),
        )),
        |(acc_type, _colon, sub, colon_sub_pairs)| {
            Account::new(
                acc_type,
                once(sub)
                    .chain(colon_sub_pairs.into_iter().map(|(_colon, sub)| sub))
                    .collect(),
            )
        },
    )(i)
}

/// Matches `AccountType`.
pub fn account_type(i: &str) -> IResult<&str, AccountType, ErrorTree<&str>> {
    alt((
        map(sym("Assets"), |_| AccountType::Assets),
        map(sym("Liabilities"), |_| AccountType::Liabilities),
        map(sym("Equity"), |_| AccountType::Equity),
        map(sym("Income"), |_| AccountType::Income),
        map(sym("Expenses"), |_| AccountType::Expenses),
    ))(i)
}

/// Matches `SubAccount`.
pub fn sub_account(i: &str) -> IResult<&str, SubAccount, ErrorTree<&str>> {
    map_res(
        recognize(tuple((
            satisfy(|c| SubAccount::is_valid_initial(&c)),
            many0_count(satisfy(|c| SubAccount::is_valid_subsequent(&c))),
        ))),
        |s: &str| s.parse::<SubAccount>(),
    )(i)
}

/// Matches the `txn` keyword or a flag.
pub fn txn(i: &str) -> IResult<&str, Flag, ErrorTree<&str>> {
    alt((map(sym("txn"), |_| Flag::Asterisk), flag))(i)
}

/// Matches any flag, including asterisk or hash.
pub fn flag(i: &str) -> IResult<&str, Flag, ErrorTree<&str>> {
    alt((
        map(sym("*"), |_| Flag::Asterisk),
        map(sym("#"), |_| Flag::Hash),
        map(sym("!"), |_| Flag::Exclamation),
        map(sym("&"), |_| Flag::Exclamation),
        map(sym("?"), |_| Flag::Question),
        map(sym("%"), |_| Flag::Percent),
        map_res(tuple((sym("'"), anychar)), |(_, c)| {
            FlagLetter::try_from(c).map(Flag::Letter)
        }),
    ))(i)
}

/// Matches zero or more quoted strings, optionally separated by whitespace.
pub fn txn_strings(i: &str) -> IResult<&str, Vec<String>, ErrorTree<&str>> {
    match opt(tuple((string, many0(tuple((space0, string))))))(i)? {
        (i, Some((s1, v))) => Ok((i, once(s1).chain(v.into_iter().map(|(_, s)| s)).collect())),
        (i, None) => Ok((i, Vec::new())),
    }
}

/// Matches zero or more tags or links, optionally separated by whitespace.
pub fn tags_links(i: &str) -> IResult<&str, Vec<Either<Tag, Link>>, ErrorTree<&str>> {
    match opt(tuple((tag_or_link, many0(tuple((space0, tag_or_link))))))(i)? {
        (i, Some((s1, v))) => Ok((i, once(s1).chain(v.into_iter().map(|(_, s)| s)).collect())),
        (i, None) => Ok((i, Vec::new())),
    }
}

/// Matches a tag or a link.
pub fn tag_or_link(i: &str) -> IResult<&str, Either<Tag, Link>, ErrorTree<&str>> {
    use Either::*;
    alt((map(tag, Left), map(link, Right)))(i)
}

/// Matches a tag.
pub fn tag(i: &str) -> IResult<&str, Tag, ErrorTree<&str>> {
    let (i, _) = sym("#")(i)?;
    map(tag_or_link_identifier, Tag::from)(i)
}

/// Matches a link.
pub fn link(i: &str) -> IResult<&str, Link, ErrorTree<&str>> {
    let (i, _) = sym("^")(i)?;
    map(tag_or_link_identifier, Link::from)(i)
}

pub fn tag_or_link_identifier(i: &str) -> IResult<&str, TagOrLinkIdentifier, ErrorTree<&str>> {
    map_res(
        take_while1(|c: char| TagOrLinkIdentifier::is_valid_char(&c)),
        TagOrLinkIdentifier::from_str,
    )(i)
}

pub fn date(i0: &str) -> IResult<&str, NaiveDate, ErrorTree<&str>> {
    fn date_sep(i: &str) -> IResult<&str, (), ErrorTree<&str>> {
        one_of("-/")(i).map(|(s, _)| (s, ()))
    }

    let (i, (year, year_len)) = map_res(take_while1(|c: char| c.is_ascii_digit()), |s: &str| {
        s.parse::<i32>().map(|y| (y, s.len()))
    })(i0)?;
    if year_len < 4 {
        return Err(ParseError::nom_error(
            i0,
            ParseErrorReason::DateMissingCentury,
        ));
    }
    let (i, _) = date_sep(i)?;
    let (i, month) = map_res(take_while1(|c: char| c.is_ascii_digit()), |s: &str| {
        s.parse::<u32>()
    })(i)?;
    let (i, _) = date_sep(i)?;
    let (i, day) = map_res(take_while1(|c: char| c.is_ascii_digit()), |s: &str| {
        s.parse::<u32>()
    })(i)?;

    match NaiveDate::from_ymd_opt(year, month, day) {
        Some(d) => Ok((i, d)),
        None => Err(ParseError::nom_error(i0, ParseErrorReason::DateOutOfRange)),
    }
}

/// Matches a quoted string supporting embedded newlines and character escapes for `\\`, `\"`, `\n`, `\t`.
pub fn string(i: &str) -> IResult<&str, String, ErrorTree<&str>> {
    fn string_content(i: &str) -> IResult<&str, String, ErrorTree<&str>> {
        escaped_transform(
            take_while1(|c| c != '\\' && c != '"'),
            '\\',
            alt((
                value("\\", sym("\\")),
                value("\"", sym("\"")),
                value("\n", sym("n")),
                value("\t", sym("t")),
            )),
        )(i)
    }

    delimited(sym("\""), string_content, sym("\""))(i)
}

#[derive(Debug)]
enum ParseErrorReason {
    DateMissingCentury,
    DateOutOfRange,
}

#[derive(Debug)]
pub struct ParseError {
    reason: ParseErrorReason,
}

impl ParseError {
    fn nom_error(location: &str, reason: ParseErrorReason) -> nom::Err<ErrorTree<&str>> {
        nom::Err::Error(ErrorTree::Base {
            location,
            kind: BaseErrorKind::External(Box::new(ParseError { reason })),
        })
    }
}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.reason {
            ParseErrorReason::DateMissingCentury => write!(f, "date requires century"),
            ParseErrorReason::DateOutOfRange => write!(f, "date out of range"),
        }
    }
}

impl Error for ParseError {}

mod arithmetic;
mod arithmetic_ast;
mod tests;

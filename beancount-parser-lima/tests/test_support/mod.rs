// TODO remove suppression for dead code warning
#![allow(dead_code)]

use ::beancount_parser_lima as lima;
use lima::{Account, AccountName, AccountType, Flag, Spanned, Subaccount};
use rust_decimal::Decimal;
use std::fmt::Debug;
use std::str::FromStr;
use time::{format_description::well_known::Iso8601, Date};

#[macro_export]
macro_rules! check_parse {
    ($source:expr, $expected:expr) => {{
        match ($source, $expected) {
            (source_val, expected_val) => {
                let sources = BeancountSources::from(source_val);
                let parser = BeancountParser::new(&sources);

                fn check<W>(
                    sources: &BeancountSources,
                    parser: &BeancountParser,
                    expected: Vec<Directive>,
                    w: W,
                ) where
                    W: Write + Copy,
                {
                    match parser.parse() {
                        Ok(ParseSuccess { directives, .. }) => {
                            assert_eq!(directives.len(), expected.len(), "directives.len()");
                            for (actual, expected) in directives.iter().zip(expected.iter()) {
                                actual.expect_eq(expected);
                            }
                        }
                        Err(ParseError { errors, .. }) => {
                            sources.write(w, errors).unwrap();
                        }
                    }
                }

                check(&sources, &parser, expected_val, &stderr());
            }
        }
    }};
}

pub trait ExpectEq<Rhs>
where
    Rhs: ?Sized,
{
    fn expect_eq(&self, expected: &Rhs);
}

#[derive(Debug)]
pub struct Directive {
    pub(crate) date: Date,
    pub(crate) metadata: Metadata,
    pub(crate) variant: DirectiveVariant,
}

impl Directive {
    fn new(date: &str, variant: DirectiveVariant) -> Self {
        Directive {
            date: date_from_string(date),
            metadata: Metadata::default(),
            variant,
        }
    }
}

impl<'a> ExpectEq<Directive> for Spanned<lima::Directive<'a>> {
    fn expect_eq(&self, expected: &Directive) {
        use DirectiveVariant::*;

        assert_eq!(self.date().item(), &expected.date);
        match (self.variant(), &expected.variant) {
            (lima::DirectiveVariant::Transaction(variant), Transaction(ref other)) => {
                variant.expect_eq(other);
            }
            _ => panic!("mismatched directive variant"),
        }
    }
}

#[derive(Debug)]
pub enum DirectiveVariant {
    Transaction(Transaction),
    // Price(Price),
    // Balance(Balance),
    // Open(Open),
    // Close(Close),
    // Commodity(Commodity),
    // Pad(Pad),
    // Document(Document),
    // Note(Note),
    // Event(Event),
    // Query(Query),
}

#[derive(Debug)]
pub struct Transaction {
    flag: Flag,
    payee: Option<&'static str>,
    narration: Option<&'static str>,
    postings: Vec<Posting>,
}

pub fn transaction(flag: Flag, postings: Vec<Posting>) -> Transaction {
    Transaction {
        flag,
        payee: None,
        narration: None,
        postings,
    }
}

impl Transaction {
    pub fn date(self, date: &str) -> Directive {
        Directive::new(date, DirectiveVariant::Transaction(self))
    }

    pub fn payee(self, x: &'static str) -> Self {
        Transaction {
            payee: Some(x),
            ..self
        }
    }

    pub fn narration(self, x: &'static str) -> Self {
        Transaction {
            narration: Some(x),
            ..self
        }
    }
}

impl<'a> ExpectEq<Transaction> for lima::Transaction<'a> {
    fn expect_eq(&self, expected: &Transaction) {
        self.flag().expect_eq(&expected.flag);
        self.payee().expect_eq(&expected.payee);
        self.narration().expect_eq(&expected.narration);
        self.postings()
            .collect::<Vec<_>>()
            .expect_eq(&expected.postings);
    }
}

#[derive(Debug)]
struct Price {
    currency: &'static str,
    amount: Amount,
}

impl Price {
    fn new(currency: &'static str, amount: Amount) -> Self {
        Price { currency, amount }
    }
}

#[derive(Debug)]
pub struct Posting {
    flag: Option<Flag>,
    account: Account<'static>,
    amount: Option<Decimal>,
    currency: Option<&'static str>,
    cost_spec: Option<CostSpec>,
    price_annotation: Option<ScopedAmount>,
    metadata: Metadata,
}

pub fn posting(account_str: &'static str) -> Posting {
    Posting {
        flag: None,
        account: account(account_str),
        amount: None,
        currency: None,
        cost_spec: None,
        price_annotation: None,
        metadata: Metadata::default(),
    }
}

impl Posting {
    pub fn flag(self, x: Flag) -> Self {
        Posting {
            flag: Some(x),
            ..self
        }
    }

    pub fn amount(self, x: Decimal) -> Self {
        Posting {
            amount: Some(x),
            ..self
        }
    }

    pub fn currency(self, x: &'static str) -> Self {
        Posting {
            currency: Some(x),
            ..self
        }
    }
}

impl<'a> ExpectEq<Posting> for Spanned<lima::Posting<'a>> {
    fn expect_eq(&self, expected: &Posting) {
        self.flag().expect_eq(&expected.flag);
        self.account().expect_eq(&expected.account);
        self.amount().expect_eq(&expected.amount);
        self.currency().expect_eq(&expected.currency);
        // TODO
        // self.cost_spec().is(cost_spec);
        // self.price_annotation().is(price_annotation);
        // self.metadata().is(metadata);
    }
}

impl<'a> ExpectEq<Vec<Posting>> for Vec<&'a Spanned<lima::Posting<'a>>> {
    fn expect_eq(&self, expected: &Vec<Posting>) {
        assert_eq!(self.len(), expected.len(), "postings.len");
        for (actual, expected) in self.iter().zip(expected.iter()) {
            actual.expect_eq(expected)
        }
    }
}

#[derive(Debug)]
struct Amount {
    number: Decimal,
    currency: &'static str,
}

impl Amount {
    fn new(number: Decimal, currency: &'static str) -> Self {
        Amount { number, currency }
    }
}

impl<'a> ExpectEq<str> for Account<'a> {
    fn expect_eq(&self, expected: &str) {
        assert_eq!(self, &account(expected));
    }
}

fn account(s: &str) -> Account {
    let mut account = s.split(':');
    let account_type_name = account.by_ref().next().unwrap();
    let subaccount = account
        .map(AccountName::try_from)
        .collect::<Result<Subaccount, _>>()
        .unwrap();

    Account::new(
        AccountType::from_str(account_type_name).unwrap(),
        subaccount,
    )
}

fn currency(s: &str) -> lima::Currency {
    lima::Currency::try_from(s).unwrap()
}

// TODO
#[derive(Debug)]
struct CostSpec();
#[derive(Debug)]
struct ScopedAmount();

#[derive(Default, Debug)]
pub struct Metadata();

impl<T, E> ExpectEq<E> for Spanned<T>
where
    T: PartialEq<E> + Debug,
    E: Debug,
{
    fn expect_eq(&self, expected: &E) {
        assert_eq!(self.item(), expected);
    }
}

impl<T, E> ExpectEq<Option<E>> for Option<&Spanned<T>>
where
    T: PartialEq<E> + Debug,
    E: Debug,
{
    fn expect_eq(&self, expected: &Option<E>) {
        match (self, expected) {
            (None, None) => (),
            (Some(actual), Some(ref expected)) => assert_eq!(actual.item(), expected),
            (Some(actual), None) => panic!("expected None got {:?}", actual.item()),
            (None, Some(ref expected)) => panic!("expected {:?} got None", expected),
        }
    }
}

fn date_from_string(s: &str) -> Date {
    time::Date::parse(s, &Iso8601::DEFAULT).unwrap()
}

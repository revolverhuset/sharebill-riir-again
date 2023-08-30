use std::collections::HashMap;

use diesel::prelude::*;
use num::{BigInt, ToPrimitive, Zero};
use sharebill::{
    rational::{sum_rat, Rational},
    schema::{credits, debits},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let conn = &mut sharebill::establish_connection("test.db");

    let cre = credits::table
        .group_by(credits::account)
        .select((credits::account, sum_rat(credits::value)))
        .load::<(String, Rational)>(conn)?;
    let deb = debits::table
        .group_by(debits::account)
        .select((debits::account, sum_rat(debits::value)))
        .load::<(String, Rational)>(conn)?;

    let mut balances = cre
        .into_iter()
        .map(|(account, value)| {
            let v = value.into_inner();
            let v = num::BigRational::from((
                BigInt::from(v.numer().clone()),
                BigInt::from(v.denom().clone()),
            ));
            (account, v)
        })
        .collect::<HashMap<_, _>>();

    for (account, value) in deb {
        let v = value.into_inner();
        let v = num::BigRational::from((
            BigInt::from(v.numer().clone()),
            BigInt::from(v.denom().clone()),
        ));
        *balances.entry(account).or_default() -= v;
    }

    for (account, balance) in balances
        .into_iter()
        .filter(|(_, balance)| !balance.is_zero())
    {
        println!("{account}: {:.2}", balance.to_f64().unwrap());
    }

    Ok(())
}

#![allow(warnings)]
#![feature(with_options)]
use chrono::{DateTime, Duration, NaiveDate, Utc};
use std::{
    collections::HashMap,
    env::temp_dir,
    fs::File,
    io::{Read, Write},
};

use attohttpc::get;
use xml::{
    attribute::OwnedAttribute,
    reader::{EventReader, XmlEvent},
};

fn main() {
    let debug = false;
    let args = std::env::args().skip(1);
    let types: Vec<ArgType> = args.map(what_is).collect();

    use ArgType::*;
    let correct_usage = match &types[..] {
        [Currency(curr_a), Currency(curr_b)] | [Currency(curr_a), Connector, Currency(curr_b)] => {
            Some((1.0, (curr_a, curr_b)))
        }
        [Currency(curr_a), Currency(curr_b), Amount(n)]
        | [Amount(n), Currency(curr_a), Currency(curr_b)]
        | [Currency(curr_a), Connector, Currency(curr_b), Amount(n)]
        | [Amount(n), Currency(curr_a), Connector, Currency(curr_b)] => {
            Some((*n, (curr_a, curr_b)))
        }
        _ => None,
    };
    if let Some((amount, currency_pair)) = correct_usage {
        let mut path = temp_dir();
        // TODO: ensure that this file is unique
        path.push("cur-rs-data.xml");
        let mut file = File::open(path.clone());
        let mut xml = match file {
            Ok(mut file) => {
                if debug {
                    println!("Reading xml from file");
                }
                let mut xml = String::new();
                file.read_to_string(&mut xml);
                // check if file is up to date
                xml
            }
            Err(e) => {
                eprintln!("File::open error: {:?}", e);
                let mut file = File::create(path.clone());
                match file {
                    Ok(mut file) => {
                        if debug {
                            println!("File not found, fetching xml and saving to file");
                        }
                        let xml = get_xml();
                        file.write_all(xml.as_bytes())
                            .unwrap_or_else(|err| panic!("Unable to write to file: {}", err));
                        // get data and put in file
                        xml
                    }
                    Err(e) => {
                        panic!("Error unable to open file {}", e);
                    }
                }
            }
        };
        let (time, currencies) = parse_xml(xml);
        let time = NaiveDate::parse_from_str(&time, "%Y-%m-%d")
            .unwrap_or_else(|e| panic!("Error: unable to parse time from xml, time: {}", e))
            .and_hms(0, 0, 0)
            .into();
        if DateTime::<Utc>::from_utc(time, Utc) < Utc::now() - Duration::days(2) {
            xml = get_xml();
            if let Ok(mut file) = File::with_options().write(true).open(path) {
                file.write(xml.as_bytes())
                    .unwrap_or_else(|e| panic!("Error: unable to write xml to file."));
            } else {
                panic!("Error: unable to open file to write to.");
            }
        } else {
            if debug {
                println!("File is fresh");
            }
        }

        let other_amount = amount * currencies[currency_pair.1] / currencies[currency_pair.0];
        println!(
            "{:.2} {} is {:.2} {}",
            amount, currency_pair.0, other_amount, currency_pair.1
        );

        //println!("xml: {}", xml);
    } else {
        println!("Incorrect usage!");
    }
}

fn get_xml() -> String {
    let url = "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml";
    let response = get(url).send().unwrap();
    let raw_xml = response.text().unwrap();
    return raw_xml;
}

fn parse_xml(raw_xml: String) -> (String, HashMap<String, f64>) {
    let xml = EventReader::from_str(raw_xml.as_str());
    let mut time = String::new();
    let mut currencies = HashMap::new();
    currencies.insert(String::from("EUR"), 1.0);
    for e in xml {
        match e {
            Ok(XmlEvent::StartElement {
                name,
                attributes,
                namespace,
            }) if name.local_name == "Cube" => {
                let mut currency = None;
                let mut rate = None;
                for a in attributes {
                    if a.name.local_name == "time" {
                        time = a.value;
                    } else if a.name.local_name == "currency" {
                        currency = Some(a.value);
                    } else if a.name.local_name == "rate" {
                        rate = Some(a.value.parse::<f64>().unwrap_or_else(|e| {
                            panic!("Error: unable to parse currency rate: {}", e)
                        }));
                    }
                }
                currency.and_then(|c| rate.and_then(|r| currencies.insert(c, r)));
            }
            Err(a) => panic!("Error in parsing xml: {}", a),
            _ => {}
        }
    }
    return (time, currencies);
}

enum ArgType {
    Amount(f64),
    Connector,
    Currency(String),
    Invalid,
}

fn what_is(s: String) -> ArgType {
    return if let Ok(num) = s.replace("_", "").parse::<f64>() {
        ArgType::Amount(num)
    } else if is_connector(&s) {
        ArgType::Connector
    } else if is_currency(&s) {
        ArgType::Currency(s.to_uppercase())
    } else {
        ArgType::Invalid
    };
}

fn is_connector(s: &String) -> bool {
    return match s.as_str() {
        "to" => true,
        "as" => true,
        "in" => true,
        _ => false,
    };
}

// This function dreams of being replaced by a compile time constant hashmap
fn is_currency(s: &String) -> bool {
    return match s.to_uppercase().as_str() {
        "EUR" => true,
        "HKD" => true,
        "THB" => true,
        "ISK" => true,
        "MXN" => true,
        "AUD" => true,
        "RUB" => true,
        "TRY" => true,
        "ZAR" => true,
        "NZD" => true,
        "BRL" => true,
        "CZK" => true,
        "JPY" => true,
        "GBP" => true,
        "CNY" => true,
        "USD" => true,
        "SEK" => true,
        "RON" => true,
        "BGN" => true,
        "ILS" => true,
        "INR" => true,
        "DKK" => true,
        "CAD" => true,
        "CHF" => true,
        "PLN" => true,
        "PHP" => true,
        "MYR" => true,
        "SGD" => true,
        "IDR" => true,
        "NOK" => true,
        "HUF" => true,
        "HRK" => true,
        "KRW" => true,
        _ => false,
    };
}

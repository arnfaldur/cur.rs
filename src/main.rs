#![allow(warnings)]
use chrono::{Date, DateTime, Datelike, Duration, NaiveDate, Utc, Weekday};
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
    let args = std::env::args().skip(1);
    let types: Vec<ArgType> = args.map(what_is).collect();

    use ArgType::*;
    if let Some(Help) = types.get(0) {
        println!(concat!(
            "usage:\n",
            "\tcur <option>\n",
            "\tcur [amount] <from currency> [connector] <to currency>\n",
            "\tcur <from currency> [connector] <to currency> [amount]\n",
            "options:\n",
            "\t-h, --help                      Print this help message\n",
            "\t-l, -c, --list, --currencies    List the available currency symbols\n",
            "connectors:\n",
            "\tas, in, to\n",
        ));
    } else if let Some(Currencies) = types.get(0) {
        // TODO: This shouldn't be hardcoded
        println!(concat!(
            "EUR\n", "HKD\n", "THB\n", "ISK\n", "MXN\n", "AUD\n", "RUB\n", "TRY\n", "ZAR\n",
            "NZD\n", "BRL\n", "CZK\n", "JPY\n", "GBP\n", "CNY\n", "USD\n", "SEK\n", "RON\n",
            "BGN\n", "ILS\n", "INR\n", "DKK\n", "CAD\n", "CHF\n", "PLN\n", "PHP\n", "MYR\n",
            "SGD\n", "IDR\n", "NOK\n", "HUF\n", "HRK\n", "KRW\n",
        ));
    } else {
        let correct_usage = match &types[..] {
            //[Invalid, Invalid] => {}
            [Currency(curr_a), Currency(curr_b)]
            | [Currency(curr_a), Connector, Currency(curr_b)] => Some((1.0, (curr_a, curr_b))),
            [Currency(curr_a), Currency(curr_b), Amount(n)]
            | [Currency(curr_a), Connector, Currency(curr_b), Amount(n)]
            | [Amount(n), Currency(curr_a), Currency(curr_b)]
            | [Amount(n), Currency(curr_a), Connector, Currency(curr_b)] => {
                Some((*n, (curr_a, curr_b)))
            }
            _ => None,
        };
        if let Some((amount, currency_pair)) = correct_usage {
            let currencies = get_currencies();

            let other_amount = amount * currencies[currency_pair.1] / currencies[currency_pair.0];

            print!("{} ", format_number(amount));
            print!("{} is ", currency_pair.0);
            print!("{} ", format_number(other_amount));
            print!("{}", currency_pair.1);
            println!("");
        } else {
            println!("cur: incorrect usage\nTry 'cur -h' for more information.");
        }
    }
}

fn format_number(number: f64) -> String {
    if number >= 1e4 {
        let mut result = format!("{:.0}", number);
        for i in (1..=number.log(1000.0).floor() as usize).rev() {
            result.insert(result.len() - i * 3, ',');
        }
        return result;
    } else {
        return format!("{:.2}", number);
    }
}

fn get_currencies() -> HashMap<String, f64> {
    let mut path = temp_dir();
    path.push("cur-rs-data.xml");
    let mut file = File::open(path.clone());
    let mut xml = match file {
        Ok(mut file) => {
            if cfg!(debug_assertions) {
                println!("Reading xml from file");
            }
            // check if file is up to date
            let mut xml = String::new();
            file.read_to_string(&mut xml);
            xml
        }
        Err(e) => {
            //eprintln!("File::open error: {:?}", e);
            let mut file = File::create(path.clone());
            match file {
                Ok(mut file) => {
                    if cfg!(debug_assertions) {
                        println!("File not found, fetching xml and saving to file");
                    }
                    // get data and put in file
                    let xml = get_xml();
                    file.write_all(xml.as_bytes())
                        .unwrap_or_else(|err| panic!("Unable to write to file: {}", err));
                    xml
                }
                Err(e) => {
                    panic!("Error unable to open file {}", e);
                }
            }
        }
    };
    let (time, currencies) = parse_xml(xml);
    let raw_date_of_data = NaiveDate::parse_from_str(&time, "%Y-%m-%d")
        .unwrap_or_else(|e| panic!("Error: unable to parse time from xml, time: {}", e))
        .into();
    let date_of_data = Date::<Utc>::from_utc(raw_date_of_data, Utc);
    //let date_of_data = DateTime::<Utc>::from_utc(raw_date_of_data, Utc)
    let shift_to_weekday =
        (Utc::today().weekday().number_from_monday() - Weekday::Fri.num_days_from_monday());
    let adjusted_today = Utc::today() - Duration::days(shift_to_weekday.max(0).into());

    if date_of_data < adjusted_today {
        // get data if current data is older than the most recent weekday
        xml = get_xml();
        if let Ok(mut file) = File::options().write(true).open(path) {
            file.write(xml.as_bytes())
                .unwrap_or_else(|e| panic!("Error: unable to write xml to file."));
        } else {
            panic!("Error: unable to open file to write to.");
        }
    } else if cfg!(debug_assertions) {
        println!("File is fresh");
    }
    currencies
}

fn get_xml() -> String {
    // TODO: The ECB datapoint isn't reliable, instead use:
    // https://fiscaldata.treasury.gov/datasets/treasury-reporting-rates-exchange/treasury-reporting-rates-of-exchange
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
    Help,
    Currencies,
    Invalid,
}

fn what_is(s: String) -> ArgType {
    return if let Ok(num) = s.replace("_", "").parse::<f64>() {
        ArgType::Amount(num)
    } else if is_connector(&s) {
        ArgType::Connector
    } else if is_currency(&s) {
        ArgType::Currency(s.to_uppercase())
    } else if is_help(&s) {
        ArgType::Help
    } else if is_currencies(&s) {
        ArgType::Currencies
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

fn is_help(s: &String) -> bool {
    return match s.as_str() {
        "-h" => true,
        "--help" => true,
        _ => false,
    };
}

fn is_currencies(s: &String) -> bool {
    return match s.as_str() {
        "-l" => true,
        "--list" => true,
        "-c" => true,
        "--currencies" => true,
        _ => false,
    };
}

// This function dreams of being replaced by a compile time constant hashmap
fn is_currency(s: &String) -> bool {
    // TODO: This shouldn't be hardcoded
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

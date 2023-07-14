use std::{
    collections::HashMap,
    env::temp_dir,
    fs::File,
    io::{Read, Write},
};

use attohttpc::get;
use chrono::{DateTime, Datelike, Duration, TimeZone, Utc, Weekday};
use xml::reader::{EventReader, XmlEvent};

fn main() {
    use ArgType::*;
    let args = std::env::args().skip(1);
    let boi: Vec<ArgType> = args.map(what_is).collect();
    let long_output = boi.contains(&Long);

    let types: Vec<ArgType> = boi
        .iter()
        .filter(|arg| **arg != Long)
        .map(|e| e.clone())
        .collect();

    if let Some(Help) = types.get(0) {
        println!(concat!(
            "usage:\n",
            "\tcur <option>\n",
            "\tcur [amount] <from currency> [connector] <to currency>\n",
            "\tcur <from currency> [connector] <to currency> [amount]\n",
            "options:\n",
            "\t-h, --help                      Print this help message\n",
            "\t-l, --long                      Longer more human readable output\n",
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

            if long_output {
                print!("{} ", format_number(amount));
                print!("{} is ", currency_pair.0);
                print!("{} ", format_number(other_amount));
                print!("{}", currency_pair.1);
                println!("");
            } else {
                println!("{}", format_number(other_amount));
            }
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
    return File::options()
        .read(true)
        .open(path.clone())
        .map_err(|e| {
            if cfg!(debug_assertions) {
                eprintln!("File::open error: {:?}", e);
            }
        })
        .ok()
        .and_then(|mut file| {
            if cfg!(debug_assertions) {
                println!("Reading xml from file");
            }
            // check if file is up to date
            let mut xml = String::new();
            file.read_to_string(&mut xml)
                .expect("unable to read xml into string");
            let (time, currencies) = parse_xml(xml);
            let fresh = is_data_fresh(Utc::now(), time.clone());

            if cfg!(debug_assertions) && !fresh {
                println!("Data is outdated {}", time);
            }

            fresh.then_some(currencies)
        })
        .map(|c| {
            if cfg!(debug_assertions) {
                println!("File is fresh");
            }
            c
        })
        .unwrap_or_else(|| {
            if cfg!(debug_assertions) {
                println!("Fetching new data and writing to file");
            }
            // get data if current data is older than the most recent weekday
            let xml = fetch_xml();

            File::options()
                .write(true)
                .create(true)
                .open(path)
                .expect("unable to open xml file to write to")
                .write_all(xml.as_bytes())
                .expect("unable to write xml to file");

            parse_xml(xml).1
        });
}

fn is_data_fresh(now: DateTime<Utc>, time: String) -> bool {
    let date_of_data = Utc
        .datetime_from_str(&(time + "T00:00:00"), "%Y-%m-%dT%H:%M:%S")
        .expect("unable to parse time from xml");
    if cfg!(debug_assertions) {
        dbg!(now);
        dbg!(date_of_data);
    }
    let adjusted_now = {
        // Data is updated at roughly 14:00 UTC we give them an extra hour
        let midnight_centered = now - Duration::hours(15);
        // How many days since last friday given it's a weekend?
        let weekend_day = ((midnight_centered.weekday().num_days_from_monday() as i64)
            - Weekday::Fri.num_days_from_monday() as i64)
            .max(0);
        midnight_centered - Duration::days(weekend_day)
    };
    if cfg!(debug_assertions) {
        dbg!(adjusted_now);
        eprintln!("------------------------------")
    }
    return date_of_data.date_naive() >= adjusted_now.date_naive();
}

fn fetch_xml() -> String {
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
                namespace: _,
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

#[derive(PartialEq, Clone)]
enum ArgType {
    Amount(f64),
    Connector,
    Currency(String),
    Help,
    Long,
    Currencies,
    Invalid,
}

fn what_is(s: String) -> ArgType {
    return if let Ok(num) = s.replace("_", "").replace(",", "").parse::<f64>() {
        ArgType::Amount(num)
    } else if is_connector(&s) {
        ArgType::Connector
    } else if is_currency(&s) {
        ArgType::Currency(s.to_uppercase())
    } else if is_help(&s) {
        ArgType::Help
    } else if is_long_flag(&s) {
        ArgType::Long
    } else if is_currencies(&s) {
        ArgType::Currencies
    } else {
        ArgType::Invalid
    };
}

fn is_long_flag(s: &String) -> bool {
    return match s.as_str() {
        "-l" => true,
        "--long" => true,
        _ => false,
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

#[cfg(test)]
mod tests {
    use chrono::offset::Utc;
    use chrono::TimeZone;

    use crate::is_data_fresh;

    #[test]
    fn is_data_fresh_time_threshold_test() {
        let pairs = vec![
            ("2023-07-13T10:00:00", "2023-07-12"),
            ("2023-07-13T11:00:00", "2023-07-12"),
            ("2023-07-13T12:00:00", "2023-07-12"),
            ("2023-07-13T13:00:00", "2023-07-12"),
            ("2023-07-13T14:00:00", "2023-07-12"),
            ("2023-07-13T14:59:59", "2023-07-12"),
            ("2023-07-13T15:00:00", "2023-07-13"),
            ("2023-07-13T16:00:00", "2023-07-13"),
            ("2023-07-13T17:00:00", "2023-07-13"),
            ("2023-07-13T18:00:00", "2023-07-13"),
            ("2023-07-13T19:00:00", "2023-07-13"),
        ];
        for pair in pairs {
            assert!(
                is_data_fresh(
                    Utc.datetime_from_str(pair.0, "%Y-%m-%dT%H:%M:%S").unwrap(),
                    pair.1.to_string()
                ),
                "{} should be fresh at {}",
                pair.1,
                pair.0
            );
        }
    }
    #[test]
    fn is_data_outdated_time_threshold_test() {
        let pairs = vec![
            ("2023-07-13T10:00:00", "2023-07-11"),
            ("2023-07-13T11:00:00", "2023-07-11"),
            ("2023-07-13T12:00:00", "2023-07-11"),
            ("2023-07-13T13:00:00", "2023-07-11"),
            ("2023-07-13T14:00:00", "2023-07-11"),
            ("2023-07-13T14:59:59", "2023-07-11"),
            ("2023-07-13T15:00:00", "2023-07-12"),
            ("2023-07-13T16:00:00", "2023-07-12"),
            ("2023-07-13T17:00:00", "2023-07-12"),
            ("2023-07-13T18:00:00", "2023-07-12"),
            ("2023-07-13T19:00:00", "2023-07-12"),
        ];
        for pair in pairs {
            assert!(
                !is_data_fresh(
                    Utc.datetime_from_str(pair.0, "%Y-%m-%dT%H:%M:%S").unwrap(),
                    pair.1.to_string()
                ),
                "{} should be outdated at {}",
                pair.1,
                pair.0
            );
        }
    }

    #[test]
    fn is_data_fresh_weekend_test() {
        let pairs = vec![
            ("2023-07-06T13:00:00", "2023-07-05"),
            ("2023-07-07T13:00:00", "2023-07-06"),
            ("2023-07-08T13:00:00", "2023-07-07"),
            ("2023-07-09T13:00:00", "2023-07-07"),
            ("2023-07-10T13:00:00", "2023-07-07"),
            ("2023-07-10T14:59:59", "2023-07-07"),
            ("2023-07-10T15:00:00", "2023-07-10"),
            ("2023-07-11T13:00:00", "2023-07-10"),
            ("2023-07-12T13:00:00", "2023-07-11"),
        ];
        for pair in pairs {
            assert!(
                is_data_fresh(
                    Utc.datetime_from_str(pair.0, "%Y-%m-%dT%H:%M:%S").unwrap(),
                    pair.1.to_string()
                ),
                "{} should be fresh at {}",
                pair.1,
                pair.0
            );
        }
    }

    #[test]
    fn is_data_outdated_weekend_test() {
        let pairs = vec![
            ("2023-07-06T13:00:00", "2023-07-04"),
            ("2023-07-07T13:00:00", "2023-07-05"),
            ("2023-07-08T13:00:00", "2023-07-06"),
            ("2023-07-09T13:00:00", "2023-07-06"),
            ("2023-07-10T13:00:00", "2023-07-06"),
            ("2023-07-10T14:59:59", "2023-07-06"),
            ("2023-07-10T15:00:00", "2023-07-07"),
            ("2023-07-11T14:00:00", "2023-07-07"),
            ("2023-07-12T14:00:00", "2023-07-10"),
        ];
        for pair in pairs {
            assert!(
                !is_data_fresh(
                    Utc.datetime_from_str(pair.0, "%Y-%m-%dT%H:%M:%S").unwrap(),
                    pair.1.to_string()
                ),
                "{} should be outdated at {}",
                pair.1,
                pair.0
            );
        }
    }
}

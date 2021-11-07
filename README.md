# Cur.rs
Cur.rs is a minimal tool to get currency exchange rates from the terminal.

It fetches it's data from the European central bank and stores it in a temporary file
chosen by [`std::env:temp_dir`](https://doc.rust-lang.org/std/env/fn.temp_dir.html).
## Installation

Clone the repository and run `cargo install --path ./` in it's root directory

## Usage

The command accepts the following arguments
``` sh
cur <option>
cur [amount] currency [connector] currency
cur currency [connector] currency [amount] 
```
`<option>` is any one of the following flags:
* `-h` or `--help` print a help message.
* `-l`, `-c`, `--list` or `--currencies` list the available currency symbols.

Arguments within `[brackets]` are optional and the `currency` arguments are case-insensitive
[TLA](https://en.wikipedia.org/wiki/Three-letter_acronym).

If `amount` is provided, it is a number that [`amount.parse::<f64>()`](https://doc.rust-lang.org/std/primitive.f64.html#impl-FromStr) understands. Otherwise it's 1.

`connector` can be any of the words `to`, `as` and `in`. This argument has no effect.



## Motivation
I often found myself wanting to know the how much something costs in another currency.
It seemed like a decent practice project so I made it.

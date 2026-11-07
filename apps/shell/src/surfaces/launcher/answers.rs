//! Unit conversions the search answers in place: "5 km in mi", "70f to c",
//! "1.5 GiB to MB". Pure, so the tests run it.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dimension {
    Length,
    Mass,
    Volume,
    Time,
    Data,
    Temperature,
}

/// A unit: its names, the first the one shown, what it measures, and how
/// many of the dimension's base unit one is (for temperature, the base is
/// kelvin and `offset` is added first).
struct Unit {
    names: &'static [&'static str],
    dimension: Dimension,
    factor: f64,
    offset: f64,
}

const fn unit(names: &'static [&'static str], dimension: Dimension, factor: f64) -> Unit {
    Unit {
        names,
        dimension,
        factor,
        offset: 0.0,
    }
}

use Dimension::{Data, Length, Mass, Time, Volume};

const UNITS: &[Unit] = &[
    unit(
        &[
            "mm",
            "millimeter",
            "millimeters",
            "millimetre",
            "millimetres",
        ],
        Length,
        0.001,
    ),
    unit(
        &[
            "cm",
            "centimeter",
            "centimeters",
            "centimetre",
            "centimetres",
        ],
        Length,
        0.01,
    ),
    unit(&["m", "meter", "meters", "metre", "metres"], Length, 1.0),
    unit(
        &["km", "kilometer", "kilometers", "kilometre", "kilometres"],
        Length,
        1000.0,
    ),
    unit(&["in", "inch", "inches", "\""], Length, 0.0254),
    unit(&["ft", "foot", "feet", "'"], Length, 0.3048),
    unit(&["yd", "yard", "yards"], Length, 0.9144),
    unit(&["mi", "mile", "miles"], Length, 1609.344),
    unit(&["nmi", "nautical mile", "nautical miles"], Length, 1852.0),
    unit(&["mg", "milligram", "milligrams"], Mass, 1e-6),
    unit(&["g", "gram", "grams"], Mass, 0.001),
    unit(&["kg", "kilogram", "kilograms", "kilo", "kilos"], Mass, 1.0),
    unit(&["t", "tonne", "tonnes"], Mass, 1000.0),
    unit(&["oz", "ounce", "ounces"], Mass, 0.028_349_523_125),
    unit(&["lb", "lbs", "pound", "pounds"], Mass, 0.453_592_37),
    unit(&["st", "stone", "stones"], Mass, 6.350_293_18),
    unit(
        &[
            "ml",
            "milliliter",
            "milliliters",
            "millilitre",
            "millilitres",
        ],
        Volume,
        0.001,
    ),
    unit(
        &[
            "cl",
            "centiliter",
            "centiliters",
            "centilitre",
            "centilitres",
        ],
        Volume,
        0.01,
    ),
    unit(
        &["dl", "deciliter", "deciliters", "decilitre", "decilitres"],
        Volume,
        0.1,
    ),
    unit(&["l", "liter", "liters", "litre", "litres"], Volume, 1.0),
    unit(
        &["tsp", "teaspoon", "teaspoons"],
        Volume,
        0.004_928_921_593_75,
    ),
    unit(
        &["tbsp", "tablespoon", "tablespoons"],
        Volume,
        0.014_786_764_781_25,
    ),
    unit(&["cup", "cups"], Volume, 0.236_588_236_5),
    unit(&["pt", "pint", "pints"], Volume, 0.473_176_473),
    unit(&["gal", "gallon", "gallons"], Volume, 3.785_411_784),
    unit(
        &["floz", "fl oz", "fluid ounce", "fluid ounces"],
        Volume,
        0.029_573_529_562_5,
    ),
    unit(&["ms", "millisecond", "milliseconds"], Time, 0.001),
    unit(&["s", "sec", "secs", "second", "seconds"], Time, 1.0),
    unit(&["min", "mins", "minute", "minutes"], Time, 60.0),
    unit(&["h", "hr", "hrs", "hour", "hours"], Time, 3600.0),
    unit(&["d", "day", "days"], Time, 86400.0),
    unit(&["wk", "week", "weeks"], Time, 604_800.0),
    unit(&["yr", "year", "years"], Time, 31_557_600.0),
    unit(&["B", "byte", "bytes"], Data, 1.0),
    unit(&["kB", "KB", "kilobyte", "kilobytes"], Data, 1e3),
    unit(&["MB", "megabyte", "megabytes"], Data, 1e6),
    unit(&["GB", "gigabyte", "gigabytes"], Data, 1e9),
    unit(&["TB", "terabyte", "terabytes"], Data, 1e12),
    unit(&["KiB", "kibibyte", "kibibytes"], Data, 1024.0),
    unit(&["MiB", "mebibyte", "mebibytes"], Data, 1_048_576.0),
    unit(&["GiB", "gibibyte", "gibibytes"], Data, 1_073_741_824.0),
    unit(&["TiB", "tebibyte", "tebibytes"], Data, 1_099_511_627_776.0),
    Unit {
        names: &["°C", "c", "celsius", "degc"],
        dimension: Dimension::Temperature,
        factor: 1.0,
        offset: 273.15,
    },
    Unit {
        names: &["°F", "f", "fahrenheit", "degf"],
        dimension: Dimension::Temperature,
        factor: 5.0 / 9.0,
        offset: 459.67,
    },
    Unit {
        names: &["K", "k", "kelvin"],
        dimension: Dimension::Temperature,
        factor: 1.0,
        offset: 0.0,
    },
];

/// The unit called `name`. Byte units keep their case (Mb isn't MB)
/// unless it's all lower case; the rest match in any case.
fn find(name: &str) -> Option<&'static Unit> {
    let name = name.trim().trim_start_matches('°');
    UNITS
        .iter()
        .find(|unit| unit.names.iter().any(|n| n.trim_start_matches('°') == name))
        .or_else(|| {
            let lower = name.to_lowercase();
            UNITS.iter().find(|unit| {
                (unit.dimension != Data || lower == name)
                    && unit
                        .names
                        .iter()
                        .any(|n| n.trim_start_matches('°').to_lowercase() == lower)
            })
        })
}

/// A conversion's answer: what was asked, and what it comes to, both as
/// they're shown.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Converted {
    pub(super) from: String,
    pub(super) to: String,
    /// The number alone, as it's copied.
    pub(super) value: String,
}

/// `query` as a conversion: a number, a unit, "in" or "to" (or "=" or
/// "as"), and another unit of the same kind.
pub(super) fn convert(query: &str) -> Option<Converted> {
    let query = query.trim();
    let split = [" in ", " to ", " as ", " = ", "=", " > "]
        .iter()
        .find_map(|sep| query.split_once(sep))?;
    let (amount, to_name) = split;
    let (number, from_name) = split_number(amount.trim())?;
    let from = find(from_name)?;
    let to = find(to_name)?;
    if from.dimension != to.dimension {
        return None;
    }
    let base = (number + from.offset) * from.factor;
    // rounded clear of the float's dust, so 0 °C isn't -0.000000001
    let value = ((base / to.factor - to.offset) * 1e9).round() / 1e9;
    if !value.is_finite() {
        return None;
    }
    let shown = super::model::format_number(value);
    Some(Converted {
        from: format!("{} {}", super::model::format_number(number), from.names[0]),
        to: format!("{shown} {}", to.names[0]),
        value: shown,
    })
}

/// "5km" or "5 km" as the number and what follows it.
fn split_number(text: &str) -> Option<(f64, &str)> {
    let end = text
        .char_indices()
        .find(|(at, c)| !(c.is_ascii_digit() || *c == '.' || (*at == 0 && *c == '-')))
        .map_or(text.len(), |(at, _)| at);
    let number = text[..end].parse().ok()?;
    let rest = text[end..].trim();
    (!rest.is_empty()).then_some((number, rest))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to(query: &str) -> Option<String> {
        convert(query).map(|answer| answer.to)
    }

    #[test]
    fn lengths_masses_and_volumes_convert() {
        assert_eq!(to("5 km in mi"), Some(String::from("3.106856 mi")));
        assert_eq!(to("1 mile to km"), Some(String::from("1.609344 km")));
        assert_eq!(to("2kg to lb"), Some(String::from("4.409245 lb")));
        assert_eq!(to("1 gal in l"), Some(String::from("3.785412 l")));
    }

    #[test]
    fn temperatures_convert_with_their_offsets() {
        assert_eq!(to("100 c to f"), Some(String::from("212 °F")));
        assert_eq!(to("32°F in °C"), Some(String::from("0 °C")));
        assert_eq!(to("0 K to C"), Some(String::from("-273.15 °C")));
    }

    #[test]
    fn bytes_keep_their_case() {
        assert_eq!(to("1.5 GiB to MB"), Some(String::from("1610.612736 MB")));
        assert_eq!(to("1 TB in GB"), Some(String::from("1000 GB")));
        assert_eq!(to("1 Mb in KB"), None);
        assert_eq!(to("2 mb in kb"), Some(String::from("2000 kB")));
    }

    #[test]
    fn mismatched_or_unknown_units_are_not_answers() {
        assert_eq!(to("5 km in kg"), None);
        assert_eq!(to("5 parsecs in km"), None);
        assert_eq!(to("firefox in km"), None);
        assert_eq!(to("5 km"), None);
    }

    #[test]
    fn the_question_is_shown_too() {
        let answer = convert("3h in min").unwrap();
        assert_eq!(answer.from, "3 h");
        assert_eq!(answer.value, "180");
    }
}

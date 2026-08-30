// Évaluateur Rust de l'AST formula. Fonctions supportées : logique, texte,
// math, date, array (voir SPEC §8).

use std::collections::HashMap;

use serde_json::{json, Value};
use unicode_segmentation::UnicodeSegmentation;

use crate::formula::parser::{Expr, Op};

pub type Context = HashMap<String, Value>;

pub fn eval(expr: &Expr, ctx: &Context) -> Value {
    match expr {
        Expr::Number(n) => json!(n),
        Expr::Str(s) => json!(s),
        Expr::Bool(b) => json!(b),
        Expr::Null => Value::Null,
        Expr::Field(name) => ctx.get(name).cloned().unwrap_or(Value::Null),
        Expr::Unary(op, e) => eval_unary(*op, e, ctx),
        Expr::Binary(op, l, r) => eval_binary(*op, l, r, ctx),
        Expr::Call(name, args) => eval_call(name, args, ctx),
    }
}

fn err(msg: String) -> Value {
    json!(format!("#ERREUR: {msg}"))
}

// chrono panique sur certains format strings invalides (ex. "%" isolé ou
// "{}"). On valide via StrftimeItems et on renvoie une chaîne vide en échec
// plutôt que de faire planter l'application.
fn format_date_safe(dt: chrono::NaiveDateTime, fmt: &str) -> String {
    use chrono::format::{Item, strftime::StrftimeItems};
    if !StrftimeItems::new(fmt).all(|it| it != Item::Error) {
        return String::new();
    }
    dt.format(fmt).to_string()
}

// Le moteur regex (linear-time) est déjà résistant au backtracking
// catastrophique, mais on borne la taille de la compilation pour éviter un
// DoS mémoire sur des patterns pathologiques.
fn compile_regex(pat: &str) -> Result<regex::Regex, String> {
    regex::RegexBuilder::new(pat)
        .size_limit(1 << 20)
        .dfa_size_limit(1 << 20)
        .build()
        .map_err(|_| "pattern regex invalide".into())
}

fn eval_unary(op: Op, e: &Expr, ctx: &Context) -> Value {
    let v = eval(e, ctx);
    match op {
        Op::Neg => json!(-to_num(&v)),
        Op::Not => json!(!to_bool(&v)),
        _ => v,
    }
}

fn eval_binary(op: Op, l: &Expr, r: &Expr, ctx: &Context) -> Value {
    let a = eval(l, ctx);
    let b = eval(r, ctx);
    match op {
        Op::Add => {
            if is_number(&a) && is_number(&b) {
                json!(to_num(&a) + to_num(&b))
            } else {
                json!(format!("{}{}", to_str(&a), to_str(&b)))
            }
        }
        Op::Sub => json!(to_num(&a) - to_num(&b)),
        Op::Mul => json!(to_num(&a) * to_num(&b)),
        Op::Div => {
            let d = to_num(&b);
            if d == 0.0 {
                Value::Null
            } else {
                json!(to_num(&a) / d)
            }
        }
        Op::Mod => {
            let d = to_num(&b);
            if d == 0.0 {
                Value::Null
            } else {
                json!(to_num(&a) % d)
            }
        }
        Op::Concat => json!(format!("{}{}", to_str(&a), to_str(&b))),
        Op::Eq => json!(value_eq(&a, &b)),
        Op::Neq => json!(!value_eq(&a, &b)),
        Op::Lt => json!(compare_lt(&a, &b)),
        Op::Gt => json!(compare_lt(&b, &a)),
        Op::Lte => json!(!compare_lt(&b, &a)),
        Op::Gte => json!(!compare_lt(&a, &b)),
        _ => Value::Null,
    }
}

fn is_number(v: &Value) -> bool {
    matches!(v, Value::Number(_))
}

fn value_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Null, Value::Null) => true,
        (Value::Number(x), Value::Number(y)) => {
            x.as_f64().unwrap_or(0.0) == y.as_f64().unwrap_or(0.0)
        }
        (Value::String(x), Value::String(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        _ => false,
    }
}

fn compare_lt(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64().unwrap_or(0.0) < y.as_f64().unwrap_or(0.0),
        (Value::String(x), Value::String(y)) => x < y,
        (Value::Null, _) => !matches!(b, Value::Null),
        _ => false,
    }
}

fn to_num(v: &Value) -> f64 {
    match v {
        Value::Number(n) => n.as_f64().unwrap_or(0.0),
        Value::String(s) => s.trim().parse::<f64>().unwrap_or(0.0),
        Value::Bool(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        Value::Null => 0.0,
        Value::Array(_) => 0.0,
        _ => 0.0,
    }
}

pub(crate) fn to_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => format_number(n),
        Value::Bool(b) => {
            if *b {
                "true".into()
            } else {
                "false".into()
            }
        }
        Value::Null => String::new(),
        Value::Array(arr) => arr.iter().map(to_str).collect::<Vec<_>>().join(", "),
        Value::Object(_) => String::new(),
    }
}

fn format_number(n: &serde_json::Number) -> String {
    if let Some(f) = n.as_f64() {
        if f.is_finite() && f == f.trunc() && f.abs() < 1e15 {
            format!("{}", f as i64)
        } else {
            f.to_string()
        }
    } else {
        n.to_string()
    }
}

fn to_bool(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
        Value::String(s) => !s.is_empty() && s != "0" && s.to_lowercase() != "false",
        Value::Null => false,
        Value::Array(a) => !a.is_empty(),
        _ => true,
    }
}

fn flatten_numbers(v: &Value, out: &mut Vec<f64>) {
    match v {
        Value::Array(arr) => {
            for x in arr {
                flatten_numbers(x, out);
            }
        }
        Value::Null => {}
        _ => out.push(to_num(v)),
    }
}

fn eval_call(name: &str, args: &[Expr], ctx: &Context) -> Value {
    let values: Vec<Value> = args.iter().map(|a| eval(a, ctx)).collect();
    let n = name.to_ascii_uppercase();

    match n.as_str() {
        "IF" => {
            if values.is_empty() {
                return err("IF nécessite des arguments".into());
            }
            if to_bool(&values[0]) {
                values.get(1).cloned().unwrap_or(Value::Null)
            } else {
                values.get(2).cloned().unwrap_or(Value::Null)
            }
        }
        "SWITCH" => {
            if values.is_empty() {
                return err("SWITCH nécessite des arguments".into());
            }
            let target = &values[0];
            let mut i = 1;
            while i + 1 < values.len() {
                if value_eq(target, &values[i]) {
                    return values[i + 1].clone();
                }
                i += 2;
            }
            // Une valeur restante n'est un défaut explicite que si le nombre
            // d'arguments est impair (target + paires + défaut). Sinon aucun
            // cas ne matche → BLANK (et non la valeur du dernier cas).
            if (values.len() - 1) % 2 == 1 {
                values.last().cloned().unwrap_or(Value::Null)
            } else {
                Value::Null
            }
        }
        "AND" => json!(values.iter().all(to_bool)),
        "OR" => json!(values.iter().any(to_bool)),
        "NOT" => json!(!to_bool(values.first().unwrap_or(&Value::Null))),
        "CONCATENATE" => json!(values.iter().map(to_str).collect::<String>()),
        "LEN" => {
            let s = to_str(values.first().unwrap_or(&Value::Null));
            json!(s.graphemes(true).count() as f64)
        }
        "LOWER" => json!(to_str(values.first().unwrap_or(&Value::Null)).to_lowercase()),
        "UPPER" => json!(to_str(values.first().unwrap_or(&Value::Null)).to_uppercase()),
        "TRIM" => json!(to_str(values.first().unwrap_or(&Value::Null)).trim().to_string()),
        "LEFT" => {
            let s = to_str(values.first().unwrap_or(&Value::Null));
            let n = values.get(1).map(to_num).unwrap_or(1.0).max(0.0) as usize;
            let g: Vec<&str> = s.graphemes(true).collect();
            json!(g.into_iter().take(n).collect::<String>())
        }
        "RIGHT" => {
            let s = to_str(values.first().unwrap_or(&Value::Null));
            let n = values.get(1).map(to_num).unwrap_or(1.0).max(0.0) as usize;
            let g: Vec<&str> = s.graphemes(true).collect();
            let len = g.len();
            json!(g.into_iter().skip(len.saturating_sub(n)).collect::<String>())
        }
        "MID" => {
            let s = to_str(values.first().unwrap_or(&Value::Null));
            let start = values.get(1).map(to_num).unwrap_or(1.0).max(1.0) as usize;
            let len = values.get(2).map(to_num);
            let chars: Vec<char> = s.chars().collect();
            let from = start.saturating_sub(1);
            let out: String = match len {
                Some(l) => {
                    let l = l.max(0.0) as usize;
                    chars.into_iter().skip(from).take(l).collect()
                }
                None => chars.into_iter().skip(from).collect(),
            };
            json!(out)
        }
        "REGEX_MATCH" => {
            let s = to_str(values.first().unwrap_or(&Value::Null));
            let pat = to_str(values.get(1).unwrap_or(&Value::Null));
            match compile_regex(&pat) {
                Ok(re) => json!(re.is_match(&s)),
                Err(e) => err(e),
            }
        }
        "REGEX_EXTRACT" => {
            let s = to_str(values.first().unwrap_or(&Value::Null));
            let pat = to_str(values.get(1).unwrap_or(&Value::Null));
            match compile_regex(&pat) {
                Ok(re) => match re.captures(&s) {
                    Some(caps) => json!(caps.get(1).or_else(|| caps.get(0)).map(|m| m.as_str().to_string()).unwrap_or_default()),
                    None => json!(""),
                },
                Err(e) => err(e),
            }
        }
        "SUM" => {
            let mut nums = Vec::new();
            for v in &values {
                flatten_numbers(v, &mut nums);
            }
            json!(nums.iter().sum::<f64>())
        }
        "AVERAGE" => {
            let mut nums = Vec::new();
            for v in &values {
                flatten_numbers(v, &mut nums);
            }
            if nums.is_empty() {
                Value::Null
            } else {
                json!(nums.iter().sum::<f64>() / nums.len() as f64)
            }
        }
        "MIN" => {
            let mut nums = Vec::new();
            for v in &values {
                flatten_numbers(v, &mut nums);
            }
            nums.into_iter().fold(None, |acc: Option<f64>, x| Some(acc.map_or(x, |a: f64| a.min(x))))
                .map(|x| json!(x))
                .unwrap_or(Value::Null)
        }
        "MAX" => {
            let mut nums = Vec::new();
            for v in &values {
                flatten_numbers(v, &mut nums);
            }
            nums.into_iter().fold(None, |acc: Option<f64>, x| Some(acc.map_or(x, |a: f64| a.max(x))))
                .map(|x| json!(x))
                .unwrap_or(Value::Null)
        }
        "ROUND" => {
            let x = to_num(values.first().unwrap_or(&Value::Null));
            let digits = values.get(1).map(to_num).unwrap_or(0.0) as i32;
            json!(round_to(x, digits))
        }
        "ABS" => json!(to_num(values.first().unwrap_or(&Value::Null)).abs()),
        "MOD" => {
            let a = to_num(values.first().unwrap_or(&Value::Null));
            let b = to_num(values.get(1).unwrap_or(&Value::Null));
            if b == 0.0 {
                Value::Null
            } else {
                json!(a % b)
            }
        }
        "DATETIME_DIFF" => {
            let d1 = parse_date(values.first().unwrap_or(&Value::Null));
            let d2 = parse_date(values.get(1).unwrap_or(&Value::Null));
            let unit = to_str(values.get(2).unwrap_or(&Value::Null)).to_lowercase();
            match (d1, d2) {
                (Some(a), Some(b)) => json!(date_diff(a, b, &unit)),
                _ => Value::Null,
            }
        }
        "DATETIME_FORMAT" => {
            let d = parse_date(values.first().unwrap_or(&Value::Null));
            let fmt = to_str(values.get(1).unwrap_or(&Value::Null));
            let fmt = if fmt.is_empty() { "%Y-%m-%d".into() } else { fmt };
            match d {
                Some(dt) => json!(format_date_safe(dt, &fmt)),
                None => Value::Null,
            }
        }
        "DATEADD" => {
            let d = parse_date(values.first().unwrap_or(&Value::Null));
            let amount = to_num(values.get(1).unwrap_or(&Value::Null)) as i64;
            let unit = to_str(values.get(2).unwrap_or(&Value::Null)).to_lowercase();
            match d {
                Some(dt) => json!(date_add(dt, amount, &unit).format("%Y-%m-%d").to_string()),
                None => Value::Null,
            }
        }
        "TODAY" => json!(chrono::Local::now().date_naive().format("%Y-%m-%d").to_string()),
        "NOW" => json!(chrono::Utc::now().to_rfc3339()),
        "CREATED_TIME" | "LAST_MODIFIED_TIME" => ctx
            .get("created_time")
            .or_else(|| ctx.get("last_modified_time"))
            .cloned()
            .unwrap_or(Value::Null),
        "ARRAYJOIN" => {
            let delim = to_str(values.get(1).unwrap_or(&Value::Null));
            let arr = values.first().unwrap_or(&Value::Null);
            let parts: Vec<String> = match arr {
                Value::Array(a) => a.iter().map(to_str).collect(),
                other => vec![to_str(other)],
            };
            json!(parts.join(if delim.is_empty() { ", " } else { &delim }))
        }
        "ARRAYUNIQUE" => {
            let arr = values.first().unwrap_or(&Value::Null);
            let items: Vec<Value> = match arr {
                Value::Array(a) => {
                    let mut seen: Vec<String> = Vec::new();
                    a.iter().filter(|x| {
                        let k = to_str(x);
                        if seen.contains(&k) {
                            false
                        } else {
                            seen.push(k);
                            true
                        }
                    }).cloned().collect()
                }
                other => vec![other.clone()],
            };
            Value::Array(items)
        }
        "ARRAYCOMPACT" => {
            let arr = values.first().unwrap_or(&Value::Null);
            let items: Vec<Value> = match arr {
                Value::Array(a) => a.iter().filter(|x| !matches!(x, Value::Null) && !to_str(x).is_empty()).cloned().collect(),
                _ => vec![],
            };
            Value::Array(items)
        }
        _ => err(format!("fonction inconnue '{name}'")),
    }
}

fn round_to(x: f64, digits: i32) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let digits = digits.clamp(-100, 100);
    let f = 10f64.powi(digits);
    if !f.is_finite() {
        return x;
    }
    let r = (x * f).round() / f;
    if r.is_finite() {
        r
    } else {
        x
    }
}

fn parse_date(v: &Value) -> Option<chrono::NaiveDateTime> {
    use chrono::{NaiveDate, NaiveDateTime};
    let s = to_str(v).trim().to_string();
    if s.is_empty() {
        return None;
    }
    if let Ok(ts) = s.parse::<i64>() {
        if let Some(dt) = chrono::DateTime::from_timestamp(ts, 0) {
            return Some(dt.naive_utc());
        }
    }
    const FMTS: &[&str] = &[
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%d",
    ];
    for fmt in FMTS {
        if let Ok(dt) = NaiveDateTime::parse_from_str(&s, fmt) {
            return Some(dt);
        }
        if let Ok(d) = NaiveDate::parse_from_str(&s, fmt) {
            return d.and_hms_opt(0, 0, 0);
        }
    }
    None
}

fn date_diff(a: chrono::NaiveDateTime, b: chrono::NaiveDateTime, unit: &str) -> f64 {
    let secs = (a - b).num_seconds() as f64;
    match unit {
        "second" | "seconds" => secs,
        "minute" | "minutes" => secs / 60.0,
        "hour" | "hours" => secs / 3600.0,
        "day" | "days" => secs / 86400.0,
        "week" | "weeks" => secs / 604800.0,
        "month" | "months" => secs / (86400.0 * 30.44),
        "year" | "years" => secs / (86400.0 * 365.25),
        _ => secs / 86400.0,
    }
}

fn date_add(
    d: chrono::NaiveDateTime,
    amount: i64,
    unit: &str,
) -> chrono::NaiveDateTime {
    use chrono::Duration;
    match unit {
        "day" | "days" => d + Duration::days(amount),
        "hour" | "hours" => d + Duration::hours(amount),
        "minute" | "minutes" => d + Duration::minutes(amount),
        "second" | "seconds" => d + Duration::seconds(amount),
        "week" | "weeks" => d + Duration::weeks(amount),
        "month" | "months" => add_months(d, amount),
        "year" | "years" => add_months(d, amount * 12),
        _ => d + Duration::days(amount),
    }
}

fn add_months(d: chrono::NaiveDateTime, months: i64) -> chrono::NaiveDateTime {
    use chrono::Months;
    if months >= 0 {
        d.checked_add_months(Months::new(months as u32)).unwrap_or(d)
    } else {
        d.checked_sub_months(Months::new(months.unsigned_abs() as u32)).unwrap_or(d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formula::parser::parse;

    fn ev(expr: &str, ctx: &Context) -> Value {
        let e = parse(expr).unwrap();
        eval(&e, ctx)
    }

    #[test]
    fn if_logic() {
        let mut c = Context::new();
        c.insert("Montant".into(), json!(100.0));
        assert_eq!(ev("IF({Montant} > 50, 'Elevé', 'Bas')", &c), json!("Elevé"));
        assert_eq!(ev("IF({Montant} > 50, 'Elevé', 'Bas')", &c), json!("Elevé"));
    }

    #[test]
    fn math_and_text() {
        let mut c = Context::new();
        c.insert("Montant".into(), json!(100.0));
        c.insert("Nom".into(), json!("Alice"));
        assert_eq!(ev("SUM(1, 2, 3)", &c), json!(6.0));
        assert_eq!(ev("AVERAGE(1, 2, 3)", &c), json!(2.0));
        assert_eq!(ev("ROUND(2.567, 2)", &c), json!(2.57));
        assert_eq!(ev("CONCATENATE({Nom}, '-', 42)", &c), json!("Alice-42"));
        assert_eq!(ev("LEN('hello')", &c), json!(5.0));
        assert_eq!(ev("UPPER('abc')", &c), json!("ABC"));
        assert_eq!(ev("LEFT('hello', 2)", &c), json!("he"));
        assert_eq!(ev("RIGHT('hello', 2)", &c), json!("lo"));
        assert_eq!(ev("MID('hello', 2, 3)", &c), json!("ell"));
        assert_eq!(ev("REGEX_MATCH('abc123', '[0-9]+')", &c), json!(true));
    }

    #[test]
    fn arrays() {
        let mut c = Context::new();
        c.insert("List".into(), json!(["a", "b", "c"]));
        c.insert("Dup".into(), json!(["x", "x", "y"]));
        assert_eq!(ev("ARRAYJOIN({List})", &c), json!("a, b, c"));
        assert_eq!(ev("ARRAYJOIN({List}, '|')", &c), json!("a|b|c"));
        assert_eq!(ev("ARRAYUNIQUE({Dup})", &c), json!(["x", "y"]));
    }

    #[test]
    fn dates() {
        let c = Context::new();
        assert_eq!(ev("DATETIME_DIFF('2024-01-10', '2024-01-01', 'days')", &c), json!(9.0));
        assert_eq!(ev("DATEADD('2024-01-01', 5, 'days')", &c), json!("2024-01-06"));
        assert!(ev("DATETIME_FORMAT('2024-01-01', '%d/%m/%Y')", &c).as_str().is_some());
    }

    #[test]
    fn datetime_format_invalid_does_not_panic() {
        let c = Context::new();
        // Format invalide (spécificateur incomplet / inconnu) : ne panique pas,
        // renvoie "".
        let v = ev("DATETIME_FORMAT('2024-01-01', '%')", &c);
        assert_eq!(v, json!(""));
        let v2 = ev("DATETIME_FORMAT('2024-01-01', '%Q')", &c);
        assert_eq!(v2, json!(""));
        // "{}" n'est pas un code chrono : sorti littéralement, pas de panic.
        let v3 = ev("DATETIME_FORMAT('2024-01-01', '{}')", &c);
        assert_eq!(v3, json!("{}"));
    }

    #[test]
    fn regex_invalid_or_huge_returns_error() {
        let c = Context::new();
        // Pattern invalide : erreur retournée au lieu d'un crash.
        let v = ev("REGEX_MATCH('abc', '[')", &c);
        assert!(v.as_str().unwrap_or("").contains("#ERREUR"));
        // Pattern pathologique de grande taille : refusé (limite mémoire).
        let pat = format!("REGEX_EXTRACT('abc', '{}')", "a".repeat(300_000));
        let v2 = ev(&pat, &c);
        assert!(v2.as_str().unwrap_or("").contains("#ERREUR"));
    }

    #[test]
    fn switch_and_concat() {
        let mut c = Context::new();
        c.insert("St".into(), json!("A"));
        assert_eq!(ev("SWITCH({St}, 'A', 'OK', 'autre')", &c), json!("OK"));
        assert_eq!(ev("'a' & 'b'", &c), json!("ab"));
    }
}

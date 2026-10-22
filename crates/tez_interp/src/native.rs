//! Implementations of the prelude's `extern fn`s.

use std::rc::Rc;

use num_bigint::BigInt;
use num_traits::{FromPrimitive, Signed, ToPrimitive, Zero};
use tez_diag::Code;
use tez_syntax::ByteSpan;

use crate::interp::{Interp, R, big_to_usize, rt};
use crate::value::{ListVal, Value, fmt_float, values_cmp, values_eq};

const MAX_LIST: usize = 50_000_000;

fn list<'a, 'p>(v: &'a Value<'p>, span: ByteSpan) -> R<&'a ListVal<'p>> {
    match v {
        Value::List(l) => Ok(l),
        _ => Err(rt(Code::Internal, "expected a list", span)),
    }
}

fn string<'a>(v: &'a Value<'_>, span: ByteSpan) -> R<&'a str> {
    match v {
        Value::Str(s) => Ok(s),
        _ => Err(rt(Code::Internal, "expected a string", span)),
    }
}

fn big(v: &Value<'_>, span: ByteSpan) -> R<BigInt> {
    v.to_big()
        .ok_or_else(|| rt(Code::Internal, "expected an integer", span))
}

/// The number of characters. An ASCII text (checked a word at a time) has one byte per character.
fn char_count(s: &str) -> usize {
    if s.is_ascii() {
        s.len()
    } else {
        s.chars().count()
    }
}

/// The characters `from..to` (the caller has checked `from <= to <= char_count(s)`), without copying the others.
fn char_slice(s: &str, from: usize, to: usize) -> &str {
    if s.is_ascii() {
        return s.get(from..to).unwrap_or("");
    }
    let mut at = s
        .char_indices()
        .map(|(b, _)| b)
        .chain(std::iter::once(s.len()));
    let start = at.nth(from).unwrap_or(s.len());
    let end = if to == from {
        start
    } else {
        at.nth(to - from - 1).unwrap_or(s.len())
    };
    s.get(start..end).unwrap_or("")
}

/// `-12`, `3.5`, `1e9`, `-2.5E-3`: digits with an optional fraction and exponent, nothing else (no `+`, no `.5`, no `inf`).
fn parse_float(s: &str) -> Option<f64> {
    let rest = s.strip_prefix('-').unwrap_or(s);
    let (mantissa, exponent) = match rest.find(['e', 'E']) {
        Some(i) => (&rest[..i], Some(&rest[i + 1..])),
        None => (rest, None),
    };
    let digits = |t: &str| !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit());
    let (whole, frac) = match mantissa.split_once('.') {
        Some((w, f)) => (w, Some(f)),
        None => (mantissa, None),
    };
    let signed = |t: &str| digits(t.strip_prefix(['+', '-']).unwrap_or(t));
    if !digits(whole) || frac.is_some_and(|f| !digits(f)) || exponent.is_some_and(|e| !signed(e)) {
        return None;
    }
    s.parse::<f64>().ok().filter(|f| f.is_finite())
}

fn float(v: &Value<'_>, span: ByteSpan) -> R<f64> {
    match v {
        Value::Float(f) => Ok(*f),
        _ => Err(rt(Code::Internal, "expected a float", span)),
    }
}

fn new_list<'p>(v: Vec<Value<'p>>) -> Value<'p> {
    Value::List(ListVal::new(v))
}

fn str_val<'p>(s: impl Into<Rc<str>>) -> Value<'p> {
    Value::Str(s.into())
}

fn int_val<'p>(n: usize) -> Value<'p> {
    Value::Int(i64::try_from(n).unwrap_or(i64::MAX))
}

fn truth(v: &Value<'_>, span: ByteSpan) -> R<bool> {
    match v {
        Value::Bool(b) => Ok(*b),
        _ => Err(rt(Code::Internal, "expected a Bool", span)),
    }
}

fn float_to_int<'p>(f: f64, span: ByteSpan) -> R<Value<'p>> {
    BigInt::from_f64(f)
        .map(Value::from_big)
        .ok_or_else(|| rt(Code::Resource, "float is not finite", span))
}

pub(crate) fn call<'p>(
    it: &mut Interp<'p>,
    name: &str,
    a: Vec<Value<'p>>,
    span: ByteSpan,
) -> R<Value<'p>> {
    let arg = |i: usize| -> R<&Value<'p>> {
        a.get(i).ok_or_else(|| {
            rt(
                Code::Internal,
                format!("`{name}` called with too few arguments"),
                span,
            )
        })
    };
    match name {
        // ------------------------------------------------------------ Int / Float
        "Int.pow" => {
            let base = big(arg(0)?, span)?;
            let exp = arg(1)?;
            if big(exp, span)?.is_negative() {
                return Err(rt(
                    Code::Precondition,
                    "`Int.pow` needs a non-negative exponent",
                    span,
                ));
            }
            let e = big_to_usize(exp).and_then(|e| u32::try_from(e).ok());
            let small = base.abs() <= BigInt::from(1);
            match e {
                Some(e) if small || e <= 200_000 => Ok(Value::from_big(base.pow(e))),
                _ => Err(rt(Code::Resource, "exponent too large", span)),
            }
        }
        "Int.toStr" => Ok(str_val(big(arg(0)?, span)?.to_string())),
        "Int.toFloat" => {
            let f = big(arg(0)?, span)?.to_f64().filter(|f| f.is_finite());
            f.map(Value::Float)
                .ok_or_else(|| rt(Code::Resource, "integer too large for Float", span))
        }
        "Float.sqrt" => {
            let x = float(arg(0)?, span)?;
            if x < 0.0 {
                return Err(rt(
                    Code::FloatDomain,
                    "square root of a negative number",
                    span,
                ));
            }
            Ok(Value::Float(x.sqrt()))
        }
        "Float.floor" => float_to_int(float(arg(0)?, span)?.floor(), span),
        "Float.ceil" => float_to_int(float(arg(0)?, span)?.ceil(), span),
        "Float.round" => float_to_int(float(arg(0)?, span)?.round(), span),
        "Float.toStr" => Ok(str_val(fmt_float(float(arg(0)?, span)?))),

        // ------------------------------------------------------------ Str
        "Str.len" => Ok(int_val(char_count(string(arg(0)?, span)?))),
        "Str.slice" => {
            let s = string(arg(0)?, span)?;
            let n = char_count(s);
            let from = big_to_usize(arg(1)?);
            let to = big_to_usize(arg(2)?);
            match (from, to) {
                (Some(f), Some(t)) if f <= t && t <= n => {
                    Ok(str_val(char_slice(s, f, t).to_string()))
                }
                _ => Err(rt(
                    Code::Precondition,
                    "`Str.slice` range is out of bounds",
                    span,
                )),
            }
        }
        "Str.chars" => Ok(new_list(
            string(arg(0)?, span)?
                .chars()
                .map(|c| str_val(c.to_string()))
                .collect(),
        )),
        "Str.toInt" => {
            let s = string(arg(0)?, span)?;
            let digits = s.strip_prefix('-').unwrap_or(s);
            if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
                return Ok(it.none());
            }
            Ok(s.parse::<BigInt>()
                .map_or_else(|_| it.none(), |b| it.some(Value::from_big(b))))
        }
        "Str.toFloat" => {
            let s = string(arg(0)?, span)?;
            Ok(parse_float(s).map_or_else(|| it.none(), |f| it.some(Value::Float(f))))
        }
        "Char.fromCode" => {
            let code = big_to_usize(arg(0)?)
                .and_then(|n| u32::try_from(n).ok())
                .ok_or_else(|| {
                    rt(
                        Code::Precondition,
                        "`Char.fromCode` needs a code from 0 to 1114111",
                        span,
                    )
                })?;
            // a surrogate half is not a character: it becomes U+FFFD, as in the JavaScript runtime
            Ok(str_val(
                char::from_u32(code).unwrap_or('\u{FFFD}').to_string(),
            ))
        }
        "Str.contains" => Ok(Value::Bool(
            string(arg(0)?, span)?.contains(string(arg(1)?, span)?),
        )),
        "Str.startsWith" => Ok(Value::Bool(
            string(arg(0)?, span)?.starts_with(string(arg(1)?, span)?),
        )),
        "Str.endsWith" => Ok(Value::Bool(
            string(arg(0)?, span)?.ends_with(string(arg(1)?, span)?),
        )),
        "Str.trim" => Ok(str_val(string(arg(0)?, span)?.trim())),
        "Str.upper" => Ok(str_val(string(arg(0)?, span)?.to_uppercase())),
        "Str.lower" => Ok(str_val(string(arg(0)?, span)?.to_lowercase())),
        "Str.split" => {
            let s = string(arg(0)?, span)?;
            let sep = string(arg(1)?, span)?;
            if sep.is_empty() {
                return Err(rt(
                    Code::Precondition,
                    "`Str.split` needs a non-empty separator",
                    span,
                ));
            }
            Ok(new_list(s.split(sep).map(str_val).collect()))
        }
        "Str.repeat" => {
            let s = string(arg(0)?, span)?;
            let n = big_to_usize(arg(1)?).ok_or_else(|| {
                rt(
                    Code::Precondition,
                    "`Str.repeat` needs a non-negative count",
                    span,
                )
            })?;
            if n.saturating_mul(s.len()) > MAX_LIST {
                return Err(rt(Code::Resource, "string too large", span));
            }
            Ok(str_val(s.repeat(n)))
        }

        // ------------------------------------------------------------ List
        "List.len" => Ok(int_val(list(arg(0)?, span)?.len())),
        "List.get" => {
            let l = list(arg(0)?, span)?;
            match big_to_usize(arg(1)?) {
                Some(i) if i < l.len() => Ok(l.as_slice()[i].clone()),
                _ => Err(rt(Code::IndexBounds, "index out of range", span)),
            }
        }
        "List.range" => {
            // bounds may be arbitrarily large; only the length is limited
            let lo = big(arg(0)?, span)?;
            let hi = big(arg(1)?, span)?;
            if hi <= lo {
                return Ok(new_list(Vec::new()));
            }
            let n = (&hi - &lo)
                .to_usize()
                .filter(|n| *n <= MAX_LIST)
                .ok_or_else(|| rt(Code::Resource, "range too large", span))?;
            Ok(new_list(
                (0..n)
                    .map(|i| Value::from_big(&lo + BigInt::from(i)))
                    .collect(),
            ))
        }
        "List.repeat" => {
            let n = big_to_usize(arg(1)?).ok_or_else(|| {
                rt(
                    Code::Precondition,
                    "`List.repeat` needs a non-negative count",
                    span,
                )
            })?;
            if n > MAX_LIST {
                return Err(rt(Code::Resource, "list too large", span));
            }
            Ok(new_list(vec![arg(0)?.clone(); n]))
        }
        "List.map" => {
            let l = list(arg(0)?, span)?.clone();
            let f = arg(1)?.clone();
            let mut out = Vec::with_capacity(l.len());
            for x in l.as_slice() {
                out.push(it.call_value(&f, vec![x.clone()], span)?);
            }
            Ok(new_list(out))
        }
        "List.filter" => {
            let l = list(arg(0)?, span)?.clone();
            let f = arg(1)?.clone();
            let mut out = Vec::new();
            for x in l.as_slice() {
                if truth(&it.call_value(&f, vec![x.clone()], span)?, span)? {
                    out.push(x.clone());
                }
            }
            Ok(new_list(out))
        }
        "List.fold" => {
            let l = list(arg(0)?, span)?.clone();
            let mut acc = arg(1)?.clone();
            let f = arg(2)?.clone();
            for x in l.as_slice() {
                acc = it.call_value(&f, vec![acc, x.clone()], span)?;
            }
            Ok(acc)
        }
        "List.flatMap" => {
            let l = list(arg(0)?, span)?.clone();
            let f = arg(1)?.clone();
            let mut out = Vec::new();
            for x in l.as_slice() {
                let r = it.call_value(&f, vec![x.clone()], span)?;
                out.extend_from_slice(list(&r, span)?.as_slice());
            }
            Ok(new_list(out))
        }
        "List.append" => {
            let l = list(arg(0)?, span)?;
            let mut v = l.as_slice().to_vec();
            v.push(arg(1)?.clone());
            Ok(new_list(v))
        }
        "List.concat" => {
            let mut v = list(arg(0)?, span)?.as_slice().to_vec();
            v.extend_from_slice(list(arg(1)?, span)?.as_slice());
            Ok(new_list(v))
        }
        "List.reverse" => {
            let mut v = list(arg(0)?, span)?.as_slice().to_vec();
            v.reverse();
            Ok(new_list(v))
        }
        "List.take" => {
            let l = list(arg(0)?, span)?;
            let n = big(arg(1)?, span)?;
            let k = if n.is_negative() {
                0
            } else {
                n.to_usize().unwrap_or(usize::MAX).min(l.len())
            };
            Ok(Value::List(l.slice(0, k)))
        }
        "List.drop" => {
            let l = list(arg(0)?, span)?;
            let n = big(arg(1)?, span)?;
            let k = if n.is_negative() {
                0
            } else {
                n.to_usize().unwrap_or(usize::MAX).min(l.len())
            };
            Ok(Value::List(l.slice(k, l.len())))
        }
        "List.any" | "List.all" | "List.count" => {
            let l = list(arg(0)?, span)?.clone();
            let f = arg(1)?.clone();
            let mut count = 0usize;
            for x in l.as_slice() {
                let t = truth(&it.call_value(&f, vec![x.clone()], span)?, span)?;
                if name == "List.any" && t {
                    return Ok(Value::Bool(true));
                }
                if name == "List.all" && !t {
                    return Ok(Value::Bool(false));
                }
                count += usize::from(t);
            }
            Ok(match name {
                "List.any" => Value::Bool(false),
                "List.all" => Value::Bool(true),
                _ => int_val(count),
            })
        }
        "List.contains" => {
            let x = arg(1)?;
            Ok(Value::Bool(
                list(arg(0)?, span)?
                    .as_slice()
                    .iter()
                    .any(|y| values_eq(x, y)),
            ))
        }
        "List.indexOf" => {
            let x = arg(1)?;
            Ok(list(arg(0)?, span)?
                .as_slice()
                .iter()
                .position(|y| values_eq(x, y))
                .map_or_else(|| it.none(), |i| it.some(int_val(i))))
        }
        "List.first" => Ok(list(arg(0)?, span)?
            .as_slice()
            .first()
            .map_or_else(|| it.none(), |v| it.some(v.clone()))),
        "List.last" => Ok(list(arg(0)?, span)?
            .as_slice()
            .last()
            .map_or_else(|| it.none(), |v| it.some(v.clone()))),
        "List.zip" => {
            let (x, y) = (list(arg(0)?, span)?, list(arg(1)?, span)?);
            Ok(new_list(
                x.as_slice()
                    .iter()
                    .zip(y.as_slice())
                    .map(|(p, q)| Value::Tuple(Rc::new(vec![p.clone(), q.clone()])))
                    .collect(),
            ))
        }
        "List.enumerate" => Ok(new_list(
            list(arg(0)?, span)?
                .as_slice()
                .iter()
                .enumerate()
                .map(|(i, v)| Value::Tuple(Rc::new(vec![int_val(i), v.clone()])))
                .collect(),
        )),
        "List.sortBy" => {
            let l = list(arg(0)?, span)?.clone();
            let f = arg(1)?.clone();
            let mut keyed = Vec::with_capacity(l.len());
            for x in l.as_slice() {
                keyed.push((it.call_value(&f, vec![x.clone()], span)?, x.clone()));
            }
            keyed.sort_by(|p, q| values_cmp(&p.0, &q.0).unwrap_or(std::cmp::Ordering::Equal));
            Ok(new_list(keyed.into_iter().map(|(_, v)| v).collect()))
        }
        "List.sort" => {
            let mut v = list(arg(0)?, span)?.as_slice().to_vec();
            v.sort_by(|p, q| values_cmp(p, q).unwrap_or(std::cmp::Ordering::Equal));
            Ok(new_list(v))
        }
        "List.sum" => {
            let mut total = BigInt::zero();
            for x in list(arg(0)?, span)?.as_slice() {
                total += big(x, span)?;
            }
            Ok(Value::from_big(total))
        }
        "List.min" | "List.max" => {
            let l = list(arg(0)?, span)?;
            let mut best: Option<&Value<'p>> = None;
            for x in l.as_slice() {
                best = match best {
                    None => Some(x),
                    Some(b) => {
                        let ord = values_cmp(x, b).unwrap_or(std::cmp::Ordering::Equal);
                        if (name == "List.min" && ord.is_lt())
                            || (name == "List.max" && ord.is_gt())
                        {
                            Some(x)
                        } else {
                            Some(b)
                        }
                    }
                };
            }
            Ok(best.map_or_else(|| it.none(), |b| it.some(b.clone())))
        }
        "List.join" => {
            let sep = string(arg(1)?, span)?.to_string();
            let mut parts = Vec::new();
            for x in list(arg(0)?, span)?.as_slice() {
                parts.push(string(x, span)?.to_string());
            }
            Ok(str_val(parts.join(&sep)))
        }
        _ => Err(rt(
            Code::Internal,
            format!("native function `{name}` is not implemented"),
            span,
        )),
    }
}

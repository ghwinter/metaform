//! `metaform propose`: validate predicates proposed by an external model (protocol `metaform.propose/1`).
//!
//! The boundary is deliberately read-only. The engine reads a request file and a response file, checks
//! the response in a fixed order, and prints one line of JSON. It never writes a file and never touches
//! `.metaform/state.json`, so a proposal can be judged before anything is adopted.
//!
//! Validation order (the first failing rule wins):
//!   1. response `protocol` must be `metaform.propose/1` → otherwise the whole response is `wrong_protocol`
//!   2. response `request_id` must equal the request's → otherwise `wrong_request_id`
//!   3. a non-null `refused` string declines the task: nothing is accepted, the string is echoed
//!   4. per candidate: `missing_name`, `duplicate_name`; a candidate carrying both `ids` and `define`,
//!      or a malformed `define`, is `wrong_protocol`
//!   5. an `ids` candidate then fails `empty_extension`, `constant_extension`, `out_of_range`, `duplicate_ids`
//!   6. a `define` candidate then fails `empty_definition`, `duplicate_operand`, `unknown_predicate`,
//!      `empty_extension`, `constant_extension`
//!   7. the held-out prediction: `missing_prediction` when the request has an unmeasured object (or
//!      the candidate names a `rival`) and the candidate carries no `predict`; otherwise a malformed
//!      `predict` is `wrong_protocol`, and its id/label are checked as `prediction_out_of_range`,
//!      `predicted_measured_object`, `prediction_bad_label`
//!   8. a `rival` is parsed with the candidate's own rules; when both are present the prediction must
//!      land on an object where the two extensions disagree, or `predict_not_discriminating`
//!   9. `limit_exceeded` for both forms
//!  10. accepted candidates report `splits`: how many evidence-colliding pairs they separate
//!
//! A candidate is extensional (`ids`) or defined (`define: {"all_of": [...]}` / `{"any_of": [...]}`).
//! A definition may only name predicates that already exist in the request vocabulary; the operands are
//! combined with bitwise AND (`all_of`) or OR (`any_of`) and intersected with the object mask. The
//! resulting extension then follows the same acceptance path as an `ids` candidate.
//!
//! L11 -- invention is admitted with a held-out prediction. On a request that carries `object_ids`, the
//! objects in that list and not in `evidence` are *unmeasured*: their labels are the held-out test a
//! candidate has to name before adoption. A candidate may carry
//! `"predict": {"id": <object id>, "label": "pos" | "neg"}`; on a request with at least one unmeasured
//! object a candidate without it is rejected (`missing_prediction`), and a prediction is always checked
//! when present. The prediction is echoed unchanged on acceptance, and `apply` records it. A request
//! without `object_ids` declares no unmeasured objects, which keeps every pre-L11 request valid.
//!
//! A candidate may also name the rule it is competing with: `"rival": {"ids": [...]}` or
//! `"rival": {"define": {...}}`, parsed with the candidate's own shape, range and vocabulary rules.
//! The rival's extension is recomputed from the request vocabulary; it is never taken from the
//! response. A prediction is a bet only if the two rules could have made different bets there, so a
//! rivaled candidate must predict an object on which the candidate's extension and the rival's
//! extension disagree -- otherwise the answer is `predict_not_discriminating`. This is what stops a
//! coherent but false rule from passing on a prediction that every rule in its neighbourhood makes.
//! A rival without a prediction is `missing_prediction`, because there is nothing left to compare.
//! A candidate without `rival` is exactly the pre-rival candidate, and `apply` records its rival
//! column empty.
//!
//! Pair logic follows `score` in `main.rs`: positives and negatives that share an equivalence class under
//! the current vocabulary form a colliding pair, and a candidate splits the pair when its membership
//! differs on the two objects. The vocabulary is what makes a pair "colliding"; the evidence only says
//! which objects are positive and which are negative.
//!
//! JSON is parsed and written by hand so this boundary stays dependency-free and byte-for-byte
//! deterministic: no map iteration order and no serializer settings leak into the output.

use std::collections::HashSet;

use crate::{bit, DefOp, Definition, Explorer, Mask, Predicate, Vocabulary};

/// The protocol string that identifies this boundary.
const PROTOCOL: &str = "metaform.propose/1";

/// A proposal as a component supplies it: the ids it covers, with optional provenance.
pub struct Proposal<'a> {
    pub name: &'a str,
    pub ids: &'a [usize],
    pub warrant: Option<&'a str>,
    pub falsifier: Option<&'a str>,
}

/// The label a candidate commits to for an object it was not built from (L11).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PredictedLabel {
    Pos,
    Neg,
}

impl PredictedLabel {
    /// Parse the protocol spelling. Anything else is `prediction_bad_label`.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "pos" => Some(PredictedLabel::Pos),
            "neg" => Some(PredictedLabel::Neg),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            PredictedLabel::Pos => "pos",
            PredictedLabel::Neg => "neg",
        }
    }
}

/// A held-out commitment: the object the candidate names and the label it predicts for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prediction {
    pub id: usize,
    pub label: PredictedLabel,
}

/// One candidate the validator accepted, with everything `apply` needs to adopt it.
#[derive(Clone, Debug, PartialEq)]
pub struct Accepted {
    pub name: String,
    /// The extension the candidate covers. For a defined candidate this is the computed extension,
    /// not a field the response supplied.
    pub ids: Vec<usize>,
    /// How many evidence-colliding pairs the extension separates.
    pub splits: usize,
    pub warrant: Option<String>,
    pub falsifier: Option<String>,
    /// `Some` when the candidate was defined (`all_of` / `any_of`); [`Accepted::ids`] is then its
    /// extension. `None` means the candidate supplied its ids directly.
    pub definition: Option<Definition>,
    /// The held-out prediction the candidate committed to, if it carried one. Required on a request
    /// with an unmeasured object; optional (but validated) otherwise.
    pub prediction: Option<Prediction>,
    /// The extension of the `rival` the candidate named, if any. The prediction was accepted only
    /// because the candidate and this rival disagree on the predicted object; `apply` records the
    /// extension so `check-predictions` can count the discriminating bets.
    pub rival: Option<Vec<usize>>,
    /// Optional confidence in this candidate, in [0, 1]. It orders abstention in the metrics and is
    /// never used to validate the judgement itself.
    pub score: Option<f64>,
    /// Objects the candidate declines to classify. They must be unmeasured and outside the claimed
    /// extension; a declined object is the abstention the risk-coverage curve needs.
    pub abstain: Vec<usize>,
}

/// One candidate the validator rejected, with the ids it supplied (empty for a defined candidate).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rejected {
    pub name: String,
    pub ids: Vec<i64>,
    pub reason: String,
}

/// The complete result of validating one response against one request.
///
/// `accepted` / `rejected` are in response order. A whole-response rejection sets `whole_reject`
/// and leaves the candidate lists empty; a `refused` answer sets `refused` and also has no
/// candidates. Both are valid `Verdict`s: only the request itself can make validation fail.
#[derive(Clone, Debug, PartialEq)]
pub struct Verdict {
    pub request_id: String,
    pub accepted: Vec<Accepted>,
    pub rejected: Vec<Rejected>,
    pub refused: Option<String>,
    /// `Some(reason)` when the response was rejected as a whole (`wrong_protocol`,
    /// `wrong_request_id`, `malformed_response`).
    pub whole_reject: Option<String>,
}

/// The ids a mask covers, for a state with `objects` objects.
pub fn ids_of(mask: Mask, objects: usize) -> Vec<usize> {
    (0..objects).filter(|o| (mask >> o) & 1 == 1).collect()
}

/// Writes the response file a component hands to `metaform propose`. One writer for the format,
/// so a component cannot invent a slightly different shape.
pub fn response_file(request_id: &str, proposals: &[Proposal<'_>]) -> String {
    let entries: Vec<String> = proposals
        .iter()
        .map(|p| {
            let ids: Vec<String> = p.ids.iter().map(|i| i.to_string()).collect();
            let mut entry = format!("{{\"name\":{},\"ids\":[{}]", json_string(p.name), ids.join(","));
            if let Some(w) = p.warrant {
                entry.push_str(&format!(",\"warrant\":{}", json_string(w)));
            }
            if let Some(f) = p.falsifier {
                entry.push_str(&format!(",\"falsifier\":{}", json_string(f)));
            }
            entry.push('}');
            entry
        })
        .collect();
    format!(
        "{{\"protocol\":\"{PROTOCOL}\",\"request_id\":{},\"candidates\":[{}],\"refused\":null}}",
        json_string(request_id),
        entries.join(",")
    )
}

/// Default cap on accepted candidates when the request has no `limit`.
const DEFAULT_LIMIT: usize = 8;

// ---------------------------------------------------------------------------
// Minimal JSON value + parser
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    /// Object lookup. A repeated key resolves to the last occurrence, as in most JSON readers.
    fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(fields) => fields
                .iter()
                .rev()
                .find(|entry| entry.0.as_str() == key)
                .map(|entry| &entry.1),
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    fn as_arr(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(items) => Some(items),
            _ => None,
        }
    }

    /// Integral numbers only: ids and counts are integers in this protocol.
    fn as_i64(&self) -> Option<i64> {
        match self {
            Json::Num(x) if x.is_finite() && x.fract() == 0.0 => Some(*x as i64),
            _ => None,
        }
    }

    /// Any finite number, used by the optional confidence score.
    fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Num(x) if x.is_finite() => Some(*x),
            _ => None,
        }
    }
}

fn parse_json(text: &str) -> Result<Json, String> {
    let mut p = Parser { bytes: text.as_bytes(), at: 0 };
    let value = p.value()?;
    p.skip_ws();
    if p.at != p.bytes.len() {
        return Err(p.err("trailing data"));
    }
    Ok(value)
}

/// Recursive-descent JSON parser (objects, arrays, strings with escapes, numbers, literals).
struct Parser<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Parser<'a> {
    fn err(&self, msg: &str) -> String {
        format!("{msg} at byte {}", self.at)
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let b = self.peek();
        if b.is_some() {
            self.at += 1;
        }
        b
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            self.at += 1;
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.skip_ws();
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b't') => {
                self.literal("true")?;
                Ok(Json::Bool(true))
            }
            Some(b'f') => {
                self.literal("false")?;
                Ok(Json::Bool(false))
            }
            Some(b'n') => {
                self.literal("null")?;
                Ok(Json::Null)
            }
            Some(b'-') | Some(b'0'..=b'9') => self.number(),
            Some(_) => Err(self.err("unexpected byte")),
            None => Err(self.err("unexpected end of input")),
        }
    }

    fn object(&mut self) -> Result<Json, String> {
        self.at += 1; // consume `{`
        let mut fields = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.at += 1;
            return Ok(Json::Obj(fields));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some(b'"') {
                return Err(self.err("expected a string key"));
            }
            let key = self.string()?;
            self.skip_ws();
            if self.peek() != Some(b':') {
                return Err(self.err("expected `:`"));
            }
            self.at += 1;
            let value = self.value()?;
            fields.push((key, value));
            self.skip_ws();
            match self.bump() {
                Some(b',') => continue,
                Some(b'}') => return Ok(Json::Obj(fields)),
                _ => return Err(self.err("expected `,` or `}`")),
            }
        }
    }

    fn array(&mut self) -> Result<Json, String> {
        self.at += 1; // consume `[`
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.at += 1;
            return Ok(Json::Arr(items));
        }
        loop {
            items.push(self.value()?);
            self.skip_ws();
            match self.bump() {
                Some(b',') => continue,
                Some(b']') => return Ok(Json::Arr(items)),
                _ => return Err(self.err("expected `,` or `]`")),
            }
        }
    }

    fn string(&mut self) -> Result<String, String> {
        if self.bump() != Some(b'"') {
            return Err(self.err("expected a string"));
        }
        let mut out = String::new();
        loop {
            let b = match self.bump() {
                Some(b) => b,
                None => return Err(self.err("unterminated string")),
            };
            match b {
                b'"' => return Ok(out),
                b'\\' => {
                    let esc = match self.bump() {
                        Some(b) => b,
                        None => return Err(self.err("unterminated escape")),
                    };
                    match esc {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{0008}'),
                        b'f' => out.push('\u{000C}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hi = self.hex4()?;
                            let ch = if (0xD800..0xDC00).contains(&hi) {
                                // A surrogate pair only makes sense as a pair; a lone surrogate becomes U+FFFD.
                                if self.peek() == Some(b'\\') && self.bytes.get(self.at + 1) == Some(&b'u') {
                                    self.at += 2;
                                    let lo = self.hex4()?;
                                    if (0xDC00..0xE000).contains(&lo) {
                                        char::from_u32(0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00))
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            } else {
                                char::from_u32(hi)
                            };
                            out.push(ch.unwrap_or('\u{FFFD}'));
                        }
                        _ => return Err(self.err("unknown escape")),
                    }
                }
                0x00..=0x1F => return Err(self.err("control character in string")),
                0x20..=0x7F => out.push(b as char),
                _ => {
                    // Copy one complete UTF-8 sequence. The leading byte was already consumed.
                    let start = self.at - 1;
                    let len = if b & 0xE0 == 0xC0 {
                        2
                    } else if b & 0xF0 == 0xE0 {
                        3
                    } else if b & 0xF8 == 0xF0 {
                        4
                    } else {
                        return Err(self.err("invalid UTF-8"));
                    };
                    if start + len > self.bytes.len() {
                        return Err(self.err("truncated UTF-8"));
                    }
                    let part = match std::str::from_utf8(&self.bytes[start..start + len]) {
                        Ok(s) => s,
                        Err(_) => return Err(self.err("invalid UTF-8")),
                    };
                    out.push_str(part);
                    self.at = start + len;
                }
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let mut v = 0u32;
        for _ in 0..4 {
            let b = match self.bump() {
                Some(b) => b,
                None => return Err(self.err("truncated \\u escape")),
            };
            let d = match b {
                b'0'..=b'9' => (b - b'0') as u32,
                b'a'..=b'f' => (b - b'a' + 10) as u32,
                b'A'..=b'F' => (b - b'A' + 10) as u32,
                _ => return Err(self.err("invalid hex digit in \\u escape")),
            };
            v = v * 16 + d;
        }
        Ok(v)
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.at;
        if self.peek() == Some(b'-') {
            self.at += 1;
        }
        if !matches!(self.peek(), Some(b'0'..=b'9')) {
            return Err(self.err("expected a digit"));
        }
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.at += 1;
        }
        if self.peek() == Some(b'.') {
            self.at += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.err("expected a digit after `.`"));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.at += 1;
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.at += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.at += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.err("expected a digit in the exponent"));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.at += 1;
            }
        }
        let text = match std::str::from_utf8(&self.bytes[start..self.at]) {
            Ok(s) => s,
            Err(_) => return Err(self.err("invalid number")),
        };
        match text.parse::<f64>() {
            Ok(v) => Ok(Json::Num(v)),
            Err(_) => Err(self.err("invalid number")),
        }
    }

    fn literal(&mut self, word: &str) -> Result<(), String> {
        let bytes = word.as_bytes();
        if self.bytes.get(self.at..self.at + bytes.len()) == Some(bytes) {
            self.at += bytes.len();
            Ok(())
        } else {
            Err(self.err("invalid literal"))
        }
    }
}

// ---------------------------------------------------------------------------
// Request
// ---------------------------------------------------------------------------

/// A parsed `metaform.propose/1` request. Everything the validator needs is already masked to `n` objects.
struct Request {
    request_id: String,
    n: usize,
    /// The objects the request declares; `suggest --emit-request` lists `0..n`. An absent field is
    /// the pre-L11 shape and declares no unmeasured object, so old requests keep their verdicts.
    object_ids: Vec<usize>,
    /// Current vocabulary, in request order. Only names and extensions matter here.
    vocab: Vec<(String, Mask)>,
    pos: Mask,
    neg: Mask,
    limit: usize,
}

impl Request {
    /// Objects named by the request whose label is not in `evidence`. Their labels are held out, so
    /// L11 requires a candidate to name one of them and predict it.
    fn has_unmeasured(&self) -> bool {
        let measured = self.pos | self.neg;
        self.object_ids.iter().any(|&o| measured & bit(o) == 0)
    }
}

fn parse_request(text: &str) -> Result<Request, String> {
    let root = parse_json(text).map_err(|e| format!("not valid JSON: {e}"))?;
    if !matches!(&root, Json::Obj(_)) {
        return Err("top-level value must be an object".into());
    }

    let request_id = match root.get("request_id").and_then(Json::as_str) {
        Some(s) => s.to_string(),
        None => return Err("missing string field `request_id`".into()),
    };

    let n = match root.get("objects").and_then(Json::as_i64) {
        Some(n) if (0..=64).contains(&n) => n as usize,
        Some(n) => return Err(format!("`objects` must be between 0 and 64, got {n}")),
        None => return Err("missing integer field `objects`".into()),
    };

    let limit = match root.get("limit") {
        None | Some(Json::Null) => DEFAULT_LIMIT,
        Some(v) => match v.as_i64() {
            Some(l) if l >= 0 => l as usize,
            Some(l) => return Err(format!("`limit` must not be negative, got {l}")),
            None => return Err("`limit` must be an integer".into()),
        },
    };

    let mut object_ids = Vec::new();
    match root.get("object_ids") {
        None | Some(Json::Null) => {}
        Some(Json::Arr(items)) => {
            for id in items {
                if let Some(x) = id.as_i64() {
                    if x >= 0 && (x as usize) < n {
                        object_ids.push(x as usize);
                    }
                }
            }
        }
        Some(_) => return Err("`object_ids` must be an array".into()),
    }

    let mut vocab = Vec::new();
    match root.get("vocabulary") {
        None | Some(Json::Null) => {}
        Some(Json::Arr(items)) => {
            for (i, item) in items.iter().enumerate() {
                let name = match item.get("name").and_then(Json::as_str) {
                    Some(s) => s.to_string(),
                    None => return Err(format!("vocabulary[{i}].name must be a string")),
                };
                let mut mask = 0u64;
                if let Some(ids) = item.get("ids").and_then(Json::as_arr) {
                    for id in ids {
                        if let Some(x) = id.as_i64() {
                            if x >= 0 && (x as usize) < n {
                                mask |= bit(x as usize);
                            }
                        }
                    }
                }
                vocab.push((name, mask));
            }
        }
        Some(_) => return Err("`vocabulary` must be an array".into()),
    }

    let mut pos = 0u64;
    let mut neg = 0u64;
    match root.get("evidence") {
        None | Some(Json::Null) => {}
        Some(ev) if matches!(ev, Json::Obj(_)) => {
            for (key, target) in [("pos", &mut pos), ("neg", &mut neg)] {
                match ev.get(key) {
                    None | Some(Json::Null) => {}
                    Some(Json::Arr(ids)) => {
                        for id in ids {
                            if let Some(x) = id.as_i64() {
                                if x >= 0 && (x as usize) < n {
                                    *target |= bit(x as usize);
                                }
                            }
                        }
                    }
                    Some(_) => return Err(format!("`evidence.{key}` must be an array")),
                }
            }
        }
        Some(_) => return Err("`evidence` must be an object".into()),
    }

    Ok(Request {
        request_id,
        n,
        object_ids,
        vocab,
        pos,
        neg,
        limit,
    })
}

// ---------------------------------------------------------------------------
// Response validation
// ---------------------------------------------------------------------------

/// Candidate ids as they appeared: integral values are echoed back; `count` distinguishes
/// "no ids" from "ids that all failed to parse".
struct Ids {
    values: Vec<i64>,
    count: usize,
    is_array: bool,
    bad: bool,
}

fn parse_ids(value: Option<&Json>) -> Ids {
    let mut out = Ids { values: Vec::new(), count: 0, is_array: false, bad: false };
    if let Some(Json::Arr(items)) = value {
        out.is_array = true;
        out.count = items.len();
        for item in items {
            match item.as_i64() {
                Some(x) => out.values.push(x),
                None => out.bad = true,
            }
        }
    }
    out
}

fn has_duplicates(ids: &[i64]) -> bool {
    let mut seen = HashSet::new();
    ids.iter().any(|&x| !seen.insert(x))
}

/// `true` when the extension contains every object (a constant predicate: it separates nothing).
fn covers_all(n: usize, ids: &[i64]) -> bool {
    (0..n).all(|x| ids.contains(&(x as i64)))
}

/// Mask of the request's `n` objects (`n <= 64`).
fn object_mask(n: usize) -> Mask {
    if n == 0 {
        0
    } else {
        Mask::MAX >> (64 - n)
    }
}

/// Parse a `define` value. A malformed clause is a candidate-level `wrong_protocol` rejection.
///
/// The operands are returned in the spelling the candidate sent: `definition_extension` must still
/// see a repeated operand to reject it as `duplicate_operand` instead of silently merging it. The
/// accepted path canonicalises the definition after that validation.
fn parse_definition(value: &Json) -> Result<Definition, &'static str> {
    let Json::Obj(fields) = value else {
        return Err("wrong_protocol");
    };
    // Exactly one operator. Two operators (or an unknown key) would make the extension ambiguous.
    let [(key, operands)] = fields.as_slice() else {
        return Err("wrong_protocol");
    };
    let op = match key.as_str() {
        "all_of" => DefOp::AllOf,
        "any_of" => DefOp::AnyOf,
        _ => return Err("wrong_protocol"),
    };
    let Json::Arr(items) = operands else {
        return Err("wrong_protocol");
    };
    let mut names = Vec::with_capacity(items.len());
    for item in items {
        match item.as_str() {
            Some(name) => names.push(name.to_string()),
            None => return Err("wrong_protocol"),
        }
    }
    Ok(Definition { op, operands: names })
}

/// Evaluate a definition against the request vocabulary. The caller intersects with the object mask.
fn definition_extension(
    def: &Definition,
    vocab: &[(String, Mask)],
) -> Result<Mask, &'static str> {
    if def.operands.is_empty() {
        return Err("empty_definition");
    }
    let mut seen = HashSet::new();
    for operand in &def.operands {
        if !seen.insert(operand.as_str()) {
            return Err("duplicate_operand");
        }
    }
    let mut ext = match def.op {
        DefOp::AllOf => Mask::MAX,
        DefOp::AnyOf => 0,
    };
    for operand in &def.operands {
        let Some((_, mask)) = vocab.iter().find(|(name, _)| name == operand) else {
            return Err("unknown_predicate");
        };
        ext = match def.op {
            DefOp::AllOf => ext & mask,
            DefOp::AnyOf => ext | mask,
        };
    }
    Ok(ext)
}

/// Validate the optional `predict` field of one candidate.
///
/// A malformed shape is a candidate-level `wrong_protocol`; an id that does not name an object is
/// `prediction_out_of_range`; an id whose label is already in the evidence is
/// `predicted_measured_object` (it is not a held-out test); a label other than `pos` / `neg` is
/// `prediction_bad_label`. `None` means the candidate carried no prediction.
fn parse_prediction(obj: &Json, req: &Request) -> Result<Option<Prediction>, &'static str> {
    let Some(value) = obj.get("predict") else {
        return Ok(None);
    };
    // `null` is the absent spelling, matching `limit` and `refused`.
    if matches!(value, Json::Null) {
        return Ok(None);
    }
    if !matches!(value, Json::Obj(_)) {
        return Err("wrong_protocol");
    }
    let Some(raw_id) = value.get("id").and_then(Json::as_i64) else {
        return Err("prediction_out_of_range");
    };
    if raw_id < 0 || raw_id as usize >= req.n {
        return Err("prediction_out_of_range");
    }
    let id = raw_id as usize;
    if (req.pos | req.neg) & bit(id) != 0 {
        return Err("predicted_measured_object");
    }
    let Some(label) = value.get("label").and_then(Json::as_str).and_then(PredictedLabel::parse)
    else {
        return Err("prediction_bad_label");
    };
    Ok(Some(Prediction { id, label }))
}

/// The extension of one alternative: a candidate, or the `rival` a candidate names. Both forms are
/// parsed and checked by the same rules, so a rival cannot be held to a weaker standard than the
/// candidate it competes with.
struct Alternative {
    mask: Mask,
    definition: Option<Definition>,
}

/// Parse one `ids`-or-`define` alternative into its extension over the request's objects.
///
/// The rules are the candidate's own: carrying both `ids` and `define` is `wrong_protocol`; a
/// defined alternative may only name request vocabulary and fails `empty_definition`,
/// `duplicate_operand` or `unknown_predicate`; an extensional one fails `empty_extension`,
/// `constant_extension`, `out_of_range` or `duplicate_ids`; a defined one also fails
/// `empty_extension` / `constant_extension` once its operands resolve. A missing `ids` is the empty
/// extension, exactly as for a candidate.
fn alternative(value: &Json, req: &Request) -> Result<Alternative, &'static str> {
    let has_ids = value.get("ids").is_some();
    let has_define = value.get("define").is_some();
    // An alternative is either extensional or defined, never both.
    if has_ids && has_define {
        return Err("wrong_protocol");
    }
    if has_define {
        let mut definition = parse_definition(value.get("define").expect("checked present"))?;
        let ext = definition_extension(&definition, &req.vocab)? & object_mask(req.n);
        if ext == 0 {
            return Err("empty_extension");
        }
        if ext == object_mask(req.n) {
            return Err("constant_extension");
        }
        // Validation is done: `duplicate_operand` has rejected a raw repeated operand above, so
        // canonicalising now only fixes the order. The adopted definition is stored sorted and
        // unique, making all_of:[b,a] and all_of:[a,b] the same definition.
        definition.canonicalize();
        return Ok(Alternative { mask: ext, definition: Some(definition) });
    }
    let ids = parse_ids(value.get("ids"));
    if !ids.is_array || ids.count == 0 {
        return Err("empty_extension");
    }
    if covers_all(req.n, &ids.values) {
        return Err("constant_extension");
    }
    if ids.bad || ids.values.iter().any(|&x| x < 0 || x as usize >= req.n) {
        return Err("out_of_range");
    }
    if has_duplicates(&ids.values) {
        return Err("duplicate_ids");
    }
    let mut mask = 0u64;
    for &x in &ids.values {
        mask |= bit(x as usize);
    }
    Ok(Alternative { mask, definition: None })
}

/// Validate the optional `rival` field: the competing rule the candidate has to disagree with.
///
/// `null` is the absent spelling, matching `predict`, `limit` and `refused`. A present rival must be
/// an object and is parsed by [`alternative`], so it answers to the candidate's own rules; its
/// extension is `Some(mask)`, computed from the request vocabulary and the request's objects.
fn parse_rival(obj: &Json, req: &Request) -> Result<Option<Mask>, &'static str> {
    match obj.get("rival") {
        None | Some(Json::Null) => Ok(None),
        Some(value) if matches!(value, Json::Obj(_)) => Ok(Some(alternative(value, req)?.mask)),
        Some(_) => Err("wrong_protocol"),
    }
}

/// Parse the prediction and the rival, and apply the L11 admission rules.
///
/// The prediction's own rules are unchanged. A candidate that names a rival must also place a bet:
/// the rival exists to make the bet discriminating, so a rival without `predict` is
/// `missing_prediction` even on a request whose objects are all measured. When both are present,
/// the predicted object must be one where the candidate's extension and the rival's extension
/// disagree; if they agree there the bet says nothing about the difference between the two rules,
/// and the candidate is `predict_not_discriminating`.
fn checked_commitment(
    obj: &Json,
    req: &Request,
    ext: Mask,
) -> Result<(Option<Prediction>, Option<Mask>), &'static str> {
    let prediction = parse_prediction(obj, req)?;
    let rival = parse_rival(obj, req)?;
    if prediction.is_none() && (req.has_unmeasured() || rival.is_some()) {
        return Err("missing_prediction");
    }
    if let (Some(prediction), Some(rival)) = (prediction, rival) {
        if ((ext >> prediction.id) & 1) == ((rival >> prediction.id) & 1) {
            return Err("predict_not_discriminating");
        }
    }
    Ok((prediction, rival))
}

fn count_splits(n: usize, pos: Mask, neg: Mask, classes: &[Mask], ext: Mask) -> usize {
    let mut total = 0;
    for class in classes {
        for a in 0..n {
            if class & bit(a) == 0 || pos & bit(a) == 0 {
                continue;
            }
            for b in 0..n {
                if class & bit(b) == 0 || neg & bit(b) == 0 {
                    continue;
                }
                if ((ext >> a) & 1) != ((ext >> b) & 1) {
                    total += 1;
                }
            }
        }
    }
    total
}

/// Equivalence classes of the request's vocabulary over its objects.
fn evidence_classes(req: &Request) -> Vec<Mask> {
    let preds = req
        .vocab
        .iter()
        .map(|(name, ext)| Predicate::new(name.clone(), *ext))
        .collect();
    let mut e = Explorer::new(req.n, Vocabulary::with(preds));
    e.pos = req.pos;
    e.neg = req.neg;
    e.classes()
}

/// Escape one JSON string. Shared with `metaform apply`, so both commands spell a name the same way.
pub fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000C}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn ids_json(ids: &[i64]) -> String {
    let items: Vec<String> = ids.iter().map(|x| x.to_string()).collect();
    format!("[{}]", items.join(","))
}

fn prediction_json(prediction: &Prediction) -> String {
    format!(
        "{{\"id\":{},\"label\":\"{}\"}}",
        prediction.id,
        prediction.label.as_str()
    )
}

fn accepted_json(candidate: &Accepted) -> String {
    let ids: Vec<String> = candidate.ids.iter().map(|x| x.to_string()).collect();
    let mut out = format!(
        "{{\"name\":{},\"ids\":[{}],\"splits\":{}",
        json_string(&candidate.name),
        ids.join(","),
        candidate.splits
    );
    if let Some(warrant) = &candidate.warrant {
        out.push_str(",\"warrant\":");
        out.push_str(&json_string(warrant));
    }
    if let Some(falsifier) = &candidate.falsifier {
        out.push_str(",\"falsifier\":");
        out.push_str(&json_string(falsifier));
    }
    if let Some(prediction) = &candidate.prediction {
        out.push_str(",\"predict\":");
        out.push_str(&prediction_json(prediction));
    }
    if let Some(score) = candidate.score {
        out.push_str(&format!(",\"score\":{score}"));
    }
    if !candidate.abstain.is_empty() {
        let ids: Vec<String> = candidate.abstain.iter().map(|x| x.to_string()).collect();
        out.push_str(&format!(",\"abstain\":[{}]", ids.join(",")));
    }
    out.push('}');
    out
}

fn rejected_json(candidate: &Rejected) -> String {
    format!(
        "{{\"name\":{},\"ids\":{},\"reason\":{}}}",
        json_string(&candidate.name),
        ids_json(&candidate.ids),
        json_string(&candidate.reason)
    )
}

fn response_json(request_id: &str, accepted: &str, rejected: &str, refused: &str) -> String {
    format!(
        "{{\"protocol\":\"{PROTOCOL}\",\"request_id\":{},\"accepted\":{accepted},\"rejected\":{rejected},\"refused\":{refused}}}",
        json_string(request_id)
    )
}

/// A whole-response rejection (protocol, request id, or unreadable JSON) carries no candidate fields.
fn whole_reject(req: &Request, reason: &str) -> Verdict {
    Verdict {
        request_id: req.request_id.clone(),
        accepted: Vec::new(),
        rejected: Vec::new(),
        refused: None,
        whole_reject: Some(reason.to_string()),
    }
}

fn refused_response(req: &Request, refused: &str) -> Verdict {
    Verdict {
        request_id: req.request_id.clone(),
        accepted: Vec::new(),
        rejected: Vec::new(),
        refused: Some(refused.to_string()),
        whole_reject: None,
    }
}

/// Render a verdict as the one line of JSON `metaform propose` prints. Field order is fixed and
/// independent of any map iteration, so identical inputs produce identical bytes.
pub fn verdict_json(verdict: &Verdict) -> String {
    if let Some(reason) = &verdict.whole_reject {
        let rejected = format!("[{{\"reason\":{}}}]", json_string(reason));
        return response_json(&verdict.request_id, "[]", &rejected, "null");
    }
    if let Some(refused) = &verdict.refused {
        return response_json(&verdict.request_id, "[]", "[]", &json_string(refused));
    }
    let accepted: Vec<String> = verdict.accepted.iter().map(accepted_json).collect();
    let rejected: Vec<String> = verdict.rejected.iter().map(rejected_json).collect();
    response_json(
        &verdict.request_id,
        &format!("[{}]", accepted.join(",")),
        &format!("[{}]", rejected.join(",")),
        "null",
    )
}

/// Validate one response text against a parsed request. Pure: no file access, no state.
fn evaluate(req: &Request, response_text: &str) -> Verdict {
    let root = match parse_json(response_text) {
        Ok(value @ Json::Obj(_)) => value,
        _ => return whole_reject(req, "malformed_response"),
    };

    // 1. Protocol first: nothing else is looked at when the model answered another protocol.
    if root.get("protocol").and_then(Json::as_str) != Some(PROTOCOL) {
        return whole_reject(req, "wrong_protocol");
    }

    // 2. The answer must belong to this request.
    if root.get("request_id").and_then(Json::as_str) != Some(req.request_id.as_str()) {
        return whole_reject(req, "wrong_request_id");
    }

    // 3. A model is allowed to decline; that is not a validation failure.
    match root.get("refused") {
        None | Some(Json::Null) => {}
        Some(Json::Str(refused)) => return refused_response(req, refused),
        Some(_) => return whole_reject(req, "malformed_response"),
    }

    let candidates: &[Json] = match root.get("candidates") {
        None | Some(Json::Null) => &[],
        Some(Json::Arr(items)) => items,
        Some(_) => return whole_reject(req, "malformed_response"),
    };

    let classes = evidence_classes(req);
    let mut seen_names: HashSet<String> = req.vocab.iter().map(|(name, _)| name.clone()).collect();
    let mut accepted: Vec<Accepted> = Vec::new();
    let mut rejected: Vec<Rejected> = Vec::new();

    for item in candidates {
        let obj = match item {
            Json::Obj(_) => item,
            // A non-object cannot carry a name, so it fails the first per-candidate rule.
            _ => {
                rejected.push(Rejected {
                    name: String::new(),
                    ids: Vec::new(),
                    reason: "missing_name".to_string(),
                });
                continue;
            }
        };

        let name = obj.get("name").and_then(Json::as_str).unwrap_or("");
        let ids = parse_ids(obj.get("ids"));

        if name.trim().is_empty() {
            rejected.push(Rejected {
                name: name.to_string(),
                ids: ids.values,
                reason: "missing_name".to_string(),
            });
            continue;
        }
        if seen_names.contains(name) {
            rejected.push(Rejected {
                name: name.to_string(),
                ids: ids.values,
                reason: "duplicate_name".to_string(),
            });
            continue;
        }
        // Remember every named candidate, accepted or not: "an earlier candidate name" is enough.
        seen_names.insert(name.to_string());

        // A candidate is an alternative (its own extension) plus an optional commitment. A defined
        // candidate's extension is computed from the request vocabulary; an extensional one keeps
        // the order it was sent in.
        let alt = match alternative(obj, req) {
            Ok(alt) => alt,
            Err(reason) => {
                rejected.push(Rejected {
                    name: name.to_string(),
                    ids: ids.values,
                    reason: reason.to_string(),
                });
                continue;
            }
        };
        let ext = alt.mask;
        let (prediction, rival) = match checked_commitment(obj, req, ext) {
            Ok(commitment) => commitment,
            Err(reason) => {
                rejected.push(Rejected {
                    name: name.to_string(),
                    // A defined candidate never supplies ids, so this echoes the empty list, exactly
                    // as the definition branch always has.
                    ids: ids.values,
                    reason: reason.to_string(),
                });
                continue;
            }
        };
        if accepted.len() >= req.limit {
            rejected.push(Rejected {
                name: name.to_string(),
                ids: ids.values,
                reason: "limit_exceeded".to_string(),
            });
            continue;
        }

        let ext_ids = match &alt.definition {
            Some(_) => ids_of(ext, req.n),
            None => ids.values.iter().map(|&x| x as usize).collect(),
        };
        let splits = count_splits(req.n, req.pos, req.neg, &classes, ext);

        // Optional confidence. It only orders abstention downstream; it never changes whether the
        // candidate is admitted.
        let score = match obj.get("score") {
            None | Some(Json::Null) => None,
            Some(value) => match value.as_f64() {
                Some(x) if (0.0..=1.0).contains(&x) => Some(x),
                _ => {
                    rejected.push(Rejected {
                        name: name.to_string(),
                        ids: ids.values,
                        reason: "bad_score".to_string(),
                    });
                    continue;
                }
            },
        };

        // Optional abstention. A declined object must be unmeasured (the evidence already has a
        // label for it) and outside the extension the candidate claims.
        let mut abstain: Vec<usize> = Vec::new();
        let mut abstain_reason: Option<&str> = None;
        match obj.get("abstain") {
            None | Some(Json::Null) => {}
            Some(Json::Arr(items)) => {
                for item in items {
                    let Some(raw) = item.as_i64() else {
                        abstain_reason = Some("bad_abstain");
                        break;
                    };
                    let Ok(id) = usize::try_from(raw) else {
                        abstain_reason = Some("abstain_out_of_range");
                        break;
                    };
                    if id >= req.n {
                        abstain_reason = Some("abstain_out_of_range");
                        break;
                    }
                    if req.pos & bit(id) != 0 || req.neg & bit(id) != 0 {
                        abstain_reason = Some("abstain_measured_object");
                        break;
                    }
                    if ext_ids.contains(&id) {
                        abstain_reason = Some("abstain_overlaps_extension");
                        break;
                    }
                    if abstain.contains(&id) {
                        abstain_reason = Some("abstain_duplicate");
                        break;
                    }
                    abstain.push(id);
                }
            }
            Some(_) => abstain_reason = Some("bad_abstain"),
        }
        if let Some(reason) = abstain_reason {
            rejected.push(Rejected {
                name: name.to_string(),
                ids: ids.values,
                reason: reason.to_string(),
            });
            continue;
        }

        accepted.push(Accepted {
            name: name.to_string(),
            ids: ext_ids,
            splits,
            warrant: obj.get("warrant").and_then(Json::as_str).map(str::to_string),
            falsifier: obj.get("falsifier").and_then(Json::as_str).map(str::to_string),
            definition: alt.definition,
            prediction,
            rival: rival.map(|mask| ids_of(mask, req.n)),
            score,
            abstain,
        });
    }

    Verdict {
        request_id: req.request_id.clone(),
        accepted,
        rejected,
        refused: None,
        whole_reject: None,
    }
}

fn validate(req: &Request, response_text: &str) -> String {
    verdict_json(&evaluate(req, response_text))
}

/// Parse a request and validate a response against it.
///
/// `Err` means the request file itself is unusable (a CLI error). A malformed, mistargeted, or
/// refused response is a successful validation that reports the verdict, exactly as `propose` does.
pub fn validate_response(request_text: &str, response_text: &str) -> Result<Verdict, String> {
    let req = parse_request(request_text)?;
    Ok(evaluate(&req, response_text))
}

// ---------------------------------------------------------------------------
// CLI entry point
// ---------------------------------------------------------------------------

/// Read both files, validate, print one line. Exit 2 only when the request itself cannot be used.
fn run(request_path: &str, response_path: &str) -> i32 {
    let request_text = match std::fs::read_to_string(request_path) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("metaform propose: cannot read request file `{request_path}`: {e}");
            return 2;
        }
    };
    let req = match parse_request(&request_text) {
        Ok(req) => req,
        Err(e) => {
            eprintln!("metaform propose: invalid request in `{request_path}`: {e}");
            return 2;
        }
    };

    // A missing or unreadable response is a protocol failure, not a CLI failure.
    let response_text = std::fs::read_to_string(response_path).unwrap_or_default();
    println!("{}", validate(&req, &response_text));
    0
}

fn arg_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

/// `metaform propose --request <path> --response <path>`
pub fn cmd(args: &[String]) -> i32 {
    match (arg_value(args, "--request"), arg_value(args, "--response")) {
        (Some(request), Some(response)) => run(&request, &response),
        _ => {
            eprintln!("usage: metaform propose --request <request.json> --response <response.json>");
            2
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    const REQ_VALID: &str = r#"{"protocol":"metaform.propose/1","request_id":"r-1","objects":3,"vocabulary":[],"evidence":{"pos":[0],"neg":[1]},"limit":8,"ask":"..."}"#;
    const RESP_VALID: &str = r#"{"protocol":"metaform.propose/1","request_id":"r-1","candidates":[{"name":"only0","ids":[0],"warrant":"object 0 is a positive","falsifier":"a negative class-mate"}]}"#;

    fn output(request: &str, response: &str) -> String {
        validate(&parse_request(request).expect("test request must parse"), response)
    }

    #[test]
    fn propose_accepts_valid_candidate_and_counts_splits() {
        // Empty vocabulary -> one class {0,1,2}; the evidence creates the colliding pair (0,1).
        // The candidate contains 0 but not 1, so it splits exactly that one pair.
        let got = output(REQ_VALID, RESP_VALID);
        let want = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-1","accepted":["#,
            r#"{"name":"only0","ids":[0],"splits":1,"warrant":"object 0 is a positive","falsifier":"a negative class-mate"}"#,
            r#"],"rejected":[],"refused":null}"#
        );
        assert_eq!(got, want);
    }

    #[test]
    fn propose_rejects_out_of_range_and_duplicate_names() {
        // p0 groups {0,1}: the pair (0,1) collides, and candidate c1 = {1} splits it.
        let req = r#"{"protocol":"metaform.propose/1","request_id":"r-2","objects":3,"vocabulary":[{"name":"p0","ids":[0,1]}],"evidence":{"pos":[0],"neg":[1]},"limit":8}"#;
        let resp = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-2","candidates":["#,
            r#"{"name":"p0","ids":[2]},"#,
            r#"{"name":"c1","ids":[1]},"#,
            r#"{"name":"c1","ids":[0]},"#,
            r#"{"name":"c2","ids":[5]}]}"#
        );
        let got = output(req, resp);
        let want = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-2","accepted":[{"name":"c1","ids":[1],"splits":1}],"#,
            r#""rejected":[{"name":"p0","ids":[2],"reason":"duplicate_name"},"#,
            r#"{"name":"c1","ids":[0],"reason":"duplicate_name"},"#,
            r#"{"name":"c2","ids":[5],"reason":"out_of_range"}],"refused":null}"#
        );
        assert_eq!(got, want);
    }

    const REQ_DEFINED: &str = r#"{"protocol":"metaform.propose/1","request_id":"r-3","objects":3,"vocabulary":[{"name":"a","ids":[0,1]},{"name":"b","ids":[0,2]}],"evidence":{"pos":[0],"neg":[1]},"limit":8}"#;

    #[test]
    fn propose_accepts_defined_all_of_and_reports_its_extension() {
        // a & b = {0}. The definition is a Boolean combination of a and b, so it is constant on the
        // classes a and b induce: no pair collides in this request, hence splits = 0.
        let resp = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-3","candidates":["#,
            r#"{"name":"p","define":{"all_of":["a","b"]},"warrant":"a and b","falsifier":"exactly one operand"}]}"#
        );
        let want = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-3","accepted":["#,
            r#"{"name":"p","ids":[0],"splits":0,"warrant":"a and b","falsifier":"exactly one operand"}"#,
            r#"],"rejected":[],"refused":null}"#
        );
        assert_eq!(output(REQ_DEFINED, resp), want);
    }

    #[test]
    fn propose_accepts_defined_any_of_and_reports_its_extension() {
        // a | b = {0,1,2}, which is not all 4 objects.
        let req = r#"{"protocol":"metaform.propose/1","request_id":"r-4","objects":4,"vocabulary":[{"name":"a","ids":[0,1]},{"name":"b","ids":[2]}],"evidence":{"pos":[0],"neg":[3]},"limit":8}"#;
        let resp = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-4","candidates":["#,
            r#"{"name":"q","define":{"any_of":["a","b"]}}]}"#
        );
        let want = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-4","accepted":["#,
            r#"{"name":"q","ids":[0,1,2],"splits":0}"#,
            r#"],"rejected":[],"refused":null}"#
        );
        assert_eq!(output(req, resp), want);
    }

    #[test]
    fn propose_rejects_definition_with_unknown_operand() {
        let resp = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-3","candidates":["#,
            r#"{"name":"p","define":{"all_of":["a","nope"]}}]}"#
        );
        let want = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-3","accepted":[],"#,
            r#""rejected":[{"name":"p","ids":[],"reason":"unknown_predicate"}],"refused":null}"#
        );
        assert_eq!(output(REQ_DEFINED, resp), want);
    }

    #[test]
    fn propose_rejects_definition_that_names_the_candidate_itself() {
        // `p` is a candidate name, not a vocabulary name: it does not exist yet.
        let resp = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-3","candidates":["#,
            r#"{"name":"p","define":{"any_of":["p"]}}]}"#
        );
        let want = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-3","accepted":[],"#,
            r#""rejected":[{"name":"p","ids":[],"reason":"unknown_predicate"}],"refused":null}"#
        );
        assert_eq!(output(REQ_DEFINED, resp), want);
    }

    #[test]
    fn propose_rejects_operand_defined_earlier_in_the_same_response() {
        // `q` is accepted in this response, but a definition may only use the request vocabulary.
        let resp = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-3","candidates":["#,
            r#"{"name":"q","define":{"all_of":["a","b"]}},"#,
            r#"{"name":"p","define":{"any_of":["q"]}}]}"#
        );
        let want = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-3","accepted":["#,
            r#"{"name":"q","ids":[0],"splits":0}"#,
            r#"],"rejected":[{"name":"p","ids":[],"reason":"unknown_predicate"}],"refused":null}"#
        );
        assert_eq!(output(REQ_DEFINED, resp), want);
    }

    #[test]
    fn propose_rejects_empty_definition() {
        let resp = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-3","candidates":["#,
            r#"{"name":"p","define":{"all_of":[]}}]}"#
        );
        let want = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-3","accepted":[],"#,
            r#""rejected":[{"name":"p","ids":[],"reason":"empty_definition"}],"refused":null}"#
        );
        assert_eq!(output(REQ_DEFINED, resp), want);
    }

    #[test]
    fn propose_rejects_duplicate_operand() {
        let resp = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-3","candidates":["#,
            r#"{"name":"p","define":{"all_of":["a","a"]}}]}"#
        );
        let want = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-3","accepted":[],"#,
            r#""rejected":[{"name":"p","ids":[],"reason":"duplicate_operand"}],"refused":null}"#
        );
        assert_eq!(output(REQ_DEFINED, resp), want);
    }

    #[test]
    fn propose_rejects_defined_candidate_with_a_taken_name() {
        let resp = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-3","candidates":["#,
            r#"{"name":"a","define":{"any_of":["b"]}}]}"#
        );
        let want = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-3","accepted":[],"#,
            r#""rejected":[{"name":"a","ids":[],"reason":"duplicate_name"}],"refused":null}"#
        );
        assert_eq!(output(REQ_DEFINED, resp), want);
    }

    #[test]
    fn propose_rejects_candidate_with_both_ids_and_define() {
        // The candidate form is ambiguous, so it is a candidate-level `wrong_protocol`; the supplied
        // ids are echoed but not used.
        let resp = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-3","candidates":["#,
            r#"{"name":"p","ids":[0],"define":{"all_of":["a","b"]}}]}"#
        );
        let want = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-3","accepted":[],"#,
            r#""rejected":[{"name":"p","ids":[0],"reason":"wrong_protocol"}],"refused":null}"#
        );
        assert_eq!(output(REQ_DEFINED, resp), want);
    }

    #[test]
    fn propose_rejects_defined_empty_and_constant_extensions() {
        let req = r#"{"protocol":"metaform.propose/1","request_id":"r-5","objects":4,"vocabulary":[{"name":"a","ids":[0,1]},{"name":"b","ids":[2,3]}],"evidence":{"pos":[0],"neg":[2]},"limit":8}"#;
        let resp = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-5","candidates":["#,
            r#"{"name":"none","define":{"all_of":["a","b"]}},"#,
            r#"{"name":"every","define":{"any_of":["a","b"]}}]}"#
        );
        let want = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-5","accepted":[],"#,
            r#""rejected":[{"name":"none","ids":[],"reason":"empty_extension"},"#,
            r#"{"name":"every","ids":[],"reason":"constant_extension"}],"refused":null}"#
        );
        assert_eq!(output(req, resp), want);
    }

    #[test]
    fn propose_rejects_whole_response_on_protocol_mismatch() {
        // The candidate would be valid, but rule 1 rejects the response before any candidate is read.
        let resp = r#"{"protocol":"metaform.propose/2","request_id":"r-1","candidates":[{"name":"only0","ids":[0]}]}"#;
        let got = output(REQ_VALID, resp);
        let want = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-1","accepted":[],"#,
            r#""rejected":[{"reason":"wrong_protocol"}],"refused":null}"#
        );
        assert_eq!(got, want);
    }

    // ---------------------------------------------------------------------
    // L11: a held-out prediction is required while an object is unmeasured.
    // ---------------------------------------------------------------------

    /// Objects 0 and 1 are measured; object 2 is named by `object_ids` and absent from `evidence`.
    const REQ_UNMEASURED: &str = r#"{"protocol":"metaform.propose/1","request_id":"r-p1","objects":3,"object_ids":[0,1,2],"vocabulary":[],"evidence":{"pos":[0],"neg":[1]},"limit":8}"#;

    /// Every object named by the request is measured: a prediction is optional here.
    const REQ_MEASURED: &str = r#"{"protocol":"metaform.propose/1","request_id":"r-p2","objects":2,"object_ids":[0,1],"vocabulary":[],"evidence":{"pos":[0],"neg":[1]},"limit":8}"#;

    #[test]
    fn propose_requires_a_prediction_for_an_unmeasured_object() {
        let resp = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-p1","candidates":["#,
            r#"{"name":"only0","ids":[0]}]}"#
        );
        let want = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-p1","accepted":[],"#,
            r#""rejected":[{"name":"only0","ids":[0],"reason":"missing_prediction"}],"refused":null}"#
        );
        assert_eq!(output(REQ_UNMEASURED, resp), want);

        // `null` is the absent spelling, so it is missing too.
        let resp = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-p1","candidates":["#,
            r#"{"name":"only0","ids":[0],"predict":null}]}"#
        );
        assert_eq!(output(REQ_UNMEASURED, resp), want);
    }

    #[test]
    fn propose_accepts_and_echoes_a_held_out_prediction() {
        let resp = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-p1","candidates":["#,
            r#"{"name":"only0","ids":[0],"predict":{"id":2,"label":"pos"}}]}"#
        );
        let want = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-p1","accepted":["#,
            r#"{"name":"only0","ids":[0],"splits":1,"predict":{"id":2,"label":"pos"}}"#,
            r#"],"rejected":[],"refused":null}"#
        );
        assert_eq!(output(REQ_UNMEASURED, resp), want);

        // A defined candidate carries the same field and the same echo.
        let req = r#"{"protocol":"metaform.propose/1","request_id":"r-p3","objects":3,"object_ids":[0,1,2],"vocabulary":[{"name":"a","ids":[0,1]},{"name":"b","ids":[0,2]}],"evidence":{"pos":[0],"neg":[1]},"limit":8}"#;
        let resp = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-p3","candidates":["#,
            r#"{"name":"p","define":{"all_of":["a","b"]},"predict":{"id":2,"label":"neg"}}]}"#
        );
        let want = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-p3","accepted":["#,
            r#"{"name":"p","ids":[0],"splits":0,"predict":{"id":2,"label":"neg"}}"#,
            r#"],"rejected":[],"refused":null}"#
        );
        assert_eq!(output(req, resp), want);
    }

    #[test]
    fn propose_rejects_a_prediction_out_of_range() {
        let resp = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-p1","candidates":["#,
            r#"{"name":"high","ids":[0],"predict":{"id":3,"label":"pos"}},"#,
            r#"{"name":"negative","ids":[0],"predict":{"id":-1,"label":"pos"}},"#,
            r#"{"name":"missing","ids":[0],"predict":{"label":"pos"}}]}"#
        );
        let want = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-p1","accepted":[],"#,
            r#""rejected":["#,
            r#"{"name":"high","ids":[0],"reason":"prediction_out_of_range"},"#,
            r#"{"name":"negative","ids":[0],"reason":"prediction_out_of_range"},"#,
            r#"{"name":"missing","ids":[0],"reason":"prediction_out_of_range"}],"refused":null}"#
        );
        assert_eq!(output(REQ_UNMEASURED, resp), want);
    }

    #[test]
    fn propose_rejects_a_prediction_on_a_measured_object() {
        let resp = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-p1","candidates":["#,
            r#"{"name":"known_pos","ids":[0],"predict":{"id":0,"label":"neg"}},"#,
            r#"{"name":"known_neg","ids":[0],"predict":{"id":1,"label":"pos"}}]}"#
        );
        let want = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-p1","accepted":[],"#,
            r#""rejected":["#,
            r#"{"name":"known_pos","ids":[0],"reason":"predicted_measured_object"},"#,
            r#"{"name":"known_neg","ids":[0],"reason":"predicted_measured_object"}],"refused":null}"#
        );
        assert_eq!(output(REQ_UNMEASURED, resp), want);
    }

    #[test]
    fn propose_rejects_a_prediction_with_a_bad_label() {
        let resp = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-p1","candidates":["#,
            r#"{"name":"word","ids":[0],"predict":{"id":2,"label":"yes"}},"#,
            r#"{"name":"absent","ids":[0],"predict":{"id":2}},"#,
            r#"{"name":"number","ids":[0],"predict":{"id":2,"label":1}}]}"#
        );
        let want = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-p1","accepted":[],"#,
            r#""rejected":["#,
            r#"{"name":"word","ids":[0],"reason":"prediction_bad_label"},"#,
            r#"{"name":"absent","ids":[0],"reason":"prediction_bad_label"},"#,
            r#"{"name":"number","ids":[0],"reason":"prediction_bad_label"}],"refused":null}"#
        );
        assert_eq!(output(REQ_UNMEASURED, resp), want);
    }

    #[test]
    fn propose_rejects_a_malformed_prediction_as_wrong_protocol() {
        let resp = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-p1","candidates":["#,
            r#"{"name":"scalar","ids":[0],"predict":"pos"}]}"#
        );
        let want = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-p1","accepted":[],"#,
            r#""rejected":[{"name":"scalar","ids":[0],"reason":"wrong_protocol"}],"refused":null}"#
        );
        assert_eq!(output(REQ_UNMEASURED, resp), want);
    }

    #[test]
    fn propose_prediction_is_optional_when_every_object_is_measured() {
        let resp = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-p2","candidates":["#,
            r#"{"name":"only0","ids":[0]}]}"#
        );
        let want = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-p2","accepted":["#,
            r#"{"name":"only0","ids":[0],"splits":1}"#,
            r#"],"rejected":[],"refused":null}"#
        );
        assert_eq!(output(REQ_MEASURED, resp), want);
    }

    #[test]
    fn propose_validates_an_optional_prediction_when_present() {
        let resp = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-p2","candidates":["#,
            r#"{"name":"only0","ids":[0],"predict":{"id":1,"label":"pos"}}]}"#
        );
        let want = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-p2","accepted":[],"#,
            r#""rejected":[{"name":"only0","ids":[0],"reason":"predicted_measured_object"}],"refused":null}"#
        );
        assert_eq!(output(REQ_MEASURED, resp), want);
    }

    /// Backwards compatibility: before L11 a request had no `object_ids`. Such a request declares no
    /// unmeasured object, so a prediction-free candidate keeps its old verdict.
    #[test]
    fn propose_treats_a_request_without_object_ids_as_having_no_unmeasured_object() {
        let resp = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-1","candidates":["#,
            r#"{"name":"only0","ids":[0]}]}"#
        );
        let want = concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"r-1","accepted":["#,
            r#"{"name":"only0","ids":[0],"splits":1}"#,
            r#"],"rejected":[],"refused":null}"#
        );
        assert_eq!(output(REQ_VALID, resp), want);
    }

    #[test]
    fn propose_is_pure() {
        let dir = temp_dir("purity");
        let req_path = dir.join("request.json");
        let resp_path = dir.join("response.json");
        std::fs::write(&req_path, REQ_VALID).expect("write request");
        std::fs::write(&resp_path, RESP_VALID).expect("write response");
        let before = file_names(&dir);

        let first = validate(&parse_request(REQ_VALID).unwrap(), RESP_VALID);
        let second = validate(&parse_request(REQ_VALID).unwrap(), RESP_VALID);
        assert_eq!(first, second, "the same inputs must produce the same output string");

        // Run the CLI path twice as well: it must not create `.metaform/` or any side file.
        assert_eq!(run(req_path.to_str().unwrap(), resp_path.to_str().unwrap()), 0);
        assert_eq!(run(req_path.to_str().unwrap(), resp_path.to_str().unwrap()), 0);
        assert_eq!(file_names(&dir), before, "propose must not write any file");
        assert!(!dir.join(".metaform").exists(), "propose must not create a state directory");

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("metaform-propose-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn file_names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .expect("read temp dir")
            .map(|entry| entry.expect("dir entry").file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

//! `metaform answer`: turn the open questions of an `explore` run into persisted laws.
//!
//! The loop this closes:
//!
//! 1. `explore --emit-questions q.json` writes the questions the evidence cannot decide.
//! 2. An expert (person or model) writes answers for those question ids.
//! 3. `answer --questions q.json --answers a.json` records the answers in `.metaform/laws.tsv`
//!    (or `--laws <path>`), and, for a `define` answer only, adopts the new predicate in
//!    `.metaform/state.json`.
//! 4. The next `explore` starts from those laws, so an answered premise is not asked again.
//!
//! Three answer forms are accepted, keyed by the `id` in the questions file:
//!
//! - `{"id":n,"answer":"yes"}` appends a `law` with source `expert` and the question's
//!   premise/conclusion.
//! - `{"id":n,"answer":"no","counterexample":{"id":k,"lacks":[names]}}` validates that object `k`
//!   really has the premise and lacks the named conclusion attributes, then appends a `refutation`
//!   with source `expert` and a note naming `k`. It never appends a law. The object itself is not
//!   added: the state is read-only for yes/no.
//! - `{"id":n,"answer":"define","name":"p","op":"all_of","operands":[names]}` adopts `p` through
//!   the same definition path as `state add --define`, then appends a `law` with source `define`.
//!
//! Every check runs before the first write, so an unknown id, a malformed answer, a counterexample
//! that does not refute, or an existing `define` name exits 2 and changes no file. Replaying the
//! same answers file does not duplicate a law line (dedupe on source + premise + conclusion).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::{bit, DefOp, Definition};
use serde::{Deserialize, Serialize};

use crate::laws::{self, Entry};
use crate::state::{PredSpec, State};

#[derive(Deserialize)]
struct QuestionsFile {
    #[serde(default)]
    #[allow(dead_code)]
    status: String,
    questions: Vec<Question>,
}

#[derive(Deserialize)]
struct Question {
    id: usize,
    premise: Vec<String>,
    conclusion: Vec<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum AnswersFile {
    Wrapped { answers: Vec<Answer> },
    Bare(Vec<Answer>),
    Single(Answer),
}

impl AnswersFile {
    fn into_answers(self) -> Vec<Answer> {
        match self {
            AnswersFile::Wrapped { answers } => answers,
            AnswersFile::Bare(answers) => answers,
            AnswersFile::Single(answer) => vec![answer],
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Answer {
    id: usize,
    answer: String,
    #[serde(default)]
    counterexample: Option<Counterexample>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    op: Option<String>,
    #[serde(default)]
    operands: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Counterexample {
    id: usize,
    lacks: Vec<String>,
}

#[derive(Serialize)]
struct Summary<'a> {
    answers: usize,
    laws: usize,
    refutations: usize,
    duplicates: usize,
    defined: &'a [String],
}

/// Record one validated `answers` document against `questions`, returning the summary JSON line.
///
/// This is the non-CLI entry point shared by `metaform answer` and the MCP `answer_questions` tool:
/// the two inputs are text, so no question/answer files have to exist. Every check still runs
/// before the first write, and replaying the same answers against the same questions dedupes.
pub fn apply_text(
    project: &Path,
    laws_path: Option<&Path>,
    questions_text: &str,
    answers_text: &str,
) -> Result<String, String> {
    let root = project.join(".metaform");
    let state_path = root.join("state.json");
    let laws_path = laws_path
        .map(|path| path.to_path_buf())
        .unwrap_or_else(|| root.join("laws.tsv"));
    if crate::same_path(&laws_path, &state_path) {
        return Err("--laws must not be the project state".into());
    }
    let mut st = crate::state::State::load(&state_path)?;
    if st.n > 64 {
        return Err(format!(
            "at most 64 objects are supported (state has {})",
            st.n
        ));
    }

    let questions_doc: QuestionsFile =
        serde_json::from_str(questions_text).map_err(|e| format!("cannot parse questions: {e}"))?;
    let mut questions: HashMap<usize, &Question> = HashMap::new();
    for question in &questions_doc.questions {
        if questions.insert(question.id, question).is_some() {
            return Err(format!(
                "questions file repeats question id {}",
                question.id
            ));
        }
    }

    let mut names: HashSet<String> = st.predicates.iter().map(|p| p.name.clone()).collect();
    for question in &questions_doc.questions {
        for name in question.premise.iter().chain(question.conclusion.iter()) {
            if !names.contains(name) {
                return Err(format!(
                    "question {} names unknown predicate `{name}`",
                    question.id
                ));
            }
        }
    }

    let answers_file: AnswersFile =
        serde_json::from_str(answers_text).map_err(|e| format!("cannot parse answers: {e}"))?;
    let answers = answers_file.into_answers();

    let mut seen_ids: HashSet<usize> = HashSet::new();
    let mut new_entries: Vec<Entry> = Vec::new();
    let mut defined: Vec<String> = Vec::new();
    for answer in &answers {
        if !seen_ids.insert(answer.id) {
            return Err(format!("more than one answer for question {}", answer.id));
        }
        let Some(question) = questions.get(&answer.id).copied() else {
            return Err(format!("unknown question id {}", answer.id));
        };
        stage(
            &mut st,
            &mut names,
            &mut defined,
            &mut new_entries,
            question,
            answer,
        )
        .map_err(|e| format!("question {}: {e}", answer.id))?;
    }

    // Every answer validated. Theory entries go first: if the state write fails afterwards,
    // replaying the same answers file still dedupes them and retries the definition.
    let appended = laws::append(&laws_path, &new_entries)?;
    if !defined.is_empty() {
        st.save(&state_path)
            .map_err(|e| format!("cannot write `{}`: {e}", state_path.display()))?;
    }

    let summary = Summary {
        answers: answers.len(),
        laws: appended.laws,
        refutations: appended.refutations,
        duplicates: appended.duplicates,
        defined: &defined,
    };
    Ok(serde_json::to_string(&summary).expect("the answer summary serializes"))
}

/// `metaform answer --project <dir> --questions <q.json> --answers <a.json> [--laws <path>]`
pub fn cmd(args: &[String]) -> i32 {
    let (questions_path, answers_path) = match (
        crate::flag_value(args, "--questions"),
        crate::flag_value(args, "--answers"),
    ) {
        (Ok(Some(q)), Ok(Some(a))) => (q, a),
        (Ok(_), Ok(_)) => {
            eprintln!(
                    "usage: metaform answer --project <dir> --questions <q.json> --answers <a.json> [--laws <path>]"
                );
            return 2;
        }
        (Err(e), _) | (_, Err(e)) => {
            eprintln!("metaform answer: {e}");
            return 2;
        }
    };

    let project = crate::project_root(args);
    let laws_path = match crate::flag_value(args, "--laws") {
        Ok(Some(path)) => Some(std::path::PathBuf::from(path)),
        Ok(None) => None,
        Err(e) => {
            eprintln!("metaform answer: {e}");
            return 2;
        }
    };
    let questions_text = match std::fs::read_to_string(&questions_path) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("metaform answer: cannot read questions `{questions_path}`: {e}");
            return 2;
        }
    };
    let answers_text = match std::fs::read_to_string(&answers_path) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("metaform answer: cannot read answers `{answers_path}`: {e}");
            return 2;
        }
    };

    match apply_text(
        &project,
        laws_path.as_deref(),
        &questions_text,
        &answers_text,
    ) {
        Ok(line) => {
            println!("{line}");
            0
        }
        Err(e) => {
            eprintln!("metaform answer: {e}");
            2
        }
    }
}

/// Validate one answer and stage its effects. Nothing is written here: `st` and the vectors are
/// in-memory until every answer has passed.
fn stage(
    st: &mut State,
    names: &mut HashSet<String>,
    defined: &mut Vec<String>,
    new_entries: &mut Vec<Entry>,
    question: &Question,
    answer: &Answer,
) -> Result<(), String> {
    match answer.answer.as_str() {
        "yes" => {
            if answer.counterexample.is_some()
                || answer.name.is_some()
                || answer.op.is_some()
                || answer.operands.is_some()
            {
                return Err("a yes answer carries no counterexample, name, op or operands".into());
            }
            new_entries.push(Entry::law(
                "expert",
                question.premise.clone(),
                question.conclusion.clone(),
                format!("answered yes to question {}", answer.id),
            ));
        }
        "no" => {
            if answer.name.is_some() || answer.op.is_some() || answer.operands.is_some() {
                return Err("a no answer carries no name, op or operands".into());
            }
            let counterexample = answer
                .counterexample
                .as_ref()
                .ok_or_else(|| "a no answer needs a counterexample".to_string())?;
            validate_counterexample(st, question, counterexample)?;
            // A refutation is its own record, not a law: the next explore must not derive the
            // answered implication from it, only treat the premise as decided.
            new_entries.push(Entry::refutation(
                "expert",
                question.premise.clone(),
                question.conclusion.clone(),
                format!(
                    "refuted by object {}: lacks {}",
                    counterexample.id,
                    counterexample.lacks.join(",")
                ),
            ));
        }
        "define" => {
            if answer.counterexample.is_some() {
                return Err("a define answer carries no counterexample".into());
            }
            let name = answer
                .name
                .as_deref()
                .ok_or_else(|| "a define answer needs a name".to_string())?;
            if name.is_empty() {
                return Err("a define answer needs a non-empty name".into());
            }
            if names.contains(name) {
                return Err(format!("predicate `{name}` already exists"));
            }
            let op = parse_op(
                answer
                    .op
                    .as_deref()
                    .ok_or_else(|| "a define answer needs an op".to_string())?,
            )?;
            let operands = answer
                .operands
                .as_ref()
                .ok_or_else(|| "a define answer needs operands".to_string())?;
            if operands.is_empty() {
                return Err("a define answer needs at least one operand".into());
            }
            for operand in operands {
                if !names.contains(operand) {
                    return Err(format!("unknown operand `{operand}`"));
                }
            }
            let op_label = match op {
                DefOp::AllOf => "all_of",
                DefOp::AnyOf => "any_of",
            };
            let definition = Definition {
                op,
                operands: operands.clone(),
            };
            st.predicates.push(PredSpec {
                name: name.to_string(),
                ids: Vec::new(),
                definition: Some(definition),
            });
            if let Err(e) = st.resolve() {
                return Err(format!("cannot adopt the definition: {e}"));
            }
            names.insert(name.to_string());
            defined.push(name.to_string());
            new_entries.push(Entry::law(
                "define",
                question.premise.clone(),
                question.conclusion.clone(),
                format!("defined `{name}` = {op_label}:{}", operands.join(",")),
            ));
        }
        other => {
            return Err(format!(
                "unknown answer `{other}` (expected yes, no or define)"
            ))
        }
    }
    Ok(())
}

/// `k` must be a real object that has every premise attribute and lacks at least one conclusion
/// attribute. Each name in `lacks` must be one of the question's conclusions and be absent from
/// `k`'s intent, so a stale or invented counterexample cannot be persisted.
fn validate_counterexample(
    st: &State,
    question: &Question,
    counterexample: &Counterexample,
) -> Result<(), String> {
    if counterexample.lacks.is_empty() {
        return Err("the counterexample lists no lacking attribute".into());
    }
    if counterexample.id >= st.n {
        return Err(format!(
            "counterexample object {} is out of range (object count {})",
            counterexample.id, st.n
        ));
    }
    let explorer = st.explorer();
    let mut intent: HashSet<&str> = HashSet::new();
    for predicate in &explorer.vocab.preds {
        if predicate.ext & bit(counterexample.id) != 0 {
            intent.insert(predicate.name.as_str());
        }
    }
    for name in &question.premise {
        if !intent.contains(name.as_str()) {
            return Err(format!(
                "object {} does not have premise attribute `{name}`",
                counterexample.id
            ));
        }
    }
    let conclusion: HashSet<&str> = question.conclusion.iter().map(String::as_str).collect();
    for name in &counterexample.lacks {
        if !conclusion.contains(name.as_str()) {
            return Err(format!(
                "`{name}` is not one of the question's conclusion attributes"
            ));
        }
        if intent.contains(name.as_str()) {
            return Err(format!(
                "object {} has `{name}`, so it does not refute the implication",
                counterexample.id
            ));
        }
    }
    Ok(())
}

fn parse_op(raw: &str) -> Result<DefOp, String> {
    match raw.trim() {
        "all_of" | "all-of" => Ok(DefOp::AllOf),
        "any_of" | "any-of" => Ok(DefOp::AnyOf),
        other => Err(format!(
            "unknown definition operator `{other}` (expected all_of or any_of)"
        )),
    }
}

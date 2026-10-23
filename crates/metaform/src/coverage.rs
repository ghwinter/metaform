//! `metaform coverage`: turn a goal-coverage table into the adjacent work items.
//!
//! The input is a tab-separated table. The first line is the header: `item` followed by the
//! requirement names (a trailing `!` marks a blocking requirement and is stripped from every
//! output name). Each later line is one work item; its cells are `yes`, `no`, `?`, or empty
//! (empty is `?`). Blank lines and lines whose first non-space character is `#` are skipped.
//!
//! The implication test is the same extent/closure test `explore` uses, with a support floor.
//! Write `extent(X)` for the rows whose cell for X is `yes`. An implication `X -> Y` is
//! *supported* when `extent(X) ∩ extent(Y)` contains at least two rows. Its conflicts are the rows
//! in `extent(X)` that are not in `extent(Y)`: a `no` contradicts the pattern and an unknown
//! leaves it unresolved, so both are work items. The support floor is what stops one coincidental
//! row from manufacturing a rule; a `no` row does not silently erase a pattern supported by two
//! other rows, because that row is exactly the thing a person needs to look at.
//!
//! Two rows are equivalent when they have the same yes/no/? pattern across every requirement: an
//! unknown is not the same answer as a `no`. Equivalence groups of size one are omitted.
//!
//! The command is read-only. It parses and validates the whole table before printing anything, so
//! a malformed table exits 2 with an empty stdout and no file touched.

use std::collections::HashSet;

use serde::Serialize;

/// One cell of the table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    Yes,
    No,
    /// `?` or an empty cell: the fact is not known today.
    Unknown,
}

impl Cell {
    /// Parse one trimmed cell. `None` means the token is not a legal cell.
    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "" | "?" => Some(Cell::Unknown),
            "yes" => Some(Cell::Yes),
            "no" => Some(Cell::No),
            _ => None,
        }
    }
}

/// A requirement column. `name` is the output spelling, with the blocking marker removed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Requirement {
    pub name: String,
    pub blocking: bool,
}

/// One work item and its cells, in header order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub item: String,
    pub cells: Vec<Cell>,
}

/// A parsed coverage table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    pub requirements: Vec<Requirement>,
    pub rows: Vec<Row>,
}

/// Parse the TSV. Every structural problem is an error; nothing is repaired silently.
pub fn parse(text: &str) -> Result<Table, String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut requirements: Option<Vec<Requirement>> = None;
    let mut rows: Vec<Row> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for (index, raw) in text.lines().enumerate() {
        let lineno = index + 1;
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let cells: Vec<String> = line
            .split('\t')
            .map(|cell| cell.trim().to_string())
            .collect();

        let Some(header) = requirements.as_mut() else {
            // The first non-skipped line must be the header.
            if cells[0] != "item" {
                return Err(format!(
                    "line {lineno}: missing header (first column is `{}`, expected `item`)",
                    cells[0]
                ));
            }
            let mut parsed = Vec::new();
            for (column, name) in cells.iter().enumerate().skip(1) {
                let (name, blocking) = match name.strip_suffix('!') {
                    Some(stripped) => (stripped, true),
                    None => (name.as_str(), false),
                };
                if name.is_empty() {
                    return Err(format!(
                        "line {lineno}: requirement name in column {} is empty",
                        column + 1
                    ));
                }
                if !seen.insert(name.to_string()) {
                    return Err(format!("line {lineno}: duplicate requirement `{name}`"));
                }
                parsed.push(Requirement {
                    name: name.to_string(),
                    blocking,
                });
            }
            requirements = Some(parsed);
            continue;
        };

        if cells[0].is_empty() {
            return Err(format!("line {lineno}: item name is empty"));
        }
        if cells.len() > header.len() + 1 {
            return Err(format!(
                "line {lineno}: unknown requirement in column {} (the header names {} requirement(s))",
                cells.len(),
                header.len()
            ));
        }
        let mut row_cells = Vec::with_capacity(header.len());
        for (column, requirement) in header.iter().enumerate() {
            let raw_cell = cells.get(column + 1).map(String::as_str).unwrap_or("");
            let cell = Cell::parse(raw_cell).ok_or_else(|| {
                format!(
                    "line {lineno}: `{raw_cell}` is not a cell value for requirement `{}` (use yes, no, ?, or empty)",
                    requirement.name
                )
            })?;
            row_cells.push(cell);
        }
        rows.push(Row {
            item: cells[0].clone(),
            cells: row_cells,
        });
    }

    let Some(requirements) = requirements else {
        return Err("missing header: the table has no `item` header row".into());
    };
    Ok(Table { requirements, rows })
}

/// One unanswered cell.
#[derive(Clone, Debug, Serialize)]
pub struct Missing {
    pub item: String,
    pub requirement: String,
}

/// One cell that contradicts (or cannot be read against) a supported implication.
#[derive(Clone, Debug, Serialize)]
pub struct Conflict {
    pub item: String,
    pub requirement: String,
    pub implied_by: Vec<String>,
    pub reason: String,
}

/// A blocking requirement with at least one unanswered cell.
#[derive(Clone, Debug, Serialize)]
pub struct Blocking {
    pub requirement: String,
    pub missing_in: Vec<String>,
    pub why: String,
}

/// The one JSON object the command prints.
#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub items: usize,
    pub requirements: Vec<String>,
    pub missing: Vec<Missing>,
    pub conflicts: Vec<Conflict>,
    pub equivalences: Vec<Vec<String>>,
    pub blocking: Vec<Blocking>,
}

/// Reason used for a blocking cell; it is also the reason printed by `--format text`.
pub const BLOCKING_WHY: &str = "blocking requirement is unanswered";

/// Analyze a parsed table. Output order is input order (rows, then requirement columns), so the
/// same table always gives the same bytes.
pub fn analyze(table: &Table) -> Report {
    let requirements = &table.requirements;
    let names: Vec<String> = requirements.iter().map(|r| r.name.clone()).collect();
    let n = requirements.len();
    let row_count = table.rows.len();

    let mut missing = Vec::new();
    for row in &table.rows {
        for (column, cell) in row.cells.iter().enumerate() {
            if *cell == Cell::Unknown {
                missing.push(Missing {
                    item: row.item.clone(),
                    requirement: names[column].clone(),
                });
            }
        }
    }

    // extent(X): the rows whose cell for X is yes. This is the same extent the exploration
    // closure uses; the support floor is the only extra condition.
    let extents: Vec<Vec<usize>> = (0..n)
        .map(|column| {
            (0..row_count)
                .filter(|&row| table.rows[row].cells[column] == Cell::Yes)
                .collect()
        })
        .collect();

    // Row-major so a person reads one work item at a time. Within a row, requirement columns are
    // visited in header order, and the implying requirements after that.
    let mut conflicts = Vec::new();
    for row in &table.rows {
        for (conclusion, cell) in row.cells.iter().enumerate() {
            if *cell == Cell::Yes {
                continue;
            }
            for (premise, premise_cell) in row.cells.iter().enumerate() {
                if premise == conclusion || *premise_cell != Cell::Yes {
                    continue;
                }
                let support: Vec<usize> = extents[premise]
                    .iter()
                    .copied()
                    .filter(|&other| table.rows[other].cells[conclusion] == Cell::Yes)
                    .collect();
                if support.len() < 2 {
                    continue;
                }
                let implied_by: Vec<String> = support
                    .iter()
                    .map(|&other| table.rows[other].item.clone())
                    .collect();
                let state = if *cell == Cell::No { "no" } else { "?" };
                let reason = format!(
                    "implication {} -> {} (supported by {}); this row has {}={state}",
                    names[premise],
                    names[conclusion],
                    implied_by.join(", "),
                    names[conclusion]
                );
                conflicts.push(Conflict {
                    item: row.item.clone(),
                    requirement: names[conclusion].clone(),
                    implied_by,
                    reason,
                });
            }
        }
    }

    // Identical ternary patterns: an unknown is not the same answer as a no.
    let mut groups: Vec<(Vec<Cell>, Vec<usize>)> = Vec::new();
    for (row_index, row) in table.rows.iter().enumerate() {
        if let Some((_, members)) = groups.iter_mut().find(|(cells, _)| cells == &row.cells) {
            members.push(row_index);
        } else {
            groups.push((row.cells.clone(), vec![row_index]));
        }
    }
    let equivalences: Vec<Vec<String>> = groups
        .into_iter()
        .filter(|(_, members)| members.len() >= 2)
        .map(|(_, members)| {
            members
                .into_iter()
                .map(|row| table.rows[row].item.clone())
                .collect()
        })
        .collect();

    let mut blocking = Vec::new();
    for (column, requirement) in requirements.iter().enumerate() {
        if !requirement.blocking {
            continue;
        }
        let missing_in: Vec<String> = table
            .rows
            .iter()
            .filter(|row| row.cells[column] == Cell::Unknown)
            .map(|row| row.item.clone())
            .collect();
        if !missing_in.is_empty() {
            blocking.push(Blocking {
                requirement: requirement.name.clone(),
                missing_in,
                why: BLOCKING_WHY.to_string(),
            });
        }
    }

    Report {
        items: row_count,
        requirements: names,
        missing,
        conflicts,
        equivalences,
        blocking,
    }
}

/// One JSON object, one line.
pub fn to_json(report: &Report) -> String {
    serde_json::to_string(report).expect("the coverage report is serializable")
}

/// One line per problem, in the style `item: requirement: reason`. Missing cells and blocking
/// cells are separate problems, so a cell that is both appears twice.
pub fn to_text(report: &Report) -> String {
    let mut out = String::new();
    for missing in &report.missing {
        out.push_str(&format!(
            "{}: {}: missing\n",
            missing.item, missing.requirement
        ));
    }
    for conflict in &report.conflicts {
        out.push_str(&format!(
            "{}: {}: {}\n",
            conflict.item, conflict.requirement, conflict.reason
        ));
    }
    for block in &report.blocking {
        for item in &block.missing_in {
            out.push_str(&format!("{item}: {}: {}\n", block.requirement, block.why));
        }
    }
    out
}

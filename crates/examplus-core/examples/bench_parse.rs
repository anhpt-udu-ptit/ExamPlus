//! Reproducible DOCX parser & answer-key evaluator harness for ExamPlus benchmarks.
//! Reads paths via CLI args or `--file-list <path>` and writes JSONL lines to stdout.

use examplus_core::parser::answer_key::AnswerKeyMeta;
use examplus_core::parser::docx::{BlockProvenance, ParsedQuestion, ProvenanceSummary};
use examplus_core::parser::{parse_answer_key_file, parse_exam_file};
use serde::Serialize;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

#[derive(Serialize)]
struct EvalQuestionOut {
    number: u32,
    part: String,
    raw_prompt: String,
    html_content: String,
    options: Vec<EvalOptionOut>,
    short_answer: Option<String>,
    provenance: Vec<BlockProvenance>,
    needs_review: bool,
    review_reasons: Vec<String>,
}

#[derive(Serialize)]
struct EvalOptionOut {
    label: String,
    raw_content: String,
    html_content: String,
    is_correct: Option<bool>,
}

#[derive(Serialize)]
struct EvalAnswerKeyVariant {
    part1: BTreeMap<u32, String>,
    part2: BTreeMap<u32, EvalTfOut>,
    part3: BTreeMap<u32, String>,
    part1_meta: BTreeMap<u32, AnswerKeyMeta>,
    part2_meta: BTreeMap<u32, AnswerKeyMeta>,
    part3_meta: BTreeMap<u32, AnswerKeyMeta>,
}

#[derive(Serialize)]
struct EvalTfOut {
    a: bool,
    b: bool,
    c: bool,
    d: bool,
}

#[derive(Serialize)]
struct EvalDocOut {
    path: String,
    parse_ok: bool,
    parse_error: Option<String>,
    title: String,
    subject: Option<String>,
    exam_type: String,
    source_format: Option<String>,
    variant_code: Option<String>,
    duration_minutes: Option<u32>,
    part1_count: usize,
    part2_count: usize,
    part3_count: usize,
    total_questions: usize,
    media_count: usize,
    provenance_summary: ProvenanceSummary,
    questions: Vec<EvalQuestionOut>,
    answer_key_ok: bool,
    answer_key_error: Option<String>,
    answer_keys: BTreeMap<String, EvalAnswerKeyVariant>,
}

fn convert_q(part_name: &str, q: &ParsedQuestion) -> EvalQuestionOut {
    EvalQuestionOut {
        number: q.number,
        part: part_name.to_string(),
        raw_prompt: q.raw_prompt.clone(),
        html_content: q.html_content.clone(),
        options: q
            .options
            .iter()
            .map(|o| EvalOptionOut {
                label: o.label.clone(),
                raw_content: o.raw_content.clone(),
                html_content: o.html_content.clone(),
                is_correct: o.is_correct,
            })
            .collect(),
        short_answer: q.short_answer.clone(),
        provenance: q.provenance.clone(),
        needs_review: q.needs_review,
        review_reasons: q.review_reasons.clone(),
    }
}

fn eval_one(path: &PathBuf) -> EvalDocOut {
    let path_str = path.to_string_lossy().to_string();
    let parsed_res = parse_exam_file(path).map(|(_, parsed)| parsed);
    let ak_res = parse_answer_key_file(path);

    let (answer_key_ok, answer_key_error, answer_keys) = match ak_res {
        Ok(map) => {
            let mut out = BTreeMap::new();
            for (vcode, k) in map {
                let p1: BTreeMap<u32, String> = k.part1_answers.into_iter().collect();
                let p2: BTreeMap<u32, EvalTfOut> = k
                    .part2_answers
                    .into_iter()
                    .map(|(n, tf)| {
                        (
                            n,
                            EvalTfOut {
                                a: tf.a,
                                b: tf.b,
                                c: tf.c,
                                d: tf.d,
                            },
                        )
                    })
                    .collect();
                let p3: BTreeMap<u32, String> = k.part3_answers.into_iter().collect();
                out.insert(
                    vcode,
                    EvalAnswerKeyVariant {
                        part1: p1,
                        part2: p2,
                        part3: p3,
                        part1_meta: k.part1_meta.into_iter().collect(),
                        part2_meta: k.part2_meta.into_iter().collect(),
                        part3_meta: k.part3_meta.into_iter().collect(),
                    },
                );
            }
            (true, None, out)
        }
        Err(e) => (false, Some(e.to_string()), BTreeMap::new()),
    };

    match parsed_res {
        Ok(exam) => {
            let mut questions = Vec::new();
            for q in &exam.part1_questions {
                questions.push(convert_q("PART_1", q));
            }
            for q in &exam.part2_questions {
                questions.push(convert_q("PART_2", q));
            }
            for q in &exam.part3_questions {
                questions.push(convert_q("PART_3", q));
            }
            let total_questions = questions.len();
            EvalDocOut {
                path: path_str,
                parse_ok: true,
                parse_error: None,
                title: exam.title,
                subject: exam.subject,
                exam_type: format!("{:?}", exam.exam_type),
                source_format: exam.source_format,
                variant_code: exam.variant_code,
                duration_minutes: exam.duration_minutes,
                part1_count: exam.part1_questions.len(),
                part2_count: exam.part2_questions.len(),
                part3_count: exam.part3_questions.len(),
                total_questions,
                media_count: exam.media_files.len(),
                provenance_summary: exam.provenance_summary,
                questions,
                answer_key_ok,
                answer_key_error,
                answer_keys,
            }
        }
        Err(e) => EvalDocOut {
            path: path_str,
            parse_ok: false,
            parse_error: Some(e.to_string()),
            title: String::new(),
            subject: None,
            exam_type: String::new(),
            source_format: None,
            variant_code: None,
            duration_minutes: None,
            part1_count: 0,
            part2_count: 0,
            part3_count: 0,
            total_questions: 0,
            media_count: 0,
            provenance_summary: ProvenanceSummary::default(),
            questions: Vec::new(),
            answer_key_ok,
            answer_key_error,
            answer_keys,
        },
    }
}

fn main() {
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--file-list" {
            if let Some(list_path) = args.next() {
                if let Ok(content) = fs::read_to_string(&list_path) {
                    for line in content.lines() {
                        let trimmed = line.trim().trim_start_matches('\u{FEFF}');
                        if !trimmed.is_empty() {
                            paths.push(PathBuf::from(trimmed));
                        }
                    }
                }
            }
        } else {
            paths.push(PathBuf::from(arg));
        }
    }

    let stdout = io::stdout();
    let mut handle = io::BufWriter::new(stdout.lock());
    for p in &paths {
        let out = eval_one(p);
        if let Ok(line) = serde_json::to_string(&out) {
            let _ = writeln!(handle, "{}", line);
        }
    }
    let _ = handle.flush();
}

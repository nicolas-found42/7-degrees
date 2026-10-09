//! Experiment tooling: all judgments and policies use the application's public services.
use app_server::{JevHandle, query, ranking, report_data, resolution, search::PlayerCatalog};
use graph_core::{Day, LocatedTenure, Player, RosterData, Team, TeammateGraph, Tenure};
use jev_client::{
    JevAnswer, JevClient, JevConfig, JevEvaluation, JevOutcome, JevQuestion, JevRequest,
    JevTransport, http_transport::HttpJevTransport,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeSet, VecDeque},
    path::Path,
    sync::{Arc, Mutex},
    time::Instant,
};
#[derive(Clone, Deserialize, Serialize)]
struct Case {
    id: String,
    component: String,
    split: String,
    category: String,
    text: String,
    expected: String,
    context: Option<String>,
}
#[derive(Clone, Copy, Deserialize, Serialize)]
struct Policy {
    resolution: f64,
    existence: f64,
    query: f64,
    ranking: f64,
    margin: f64,
}
#[derive(Clone)]
struct Packet {
    request: JevRequest,
    evaluation: JevEvaluation,
}
#[derive(Clone)]
struct Capture {
    packets: Arc<Mutex<Vec<Packet>>>,
    replay: Option<Arc<Mutex<VecDeque<Packet>>>>,
}
impl JevTransport for Capture {
    fn evaluate(&self, c: &JevConfig, r: &JevRequest) -> JevOutcome {
        self.evaluate_measured(c, r).outcome
    }
    fn evaluate_measured(&self, c: &JevConfig, r: &JevRequest) -> JevEvaluation {
        let started = Instant::now();
        let mut e = if let Some(replay) = &self.replay {
            let p = replay.lock().unwrap().pop_front();
            match p {
                Some(p) if request_value(&p.request) == request_value(r) => p.evaluation,
                _ => JevEvaluation::unavailable(),
            }
        } else {
            HttpJevTransport::new().evaluate_measured(c, r)
        };
        e.elapsed_ms = started.elapsed().as_secs_f64() * 1000.;
        self.packets.lock().unwrap().push(Packet {
            request: r.clone(),
            evaluation: e.clone(),
        });
        e
    }
}
fn request_value(r: &JevRequest) -> Value {
    let qs=r.questions.iter().map(|(id,q)|{let v=match q{
  JevQuestion::Choice{instructions,criteria}=>json!({"type":"choice","instructions":instructions,"criteria":criteria}),
  JevQuestion::Noul{instructions,criteria}=>json!({"type":"noul","instructions":instructions,"criteria":criteria.as_ref().map(|x|json!({"yes":x.yes,"no":x.no}))}),
  JevQuestion::Score{instructions,criteria}=>json!({"type":"score","instructions":instructions,"criteria":criteria})};json!({"id":id,"question":v})}).collect::<Vec<_>>();
    json!({"state":r.state,"questions":qs})
}
fn outcome_value(o: &JevOutcome) -> Value {
    match o {
        JevOutcome::Unavailable => json!({"status":"unavailable"}),
        JevOutcome::Answers(a) => {
            json!({"status":"answers","answers":a.iter().map(|(id,a)|{let v=match a{JevAnswer::Choice(s,p,c)=>json!({"type":"choice","selected":s,"probabilities":p,"confidence":c}),JevAnswer::Noul(p)=>json!({"type":"noul","probability":p}),JevAnswer::Score(s,p,c)=>json!({"type":"score","score":s,"probabilities":p,"confidence":c})};json!({"id":id,"answer":v})}).collect::<Vec<_>>()})
        }
    }
}
fn rc(p: Policy, variant: &str) -> resolution::ResolutionConfig {
    resolution::ResolutionConfig {
        choice_probability: p.resolution,
        choice_confidence: p.resolution,
        existence_probability: p.existence,
        margin: p.margin,
        prompt_variant: if variant == "wording" {
            resolution::PromptVariant::ExplicitIdentity
        } else {
            resolution::PromptVariant::Baseline
        },
        option_order: if variant == "order" {
            resolution::OptionOrder::Reverse
        } else {
            resolution::OptionOrder::Lexical
        },
        ..Default::default()
    }
}
fn qc(p: Policy, variant: &str) -> query::QueryConfig {
    query::QueryConfig {
        minimum_probability: p.query,
        minimum_confidence: p.query,
        minimum_margin: p.margin,
        resolution: rc(p, variant),
        prompt_variant: if variant == "wording" {
            query::PromptVariant::Paraphrase
        } else {
            query::PromptVariant::Direct
        },
        option_order: if variant == "order" {
            query::OptionOrder::Reversed
        } else {
            query::OptionOrder::Natural
        },
    }
}
fn kc(p: Policy, variant: &str) -> ranking::RankingConfig {
    ranking::RankingConfig {
        minimum_confidence: p.ranking,
        prompt_variant: if variant == "wording" {
            ranking::RankingPromptVariant::GroundedPreference
        } else {
            ranking::RankingPromptVariant::Baseline
        },
        order: if variant == "order" {
            ranking::RankingOrder::Reverse
        } else {
            ranking::RankingOrder::Deterministic
        },
        ..Default::default()
    }
}
struct Context {
    graph: TeammateGraph,
    catalog: PlayerCatalog,
    reports: report_data::ReportMetadata,
    rank_graph: TeammateGraph,
    rank_catalog: PlayerCatalog,
}
fn ranking_fixture() -> (TeammateGraph, PlayerCatalog) {
    let names = [
        ("s", "Start", 2000),
        ("b", "Beta", 1960),
        ("c", "Gamma", 1990),
        ("g", "Goal", 2000),
    ];
    let mut roster = RosterData {
        players: names
            .iter()
            .map(|(id, n, _)| Player {
                id: (*id).into(),
                name: (*n).into(),
            })
            .collect(),
        ..Default::default()
    };
    for (i, (a, b)) in [("s", "b"), ("b", "g"), ("s", "c"), ("c", "g")]
        .iter()
        .enumerate()
    {
        let team = format!("T{i}");
        roster.teams.push(Team {
            id: team.clone(),
            name: team.clone(),
        });
        for p in [a, b] {
            roster.tenures.push(LocatedTenure {
                player: (*p).into(),
                team: team.clone(),
                tenure: Tenure {
                    start: Day(i as u32 * 10 + 1),
                    end: Day(i as u32 * 10 + 2),
                },
            })
        }
    }
    let catalog = PlayerCatalog::new(
        names
            .iter()
            .map(|(id, n, y)| app_server::search::SearchPlayer {
                id: (*id).into(),
                name: (*n).into(),
                aliases: vec![],
                first_season: Some(*y),
                last_season: Some(2000),
                teams: vec![],
            })
            .collect(),
    );
    (TeammateGraph::build(roster), catalog)
}
fn run(
    ctx: &Context,
    c: &Case,
    p: Policy,
    variant: &str,
    key: &str,
    replay: Option<Vec<Packet>>,
) -> (Value, Vec<Packet>) {
    let cap = Capture {
        packets: Arc::default(),
        replay: replay.map(|v| Arc::new(Mutex::new(v.into()))),
    };
    let handle = JevHandle::from_client(JevClient::new(JevConfig::new(key.into()), cap.clone()));
    let start = Instant::now();
    let result = match c.component.as_str() {
        "resolution" => serde_json::to_value(
            resolution::resolve(
                &ctx.catalog,
                &handle,
                &resolution::ResolutionInput {
                    mention: c.text.clone(),
                    context: c.context.clone(),
                },
                &rc(p, variant),
            )
            .expect("valid resolution case")
            .result,
        )
        .unwrap(),
        "query" => serde_json::to_value(
            query::execute(
                &ctx.graph,
                &ctx.catalog,
                Some(&ctx.reports),
                &handle,
                &c.text,
                &qc(p, variant),
            )
            .expect("valid query case")
            .result,
        )
        .unwrap(),
        "ranking" => serde_json::to_value(
            ranking::rank(
                &ctx.rank_graph,
                &ctx.rank_catalog,
                &handle,
                &ranking::RankingInput {
                    from: "s".into(),
                    to: "g".into(),
                    interest: c.text.clone(),
                    cursor: "v1:0".into(),
                    limit: 20,
                },
                &kc(p, variant),
            )
            .expect("valid ranking case")
            .result,
        )
        .unwrap(),
        _ => panic!("unknown component"),
    };
    let elapsed = start.elapsed().as_secs_f64() * 1000.;
    let packets = cap.packets.lock().unwrap().clone();
    (
        json!({"case":c,"variant":variant,"policy":p,"elapsed_ms":elapsed,"result":result,"calls":packets.iter().map(|p|json!({"request":request_value(&p.request),"outcome":outcome_value(&p.evaluation.outcome),"metadata":p.evaluation.metadata,"elapsed_ms":p.evaluation.elapsed_ms})).collect::<Vec<_>>() }),
        packets,
    )
}
fn prediction(c: &Case, r: &Value) -> String {
    let r = &r["result"];
    match c.component.as_str() {
        "resolution" => r["player"]["id"]
            .as_str()
            .unwrap_or_else(|| r["status"].as_str().unwrap())
            .into(),
        "query" => {
            if matches!(r["status"].as_str(), Some("executed" | "unsupported")) {
                r["operation"].as_str().unwrap_or("abstain").into()
            } else {
                "abstain".into()
            }
        }
        "ranking" => {
            if r["status"] == "ranked" {
                r["chains"][0]["chain"]["path"][1]["id"]
                    .as_str()
                    .unwrap()
                    .into()
            } else {
                "abstain".into()
            }
        }
        _ => unreachable!(),
    }
}
fn false_match(c: &Case, pred: &str) -> bool {
    if c.component == "resolution" {
        !matches!(pred, "no_match" | "clarification" | "unavailable") && pred != c.expected
    } else {
        pred != "abstain" && pred != c.expected
    }
}
fn validate(cases: &[Case]) {
    let mut ids = BTreeSet::new();
    let mut inputs = BTreeSet::new();
    for c in cases {
        assert!(ids.insert(&c.id));
        assert!(
            inputs.insert((&c.component, &c.text, &c.context)),
            "split inputs must not overlap"
        );
        assert!(matches!(c.split.as_str(), "calibration" | "heldout"));
        assert!(matches!(
            c.component.as_str(),
            "resolution" | "query" | "ranking"
        ));
        assert!(!c.expected.is_empty());
    }
    for component in ["resolution", "query", "ranking"] {
        for split in ["calibration", "heldout"] {
            assert!(
                cases
                    .iter()
                    .any(|c| c.component == component && c.split == split)
            );
        }
    }
    println!(
        "Validated {} labeled cases with explicit disjoint case IDs and splits",
        cases.len()
    );
}
fn write(path: &Path, v: &Value) {
    let text = if path.file_name().is_some_and(|n| n == "measurements.json") {
        format!(
            "{{\"model\":{},\"records\":[\n{}\n]}}\n",
            v["model"],
            v["records"]
                .as_array()
                .unwrap()
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join(",\n")
        )
    } else {
        serde_json::to_string_pretty(v).unwrap() + "\n"
    };
    std::fs::write(path, text).unwrap();
}

fn decode_packets(r: &Value) -> Vec<Packet> {
    r["calls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|call| {
            let request = &call["request"];
            let questions = request["questions"]
                .as_array()
                .unwrap()
                .iter()
                .map(|q| {
                    let v = &q["question"];
                    let instructions = v["instructions"].as_str().unwrap().into();
                    let question = match v["type"].as_str().unwrap() {
                        "choice" => JevQuestion::Choice {
                            instructions,
                            criteria: serde_json::from_value(v["criteria"].clone()).unwrap(),
                        },
                        "score" => JevQuestion::Score {
                            instructions,
                            criteria: serde_json::from_value(v["criteria"].clone()).unwrap(),
                        },
                        "noul" => JevQuestion::Noul {
                            instructions,
                            criteria: if v["criteria"].is_null() {
                                None
                            } else {
                                Some(jev_client::YesNoMeanings {
                                    yes: v["criteria"]["yes"].as_str().unwrap().into(),
                                    no: v["criteria"]["no"].as_str().unwrap().into(),
                                })
                            },
                        },
                        _ => unreachable!(),
                    };
                    (q["id"].as_str().unwrap().into(), question)
                })
                .collect();
            let outcome = if call["outcome"]["status"] == "unavailable" {
                JevOutcome::Unavailable
            } else {
                JevOutcome::Answers(
                    call["outcome"]["answers"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|a| {
                            let v = &a["answer"];
                            let answer = match v["type"].as_str().unwrap() {
                                "choice" => JevAnswer::Choice(
                                    v["selected"].as_str().unwrap().into(),
                                    serde_json::from_value(v["probabilities"].clone()).unwrap(),
                                    v["confidence"].as_f64().unwrap(),
                                ),
                                "score" => JevAnswer::Score(
                                    v["score"].as_f64().unwrap(),
                                    serde_json::from_value(v["probabilities"].clone()).unwrap(),
                                    v["confidence"].as_f64().unwrap(),
                                ),
                                "noul" => JevAnswer::Noul(v["probability"].as_f64().unwrap()),
                                _ => unreachable!(),
                            };
                            (a["id"].as_str().unwrap().into(), answer)
                        })
                        .collect(),
                )
            };
            Packet {
                request: JevRequest {
                    state: request["state"].clone(),
                    questions,
                },
                evaluation: outcome.into(),
            }
        })
        .collect()
}

fn equivalent(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() < 1e-12
        }
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| equivalent(a, b))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(k, v)| b.get(k).is_some_and(|other| equivalent(v, other)))
        }
        _ => a == b,
    }
}
fn archive(dir: &Path) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let target = dir
        .join("archive")
        .join(format!("{stamp}-{}", std::process::id()));
    std::fs::create_dir_all(&target).unwrap();
    for name in [
        "measurements.json",
        "summary.json",
        "calibration.json",
        "policy.json",
        "cases.json",
        "run-manifest.json",
        "initial-abort.txt",
    ] {
        let source = dir.join(name);
        if source.exists() {
            std::fs::copy(source, target.join(name)).unwrap();
        }
    }
    println!(
        "Archived current artifacts under docs/evaluation/archive/{}",
        target.file_name().unwrap().to_string_lossy()
    );
}
// Replay the measured source snapshot and its former import policy, exclusively
// in this offline experiment binary. Production imports always reject uncertain
// repeat-signing notes. These historical outcomes are not current graph claims.
fn recorded_reports(root: &std::path::Path) -> (TeammateGraph, report_data::ReportMetadata) {
    use sha2::{Digest, Sha256};
    let mut sources = Vec::new();
    for (path, hash) in [
        (
            "docs/reports/t3/player-universe.csv",
            "81031c7e3cf95845060119a3347d3cf37ec60c6a24b0ced9978fb7df2e7e651d",
        ),
        (
            "docs/reports/t4/tenures.csv",
            "5dbff2a790e87941b6bb9cd9c05c6f91414ae0f47ae1f2992d8d3c230a4fc24e",
        ),
    ] {
        let output = std::process::Command::new("git")
            .current_dir(root)
            .args([
                "show",
                &format!("b107788fccd0c789f4bcb06429d317ee9a390d36:{path}"),
            ])
            .output()
            .expect("recorded Git snapshot");
        assert!(
            output.status.success(),
            "Recorded source commit must be available"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&output.stdout)),
            hash,
            "Immutable measured source hash"
        );

        if path.contains("t4/") {
            let mut reader = csv::Reader::from_reader(output.stdout.as_slice());
            let header = reader.headers().unwrap().clone();
            let notes = header
                .iter()
                .position(|v| v == "unresolved_reasons")
                .unwrap();
            let mut writer = csv::Writer::from_writer(Vec::new());
            writer.write_record(&header).unwrap();
            for row in reader.records() {
                let row = row.unwrap();
                let fields: Vec<String> = row
                    .iter()
                    .enumerate()
                    .map(|(i, text)| {
                        if i == notes {
                            text.split(';')
                                .filter(|note| note.trim() != "repeat-signing-continues-open-stint")
                                .collect::<Vec<_>>()
                                .join(";")
                        } else {
                            text.into()
                        }
                    })
                    .collect();
                writer.write_record(fields).unwrap();
            }
            sources.push(writer.into_inner().unwrap());
        } else {
            sources.push(output.stdout);
        }
    }
    eprintln!(
        "Offline historical replay: hash-verified b107788 snapshot; superseded repeat-signing continuity policy reproduced only for recorded measurement comparability."
    );
    // The transformed historical bytes never become importable disk artifacts.
    // Production and this binary share the strict reader, without a policy flag.
    report_data::load_from_readers(sources[0].as_slice(), sources[1].as_slice()).unwrap()
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join("docs/evaluation");
    let cases: Vec<Case> =
        serde_json::from_slice(&std::fs::read(dir.join("cases.json")).unwrap()).unwrap();
    validate(&cases);
    let arg = std::env::args().nth(1).unwrap_or_default();
    if arg == "--archive" {
        archive(&dir);
        return;
    }
    if arg == "--validate" {
        return;
    }
    assert!(
        arg == "--live" || arg == "--replay",
        "Usage: semantic-eval --validate | --replay | --archive | --live [prior-measurements.json] (live makes bounded paid provider calls)"
    );
    let key = if arg == "--replay" {
        "offline-replay".into()
    } else {
        std::env::var("OPENROUTER_API_KEY").expect("OPENROUTER_API_KEY required")
    };
    assert!(!key.trim().is_empty());
    let (graph, reports) = if arg == "--replay" {
        recorded_reports(&root)
    } else {
        report_data::load(&root.join("docs/reports")).unwrap()
    };
    let catalog = PlayerCatalog::from_graph(&graph, Some(&reports));
    let (rank_graph, rank_catalog) = ranking_fixture();
    let ctx = Context {
        graph,
        catalog,
        reports,
        rank_graph,
        rank_catalog,
    };
    if arg == "--replay" {
        let policy: Policy =
            serde_json::from_slice(&std::fs::read(dir.join("policy.json")).unwrap()).unwrap();
        let v: Value =
            serde_json::from_slice(&std::fs::read(dir.join("measurements.json")).unwrap()).unwrap();
        let mut n = 0;
        for r in v["records"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["policy"] == json!(policy))
        {
            let c: Case = serde_json::from_value(r["case"].clone()).unwrap();
            let (replayed, _) = run(
                &ctx,
                &c,
                policy,
                r["variant"].as_str().unwrap(),
                &key,
                Some(decode_packets(r)),
            );
            assert!(
                equivalent(&replayed["result"], &r["result"]),
                "recorded/runtime outcome differs for {} {}",
                c.id,
                r["variant"]
            );
            n += 1;
        }
        assert_eq!(n, cases.len() * 3);
        println!(
            "Replayed {n} frozen case-variant outcomes through current runtime services without network calls"
        );
        return;
    }
    archive(&dir);
    let permissive = Policy {
        resolution: 0.,
        existence: 0.5,
        query: 0.,
        ranking: 0.,
        margin: 0.,
    };
    let mut captured = vec![];
    let reuse = std::env::args().nth(2);
    let mut raw = if let Some(path) = reuse {
        let v: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        v["records"].as_array().unwrap().clone()
    } else {
        vec![]
    };
    for c in cases.iter().filter(|c| c.split == "calibration") {
        if let Some(r) = raw
            .iter()
            .find(|r| r["case"]["id"] == c.id && r["policy"]["query"] == 0.)
        {
            captured.push((c, decode_packets(r)));
            continue;
        }
        let (r, p) = run(&ctx, c, permissive, "baseline", &key, None);
        println!("calibration {}: {}", c.id, prediction(c, &r));
        raw.push(r);
        write(
            &dir.join("measurements.json"),
            &json!({"model":jev_client::JEVC_MODEL,"records":raw}),
        );
        captured.push((c, p));
    }
    let mut policy = Policy {
        resolution: 0.9,
        existence: 0.9,
        query: 0.9,
        ranking: 0.5,
        margin: 0.25,
    };
    let mut sweep = vec![];
    // Safety-first predeclared objective: minimize false automatic decisions, then
    // maximize exact labeled correctness, then prefer the higher threshold in a tie.
    for component in ["resolution", "query", "ranking"] {
        let mut best = None;
        let thresholds = if component == "ranking" {
            vec![0., 0.25, 0.5, 0.75, 0.9]
        } else {
            vec![0.5, 0.7, 0.9, 0.95]
        };
        for t in thresholds {
            let existences = if component == "resolution" {
                vec![0.5, 0.7, 0.9, 0.95]
            } else {
                vec![policy.existence]
            };
            for existence in existences {
                let mut candidate = policy;
                match component {
                    "resolution" => {
                        candidate.resolution = t;
                        candidate.existence = existence
                    }
                    "query" => candidate.query = t,
                    "ranking" => candidate.ranking = t,
                    _ => unreachable!(),
                };
                let (mut correct, mut wrong, mut abstain, mut missing_replay) = (0, 0, 0, 0);
                for (c, p) in captured.iter().filter(|(c, _)| c.component == component) {
                    let (r, replayed) = run(&ctx, c, candidate, "baseline", &key, Some(p.clone()));
                    // All replay calls must have exactly matching captured request meaning.
                    missing_replay += usize::from(
                        replayed.len() > p.len()
                            || replayed.iter().zip(p).any(|(a, b)| {
                                request_value(&a.request) != request_value(&b.request)
                            }),
                    );
                    let pred = prediction(c, &r);
                    correct += usize::from(pred == c.expected);
                    wrong += usize::from(false_match(c, &pred));
                    abstain += usize::from(matches!(
                        pred.as_str(),
                        "clarification" | "unavailable" | "abstain"
                    ));
                }
                assert_eq!(
                    missing_replay, 0,
                    "uncaptured calibration branch; collect actual runtime observations"
                );
                sweep.push(json!({"component":component,"threshold":t,"existence":existence,"correct":correct,"false_automatic":wrong,"abstain":abstain,"missing_replay":missing_replay}));
                let score = (-(wrong as i32), correct, t, existence);
                if best.as_ref().is_none_or(|(old, _)| score > *old) {
                    best = Some((score, candidate));
                }
            }
        }
        policy = best.unwrap().1;
    }
    write(&dir.join("policy.json"), &json!(policy));
    write(
        &dir.join("calibration.json"),
        &json!({"objective":"minimize false automatic; maximize exact label correctness; highest threshold tie","sweep":sweep,"selected":policy}),
    );
    for c in &cases {
        for variant in ["baseline", "wording", "order"] {
            if raw.iter().any(|r| {
                r["case"]["id"] == c.id && r["variant"] == variant && r["policy"] == json!(policy)
            }) {
                continue;
            }
            let (r, _) = run(&ctx, c, policy, variant, &key, None);
            println!("{} {} {}: {}", c.split, c.id, variant, prediction(c, &r));
            raw.push(r);
            write(
                &dir.join("measurements.json"),
                &json!({"model":jev_client::JEVC_MODEL,"records":raw}),
            );
        }
    }
    let mut summary = vec![];
    for split in ["calibration", "heldout"] {
        for component in ["resolution", "query", "ranking"] {
            for variant in ["baseline", "wording", "order"] {
                let records: Vec<_> = raw
                    .iter()
                    .filter(|r| {
                        r["case"]["split"] == split
                            && r["case"]["component"] == component
                            && r["variant"] == variant
                            && r["policy"] == json!(policy)
                    })
                    .collect();
                let mut correct = 0;
                let mut wrong = 0;
                let mut abstain = 0;
                let mut latency = vec![];
                let mut input = Some(0u64);
                let mut output = Some(0u64);
                let mut cost = Some(0.);
                let mut calls = 0;
                let mut unavailable = 0;
                for r in &records {
                    let c: Case = serde_json::from_value(r["case"].clone()).unwrap();
                    let pred = prediction(&c, r);
                    correct += usize::from(pred == c.expected);
                    wrong += usize::from(false_match(&c, &pred));
                    abstain += usize::from(matches!(
                        pred.as_str(),
                        "clarification" | "unavailable" | "abstain"
                    ));
                    latency.push(r["elapsed_ms"].as_f64().unwrap());
                    for call in r["calls"].as_array().unwrap() {
                        calls += 1;
                        unavailable += usize::from(call["outcome"]["status"] == "unavailable");
                        input = input
                            .zip(call["metadata"]["input_tokens"].as_u64())
                            .map(|(a, b)| a + b);
                        output = output
                            .zip(call["metadata"]["output_tokens"].as_u64())
                            .map(|(a, b)| a + b);
                        cost = cost
                            .zip(call["metadata"]["cost_usd"].as_f64())
                            .map(|(a, b)| a + b);
                    }
                }
                latency.sort_by(f64::total_cmp);
                summary.push(json!({"split":split,"component":component,"variant":variant,"n":records.len(),"correct":correct,"false_automatic":wrong,"abstain":abstain,"provider_calls":calls,"unavailable_calls":unavailable,"input_tokens":input,"output_tokens":output,"cost_usd":cost,"median_ms":latency[latency.len()/2],"max_ms":latency.last()}));
            }
        }
    }
    let mut baseline = vec![];
    for c in cases.iter().filter(|c| c.component == "resolution") {
        let start = Instant::now();
        let s = ctx.catalog.lexical_shortlist(&c.text, 20).unwrap();
        let pred = s
            .candidates
            .first()
            .map(|c| c.player.id.clone())
            .unwrap_or("no_match".into());
        baseline.push(json!({"case_id":c.id,"split":c.split,"prediction":pred,"correct":pred==c.expected,"false_match":false_match(c,&pred),"elapsed_ms":start.elapsed().as_secs_f64()*1000.,"status":s.status,"matches_total":s.matches_total}));
    }
    let mut sensitivity = vec![];
    for c in &cases {
        let predictions: [String; 3] = ["baseline", "wording", "order"].map(|v| {
            prediction(
                c,
                raw.iter()
                    .rev()
                    .find(|r| r["case"]["id"] == c.id && r["variant"] == v)
                    .unwrap(),
            )
        });
        sensitivity.push(json!({"case_id":c.id,"split":c.split,"component":c.component,"baseline":predictions[0],"wording":predictions[1],"order":predictions[2],"wording_changed":predictions[0]!=predictions[1],"order_changed":predictions[0]!=predictions[2]}));
    }
    write(
        &dir.join("summary.json"),
        &json!({"policy":policy,"groups":summary,"lexical_top1_baseline":baseline,"sensitivity":sensitivity}),
    );
}

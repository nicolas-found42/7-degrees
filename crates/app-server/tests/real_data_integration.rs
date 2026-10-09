//! Approved data-to-graph HTTP seam over synthetic canonical-import records.
use axum::{Router, body::Body, http::Request};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

async fn get(app: Router, path: &str) -> Value {
    let response = app
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}
fn snapshot() -> std::path::PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "seven-degrees-import-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("t3")).unwrap();
    std::fs::create_dir_all(root.join("t4")).unwrap();
    std::fs::write(root.join("t3/player-universe.csv"), "bbr_player_id,display_name,first_season,last_season,aba_only,s1_display_name\na,Player Alpha,2000,2002,N,Alpha Alternative\nb,Player Beta,2000,2002,N,\nc,Player Gamma,2000,2002,N,\nd,Player Delta,2000,2002,N,\nx,ABA Only,2000,2002,Y,\n").unwrap();
    std::fs::write(root.join("t4/tenures.csv"), "bbr_player_id,display_name,season,lg,canonical_franchise,membership_source,evidence_class,start_day,end_day,start_anchored,end_anchored,unresolved_reasons,arrival_days,departure_days\na,Player Alpha,2000,NBA,RED,S2,directly-evidenced,1,5,1,1,,1,5\nb,Player Beta,2000,NBA,RED,S2,directly-evidenced,2,4,1,1,,2,4\nb,Player Beta,2001,NBA,BLUE,S2,directly-evidenced,10,13,1,1,,10,13\nc,Player Gamma,2001,NBA,BLUE,S2,directly-evidenced,11,12,1,1,,11,12\nd,Player Delta,2000,NBA,RED,S2,inferred,1,5,0,0,,,\nx,ABA Only,2000,ABA,RED,S2,directly-evidenced,1,5,1,1,,1,5\n").unwrap();
    root
}
#[tokio::test]
async fn canonical_import_returns_real_paths_and_record_provenance() {
    let app = app_server::app_with_report_data(snapshot(), app_server::JevHandle::unconfigured())
        .unwrap();
    let summary = get(app.clone(), "/api/fixture").await;
    assert_eq!(summary["players"].as_array().unwrap().len(), 4);
    let chain = get(app.clone(), "/api/connection?from=a&to=c").await;
    assert_eq!(chain["path"], json!(["a", "b", "c"]));
    assert_eq!(chain["degree"], 2);
    let edges = get(app, "/api/edges/a").await;
    assert_eq!(edges["edges"].as_array().unwrap().len(), 1);
    assert_eq!(
        edges["edges"][0]["evidence"][0]["records"][0]["record"],
        "t4/tenures.csv:2"
    );
}

#[tokio::test]
async fn season_brackets_are_uncertainty_and_never_teammate_facts() {
    let app = app_server::app_with_report_data(snapshot(), app_server::JevHandle::unconfigured())
        .unwrap();
    let chain = get(app.clone(), "/api/connection?from=a&to=d").await;
    assert_eq!(chain["result"], "disconnected");
    assert_eq!(chain["certainty"], "unresolved_coverage");
    let coverage = get(app.clone(), "/api/coverage").await;
    assert_eq!(coverage["certified_tenures"], 4);
    assert_eq!(coverage["excluded_tenures"]["inferred"], 1);
    assert_eq!(coverage["players_without_certified_tenure"], 1);
    assert_eq!(coverage["complete"], false);
    let players = get(app, "/api/players").await;
    assert_eq!(
        players["players"][0]["aliases"],
        json!(["Alpha Alternative"])
    );
    assert_eq!(players["players"][0]["teams"], json!(["RED"]));
}

#[tokio::test]
async fn repeated_source_rows_never_create_a_self_teammate_or_double_overlap() {
    use std::io::Write;
    let root = snapshot();
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(root.join("t4/tenures.csv"))
        .unwrap();
    writeln!(
        file,
        "a,Player Alpha,2000,NBA,RED,S2,directly-evidenced,1,5,1,1,,1,5"
    )
    .unwrap();
    let app =
        app_server::app_with_report_data(root, app_server::JevHandle::unconfigured()).unwrap();
    let edges = get(app, "/api/edges/a").await;
    assert_eq!(edges["edges"].as_array().unwrap().len(), 1);
    assert_eq!(edges["edges"][0]["overlap_days"], 2);
}

#[tokio::test]
async fn alternative_shortest_chains_are_deterministic_and_paginated() {
    use std::io::Write;
    let root = snapshot();
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(root.join("t4/tenures.csv"))
        .unwrap();
    writeln!(
        file,
        "d,Player Delta,2000,NBA,RED,S2,directly-evidenced,3,5,1,1,,3,5"
    )
    .unwrap();
    writeln!(
        file,
        "d,Player Delta,2001,NBA,BLUE,S2,directly-evidenced,10,12,1,1,,10,12"
    )
    .unwrap();
    let app =
        app_server::app_with_report_data(root, app_server::JevHandle::unconfigured()).unwrap();
    let first = get(app.clone(), "/api/paths?from=a&to=c&limit=1").await;
    assert_eq!(first["paths"].as_array().unwrap().len(), 1);
    assert_eq!(first["paths"][0]["path"], json!(["a", "b", "c"]));
    assert_eq!(first["total"], 2);
    assert_eq!(first["next_offset"], 1);
    let second = get(app, "/api/paths?from=a&to=c&limit=1&offset=1").await;
    assert_eq!(second["paths"][0]["path"], json!(["a", "d", "c"]));
    assert_eq!(second["next_offset"], Value::Null);
}

#[tokio::test]
async fn coverage_gaps_remain_inspectable_for_the_affected_player() {
    let app = app_server::app_with_report_data(snapshot(), app_server::JevHandle::unconfigured())
        .unwrap();
    let gaps = get(app, "/api/coverage/d").await;
    assert_eq!(gaps["records"][0]["record"], "t4/tenures.csv:6");
    assert_eq!(gaps["records"][0]["evidence_class"], "inferred");
    assert_eq!(gaps["records"][0]["start_day"], 1);
    assert_eq!(gaps["records"][0]["team"], "RED");
}

/// Each neighboring layer is a complete bipartite graph. The independently
/// known number of shortest chains is two choices per layer: 2^layers.
fn binary_layers(layers_count: usize) -> std::path::PathBuf {
    let root = snapshot();
    let mut players = String::from(
        "bbr_player_id,display_name,first_season,last_season,aba_only,s1_display_name\n",
    );
    players.push_str("s,Start,2000,2000,N,\ng,Goal,2000,2000,N,\n");
    let mut layers = vec![vec!["s".to_string()]];
    for layer in 0..layers_count {
        let ids = vec![format!("l{layer:02}a"), format!("l{layer:02}b")];
        for id in &ids {
            players.push_str(&format!("{id},{id},2000,2000,N,\n"));
        }
        layers.push(ids);
    }
    layers.push(vec!["g".to_string()]);
    let mut rows = String::from(
        "bbr_player_id,season,lg,canonical_franchise,membership_source,evidence_class,start_day,end_day,start_anchored,end_anchored,unresolved_reasons,arrival_days,departure_days\n",
    );
    let mut edge = 0;
    for pair in layers.windows(2) {
        for a in &pair[0] {
            for b in &pair[1] {
                let team = format!("edge{edge}");
                edge += 1;
                for player in [a, b] {
                    rows.push_str(&format!(
                        "{player},2000,NBA,{team},S2,directly-evidenced,1,2,1,1,,1,2\n"
                    ));
                }
            }
        }
    }
    std::fs::write(root.join("t3/player-universe.csv"), players).unwrap();
    std::fs::write(root.join("t4/tenures.csv"), rows).unwrap();
    root
}

#[tokio::test]
async fn the_final_shortest_chain_survives_the_u64_boundary() {
    let app =
        app_server::app_with_report_data(binary_layers(64), app_server::JevHandle::unconfigured())
            .unwrap();
    let before = get(
        app.clone(),
        "/api/paths?from=s&to=g&offset=18446744073709551614&limit=1",
    )
    .await;
    assert_eq!(before["paths"].as_array().unwrap().len(), 1);
    assert_eq!(before["next_offset"], json!(u64::MAX));
    let last = get(
        app,
        "/api/paths?from=s&to=g&offset=18446744073709551615&limit=1",
    )
    .await;
    assert_eq!(last["paths"].as_array().unwrap().len(), 1);
    assert_eq!(last["next_offset"], Value::Null);
    let path = last["paths"][0]["path"].as_array().unwrap();
    assert_eq!(path[1], "l00b");
    assert_eq!(path[64], "l63b");
}

#[tokio::test]
async fn continuation_cursors_reach_shortest_chains_beyond_fixed_integer_limits() {
    // 2^129 paths deliberately exceeds both u64 and u128. Exact decimal
    // expectations are independent constants, not recomputed by the test.
    let app =
        app_server::app_with_report_data(binary_layers(129), app_server::JevHandle::unconfigured())
            .unwrap();
    let page = get(
        app.clone(),
        "/api/paths?from=s&to=g&offset=18446744073709551615&limit=1",
    )
    .await;
    assert_eq!(
        page["total_exact"],
        "680564733841876926926749214863536422912"
    );
    assert_eq!(page["next_cursor"], "v1:18446744073709551616");
    let resumed = get(
        app.clone(),
        "/api/paths?from=s&to=g&cursor=v1:18446744073709551616&limit=1",
    )
    .await;
    assert_eq!(resumed["paths"].as_array().unwrap().len(), 1);
    assert_ne!(resumed["paths"][0]["path"], page["paths"][0]["path"]);
    assert_eq!(resumed["next_cursor"], "v1:18446744073709551617");
    let penultimate = get(
        app.clone(),
        "/api/paths?from=s&to=g&cursor=v1:680564733841876926926749214863536422910&limit=1",
    )
    .await;
    assert_eq!(
        penultimate["next_cursor"],
        "v1:680564733841876926926749214863536422911"
    );
    let last = get(
        app,
        "/api/paths?from=s&to=g&cursor=v1:680564733841876926926749214863536422911&limit=1",
    )
    .await;
    assert_eq!(last["paths"].as_array().unwrap().len(), 1);
    assert_eq!(last["next_cursor"], Value::Null);
    assert_eq!(last["paths"][0]["path"][1], "l00b");
    assert_eq!(last["paths"][0]["path"][129], "l128b");
}

#[tokio::test]
async fn legacy_repeat_signing_continuity_note_blocks_a_runtime_edge() {
    let root = snapshot();
    let path = root.join("t4/tenures.csv");
    let rows = std::fs::read_to_string(&path).unwrap().replace(
        ",1,5,1,1,,1,5",
        ",1,5,1,1,repeat-signing-continues-open-stint,1,5",
    );
    std::fs::write(path, rows).unwrap();
    let app =
        app_server::app_with_report_data(root, app_server::JevHandle::unconfigured()).unwrap();
    assert_eq!(
        get(app.clone(), "/api/connection?from=a&to=b").await["result"],
        "disconnected"
    );
    assert_eq!(get(app, "/api/coverage").await["certified_tenures"], 3);
}

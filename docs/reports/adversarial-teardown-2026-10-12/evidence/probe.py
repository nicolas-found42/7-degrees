#!/usr/bin/env python3
"""Bounded local HTTP consumer; standard library only, no credentials or writes to app."""
import argparse
import collections
import hashlib
import html
import json
import random
import re
import time
import urllib.error
import urllib.request
from pathlib import Path
from urllib.parse import urlencode


def run(base, expect_fixes):
    receipts = []

    def get(path, status=200):
        started = time.perf_counter()
        try:
            with urllib.request.urlopen(base + path, timeout=45) as response:
                body, actual = response.read(), response.status
        except urllib.error.HTTPError as error:
            body, actual = error.read(), error.code
        receipts.append({
            "path": path, "status": actual,
            "ms": round((time.perf_counter() - started) * 1000, 2),
            "bytes": len(body), "sha256": hashlib.sha256(body).hexdigest(),
        })
        assert actual == status, (path, actual, body[:300])
        return body.decode()

    def api(path, status=200):
        return json.loads(get(path, status))

    positives = {
        "Quincy Acy": {"acyqu01"}, "Qunicy Acy": {"acyqu01"},
        "Dee Brown": {"brownde01", "brownde03"},
        "José Calderón": {"caldejo01"}, "Jose Calderon": {"caldejo01"},
        "Shaq": {"onealsh01"},
    }
    retrieved = relevant = expected = 0
    for query, labels in positives.items():
        result = api("/api/search?" + urlencode({"q": query}))
        found = {c["player"]["id"] for c in result["candidates"]}
        assert found == labels, (query, found)
        retrieved += len(found)
        relevant += len(found & labels)
        expected += len(labels)
    for query, status in [("intergalactic banana", "no_match"), ("...", "empty_query")]:
        result = api("/api/search?" + urlencode({"q": query}))
        assert result["status"] == status and result["candidates"] == []
    api("/api/search?" + urlencode({"q": "x" * 161}), 400)
    semantic = api("/api/semantic-status")
    assert semantic == {"available": False, "reason": "unconfigured"}
    assert api("/api/query?" + urlencode({"q": "Connect Quincy Acy to Andrew Bogut"}))["status"] == "unavailable"
    direct = api("/api/connection?from=acyqu01&to=bogutan01")
    assert direct["degree"] == 1 and direct["links"][0]["overlap_days"] == 121
    self_chain = api("/api/connection?from=acyqu01&to=acyqu01")
    assert self_chain["degree"] == 0 and self_chain["links"] == []
    evidence_page = get("/edge?from=acyqu01&to=bogutan01")
    for text in ["2016-07-20", "2016-11-18", "121", "Source evidence is incomplete"]:
        assert text in evidence_page
    hostile = get("/search?" + urlencode({"q": '<script>alert(1)</script>'}))
    assert "&lt;script&gt;" in hostile and "<script>" not in hostile
    for path, status in [
        ("/api/paths?from=acyqu01&to=bogutan01&cursor=bad", 400),
        ("/api/paths?from=acyqu01&to=bogutan01&limit=0", 400),
        ("/api/paths?from=acyqu01&to=bogutan01&cursor=v1:0&offset=0", 400),
        ("/api/neighborhood?player=acyqu01&depth=3", 400),
        ("/api/connection?from=unknown&to=bogutan01", 404),
        ("/api/edges?limit=5001", 400),
    ]:
        get(path, status)
    full = api("/api/paths?from=maravpe01&to=abdulka01&limit=500")
    small, cursor = [], "v1:0"
    for _ in range(500):
        page = api("/api/paths?" + urlencode({
            "from": "maravpe01", "to": "abdulka01", "limit": 1, "cursor": cursor,
        }))
        small.extend(page["paths"])
        cursor = page["next_cursor"]
        if cursor is None:
            break
    assert cursor is None and small == full["paths"]
    assert len(small) == int(full["total_exact"]) == 12
    assert all(p["degree"] == 2 for p in small)

    network = api("/api/network")
    nodes = {n["id"]: n for n in network["nodes"]}
    adjacency = {i: set() for i in nodes}
    edges = set()
    for link in network["links"]:
        a, b = link["from"], link["to"]
        assert a != b and a in nodes and b in nodes
        pair = tuple(sorted([a, b]))
        assert pair not in edges
        edges.add(pair)
        adjacency[a].add(b)
        adjacency[b].add(a)
    assert len(nodes) == 5106 and len(edges) == 101395
    isolates = [i for i in nodes if not adjacency[i]]
    assert len(isolates) == 41
    rng = random.Random(20261012)
    pairs = [("acyqu01", "bogutan01"), ("maravpe01", "abdulka01"),
             ("brownde01", "brownde03"), ("acyqu01", isolates[0])]
    pairs += [tuple(rng.sample(sorted(nodes), 2)) for _ in range(24)]
    distances_checked = []
    for start, goal in pairs:
        distances, queue = {start: 0}, collections.deque([start])
        while queue:
            node = queue.popleft()
            for neighbor in adjacency[node]:
                if neighbor not in distances:
                    distances[neighbor] = distances[node] + 1
                    queue.append(neighbor)
        result = api("/api/connection?" + urlencode({"from": start, "to": goal}))
        assert result.get("degree") == distances.get(goal)
        if goal in distances:
            path = result["path"]
            assert path[0] == start and path[-1] == goal
            assert len(path) == distances[goal] + 1
            assert all(tuple(sorted(p)) in edges for p in zip(path, path[1:]))
        else:
            assert result["certainty"] == "unresolved_coverage"
        distances_checked.append({"from": start, "to": goal, "degree": distances.get(goal)})

    seen, offset, pages = set(), 0, []
    proof_counts = collections.Counter()
    proof_samples = {}
    for _ in range(22):
        page = api(f"/api/edges?limit=5000&offset={offset}")
        assert page["total_edges"] == len(edges)
        assert page["offset"] == offset
        for edge in page["edges"]:
            pair = tuple(sorted([edge["a"], edge["b"]]))
            assert pair in edges and pair not in seen
            seen.add(pair)
            assert edge["evidence"]
            for proof in edge["evidence"]:
                if proof["appearance_proof"]:
                    kind = "appearance"
                    p = proof["appearance_proof"]
                    assert p["a_games"] + p["b_games"] - p["team_games"] == p["minimum_shared_games"] > 0
                    assert len(p["player_records"]) == 2 and p["team_record"]
                    assert proof["overlap_days"] is None and proof["overlaps"] == []
                elif proof["game_witness"]:
                    kind = "game_witness"
                    p = proof["game_witness"]
                    assert p["game_id"] and p["date"] and len(p["sources"]) == 2
                    assert proof["overlap_days"] is None and proof["overlaps"] == []
                else:
                    kind = "dated"
                    assert proof["overlap_days"] > 0 and proof["overlaps"]
                    for overlap in proof["overlaps"]:
                        assert overlap["start_day"] < overlap["end_day"]
                        assert len(overlap["records"]) == 2
                        assert all(r["evidence_class"] == "directly-evidenced" for r in overlap["records"])
                proof_counts[kind] += 1
                proof_samples.setdefault(kind, {"pair": pair, "proof": proof})
        pages.append({"offset": offset, "count": len(page["edges"]), "next_offset": page["next_offset"]})
        if page["next_offset"] is None:
            break
        assert page["next_offset"] == offset + len(page["edges"])
        offset = page["next_offset"]
    assert seen == edges and pages[-1]["next_offset"] is None

    markup = get("/chain?from=brownde03")
    from_select = markup.split('<select id="from"')[1].split('</select>')[0]
    options = [(html.unescape(i), html.unescape(label)) for i, label in
               re.findall(r'<option value="([^"]+)"[^>]*>(.*?)</option>', from_select)]
    assert len(options) == len(nodes)
    labels = collections.Counter(label for _, label in options)
    collisions = {label: count for label, count in labels.items() if count > 1}
    result_cards = get("/search?q=Dee%20Brown")
    if expect_fixes:
        assert not collisions
        assert '<option value="brownde03" selected>Dee Brown — 2007–2009 · brownde03</option>' in from_select
        assert dict(options)["brownde01"] == "Dee Brown — 1991–2002 · brownde01"
        assert dict(options)["acyqu01"] == "Quincy Acy"
        for player in ["brownde01", "brownde03"]:
            assert f"Player ID: <code>{player}</code>" in result_cards
            assert f"Player ID: <code>{player}</code>" in get("/players/" + player)
    stats = api("/api/stats")
    assert stats["players"] == len(nodes)
    assert sum(stats["histogram"].values()) + stats["unreachable_pairs"] == len(nodes) * (len(nodes) - 1) // 2
    return {
        "base": base, "expect_fixes": expect_fixes, "precision": [relevant, retrieved],
        "completeness": [relevant, expected], "negative_queries": 2,
        "path_alternatives": len(small), "nodes": len(nodes), "edges": len(edges),
        "isolates": len(isolates), "distances_checked": distances_checked,
        "edge_pages": pages, "proof_counts": dict(proof_counts), "proof_samples": proof_samples,
        "duplicate_labels": collisions, "stats": stats, "http_receipts": receipts,
        "boundary": "Pinned local graph; proof structure/arithmetic checked, historical sources not independently reconstructed.",
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--expect-fixes", action="store_true")
    args = parser.parse_args()
    assert args.base.startswith("http://127.0.0.1:"), "local disposable server only"
    assert not args.output.exists(), "preserve previous evidence"
    started = time.perf_counter()
    result = run(args.base, args.expect_fixes)
    result["elapsed_seconds"] = round(time.perf_counter() - started, 2)
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({k: v for k, v in result.items() if k not in
                      {"http_receipts", "proof_samples", "distances_checked", "edge_pages"}}, indent=2))

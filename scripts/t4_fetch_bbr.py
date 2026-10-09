#!/usr/bin/env python3
"""T4 — cached, throttled fetch + parse of Basketball-Reference transaction pages.

Ticket #6 (*T4: Reconstruct roster tenures and report coverage*).

Fetches the bounded, ordered subset of BRef league transaction pages

    https://www.basketball-reference.com/leagues/<LEAGUE>_<YEAR>_transactions.html
    BAA_1947 .. BAA_1949, NBA_1950 .. NBA_2026          (80 pages)

* cache-first: a page already cached on disk is never re-fetched
* throttled: >= 5.2 s between HTTP requests (spacing measured request-start to
  request-start); research pass observed Cloudflare 1015 throttling on rapid fetches
* one retry per page on 429 / 503 / Cloudflare-1015 bodies / connection errors,
  after a longer 45 s back-off — a second failure is recorded as an *unresolved
  fetch*, never as proof that the historical record does not exist
* every request (and every cache hit) is recorded in a JSONL request ledger with
  UTC timestamp, status, bytes — the ledger is copied into docs/reports/t4/ and
  summarized in the coverage report

Parsing (offline, from cache): each page's dated transaction rows become records
with date text, date precision (precise / fuzzy / unknown — fuzzy rows are NEVER
guessed to a date), event class, franchise abbrs, and player slugs, plus the
page's "Transactions listed are from X to Y" window. Output:
  data/t4/bbr-request-ledger.jsonl      request ledger (append)
  data/t4/fetch-summary.json            pass summary (pages, statuses, retries)
  data/t4/transactions-parsed.pkl       parsed rows (input to t4_reconcile.py)
  data/cache/bbr/<page>.html            raw page cache (gitignored)
  docs/reports/t4/bbr-transactions-parsed.csv   retained parsed-row CSV
  docs/reports/t4/bbr-request-ledger.csv        retained ledger CSV

Run:  python3 scripts/t4_fetch_bbr.py [data_dir] [--no-fetch]
  data_dir defaults to the repo's `data/`, falling back to the sibling main
  checkout `../7-degrees/data` (worktrees have no data/). --no-fetch parses
  from cache only (no network, no waits). Read-only except data/cache/,
  data/t4/ and docs/reports/t4/ writes; source data is never modified.
"""

import csv
import json
import os
import pickle
import re
import sys
import time
import urllib.request
import urllib.error
from datetime import datetime, timezone
from html.parser import HTMLParser

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)

BAA_YEARS = (1947, 1948, 1949)
NBA_YEARS = tuple(range(1950, 2027))          # seasons 1949-50 .. 2025-26
PAGES = [("BAA", y) for y in BAA_YEARS] + [("NBA", y) for y in NBA_YEARS]

THROTTLE_SECONDS = 5.2        # minimum spacing between HTTP request starts
RETRY_BACKOFF_SECONDS = 45.0  # longer back-off before the single retry
REQUEST_TIMEOUT = 60
USER_AGENT = (
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 "
    "(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36"
)

REPORT_DIR = os.path.join(REPO, "docs", "reports")
T4_DIR = os.path.join(REPORT_DIR, "t4")


# ------------------------------------------------------------------- paths --
def resolve_paths(data_dir):
    if data_dir is None:
        data_dir = os.path.join(REPO, "data")
    if not os.path.isdir(data_dir):
        alt = os.path.normpath(os.path.join(REPO, "..", "7-degrees", "data"))
        if os.path.isdir(alt):
            data_dir = alt
    return {
        "data_dir": data_dir,
        "cache_dir": os.path.join(data_dir, "cache", "bbr"),
        "t4_data": os.path.join(data_dir, "t4"),
        "ledger": os.path.join(data_dir, "t4", "bbr-request-ledger.jsonl"),
        "summary": os.path.join(data_dir, "t4", "fetch-summary.json"),
        "parsed_pkl": os.path.join(data_dir, "t4", "transactions-parsed.pkl"),
    }


def page_url(league, year):
    return "https://www.basketball-reference.com/leagues/%s_%d_transactions.html" % (league, year)


def page_cache_path(paths, league, year):
    return os.path.join(paths["cache_dir"], "%s_%d_transactions.html" % (league, year))


# ------------------------------------------------------------------ ledger --
def ledger_append(ledger_path, record):
    os.makedirs(os.path.dirname(ledger_path), exist_ok=True)
    with open(ledger_path, "a", encoding="utf-8") as f:
        f.write(json.dumps(record, sort_keys=True) + "\n")


def ledger_rows(ledger_path):
    rows = []
    if os.path.exists(ledger_path):
        with open(ledger_path, encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if line:
                    rows.append(json.loads(line))
    return rows


# ------------------------------------------------------------------ fetcher --
class FetchOutcome(object):
    __slots__ = ("status", "body", "error")

    def __init__(self, status=None, body=b"", error=None):
        self.status = status      # HTTP status int, or None on transport failure
        self.body = body          # bytes
        self.error = error        # transport error string

    @property
    def ok(self):
        return self.status == 200 and self.body

    def throttled(self):
        """429 / 503 / Cloudflare 1015 rate-limit markers."""
        if self.status in (429, 503):
            return True
        if self.status is not None and self.body:
            try:
                head = self.body[:4000].decode("utf-8", "replace")
            except Exception:
                head = ""
            if "1015" in head and ("Cloudflare" in head or "Error" in head):
                return True
        return False


def http_get(url, timeout=REQUEST_TIMEOUT):
    req = urllib.request.Request(url, headers={
        "User-Agent": USER_AGENT,
        "Accept": "text/html,application/xhtml+xml",
        "Accept-Language": "en-US,en;q=0.9",
    })
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return FetchOutcome(status=resp.getcode(), body=resp.read())
    except urllib.error.HTTPError as e:
        try:
            body = e.read()
        except Exception:
            body = b""
        return FetchOutcome(status=e.code, body=body, error=str(e))
    except Exception as e:  # URLError, timeout, ConnectionReset, ...
        return FetchOutcome(status=None, body=b"", error="%s: %s" % (type(e).__name__, e))


def fetch_page_throttled(paths, url, ledger_path, state):
    """Cache-first fetch: never re-fetch a cached page. One retry on throttle."""
    league_year = url.rsplit("/", 1)[-1].replace("_transactions.html", "")
    league, year_s = league_year.split("_", 1)
    cache = page_cache_path(paths, league, int(year_s))
    if os.path.exists(cache) and os.path.getsize(cache) > 0:
        ledger_append(ledger_path, {
            "ts_utc": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "action": "cache-hit", "url": url, "status": None,
            "bytes": os.path.getsize(cache), "attempt": 0,
            "note": "cached (no request)",
        })
        state["cache_hits"] += 1
        with open(cache, "rb") as f:
            return f.read(), "cache"
    for attempt in (1, 2):
        # hard throttle spacing measured between request STARTS
        now = time.monotonic()
        wait = state["min_spacing"] - (now - state["last_request_start"])
        if wait > 0:
            time.sleep(wait)
        # ledger timestamp = request START so the ledger itself shows the
        # enforced start-to-start spacing (response timestamps would drift by
        # the round-trip time)
        started_utc = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
        state["last_request_start"] = time.monotonic()
        out = http_get(url)
        ok = out.ok
        ledger_append(ledger_path, {
            "ts_utc": started_utc,
            "action": "http-get" if attempt == 1 else "http-get-retry",
            "url": url, "status": out.status,
            "bytes": len(out.body) if out.body else 0,
            "attempt": attempt,
            "error": out.error or "",
            "note": "throttled-response" if out.throttled() else ("ok" if ok else "failed"),
        })
        if ok:
            with open(cache, "wb") as f:
                f.write(out.body)
            state["fetched"] += 1
            state["bytes"] += len(out.body)
            if attempt == 2:
                state["retries_ok"] += 1
            return out.body, "http"
        if attempt == 1 and (out.throttled() or out.status is None or out.error):
            time.sleep(RETRY_BACKOFF_SECONDS)
            state["last_request_start"] = 0.0  # retry waits a full spacing after backoff
            continue
        state["fetch_failures"] += 1
        state["failures"][league_year] = {
            "status": out.status, "error": out.error or "",
        }
        return None, "failed"
    return None, "failed"


# ------------------------------------------------------------------ parser --
MONTHS = {
    "January": 1, "February": 2, "March": 3, "April": 4, "May": 5, "June": 6,
    "July": 7, "August": 8, "September": 9, "October": 10, "November": 11,
    "December": 12,
}

DATE_FULL_RE = re.compile(r"^(January|February|March|April|May|June|July|August|"
                          r"September|October|November|December)\s+(\d{1,2}),\s*(\d{4})$")
DATE_FUZZY_MONTH_RE = re.compile(r"^(January|February|March|April|May|June|July|August|"
                                 r"September|October|November|December)\s+\?+,\s*(\d{4})$")
DATE_FUZZY_YEAR_RE = re.compile(r"^\??\s*(\d{4})$")


def parse_date_text(text):
    """('precise'|'fuzzy'|'unknown', iso_date_or_'') — fuzzy rows stay fuzzy
    (month/year only), never guessed to a day."""
    t = " ".join((text or "").split())
    m = DATE_FULL_RE.match(t)
    if m:
        return "precise", "%04d-%02d-%02d" % (int(m.group(3)), MONTHS[m.group(1)], int(m.group(2)))
    m = DATE_FUZZY_MONTH_RE.match(t)
    if m:
        return "fuzzy", "%04d-%02d" % (int(m.group(2)), MONTHS[m.group(1)])
    m = DATE_FUZZY_YEAR_RE.match(t)
    if m:
        return "fuzzy", m.group(1)
    if t:
        return "unknown", ""
    return "unknown", ""


PLAYER_HREF_RE = re.compile(r"^/players/[a-z0-9]/([a-z0-9]+)\.html$")
COACH_HREF_RE = re.compile(r"^/coaches/[a-z0-9]+\.html$")
TEAM_HREF_RE = re.compile(r"^/teams/([A-Za-z0-9]+)/(\d{4})\.html$")


class TxnListParser(HTMLParser):
    """Extract the transaction list: <span> date + <p> event sentences.

    Walks into the first <ul ...> following the anchor
    <span class="section_anchor" id="transactions_link" ...>, tracking list
    nesting depth so nested tables/lists inside <li> are contained.
    """

    def __init__(self):
        HTMLParser.__init__(self, convert_charrefs=True)
        self.in_txn_area = False
        self.list_depth = 0
        self.in_li = False
        self.date_seen = False
        self.p_depth = 0
        self.rows = []            # one dict per <li>
        self.cur = None           # dict while inside a transaction <li>
        self.buf = []
        self.anchor = None        # dict while inside an <a>
        self.anchor_attrs = None
        self.in_date = False
        self._last_open_seg = None

    def handle_starttag(self, tag, attrs):
        a = dict(attrs)
        if tag == "span" and a.get("id") == "transactions_link":
            self.in_txn_area = True
            return
        if not self.in_txn_area:
            return
        if tag == "ul" and self.cur is None:
            # first list after the anchor = the transaction list; further nested
            # lists sit inside a li (cur is not None) and are just text
            self.list_depth += 1
        elif tag == "ul":
            self.list_depth += 1
        elif tag == "li" and self.list_depth >= 1:
            if self.cur is not None:
                # nested li inside a transaction li — treat as continuing text
                self.buf.append(" | ")
                return
            self.in_li = True
            self.date_seen = False
            self.cur = {"date": "", "segments": [], "raw": ""}
        elif tag == "p":
            if self.cur is not None and self.in_li:
                self.p_depth += 1
                self.buf = []
        elif tag == "span" and self.in_li and not self.date_seen and self.p_depth == 0:
            self.date_seen = True
            self.buf = []
            self.in_date = True
        elif tag == "a" and self.cur is not None and self.in_li and self.p_depth > 0:
            self.anchor_attrs = a
            self.anchor = {"attrs": a, "buf": []}

    def handle_endtag(self, tag):
        if not self.in_txn_area:
            return
        if tag == "ul":
            if self.list_depth > 0:
                self.list_depth -= 1
                if self.list_depth == 0:
                    self.in_txn_area = False
        elif tag == "li":
            if self.in_li and self.list_depth == 1:
                self._finish_row()
                self.in_li = False
                self.cur = None
        elif tag == "p":
            if self.p_depth > 0:
                self._finish_segment()
                self.p_depth -= 1
        elif tag == "span":
            self.in_date = False
        elif tag == "a":
            self._finish_anchor()

    in_date = False

    def _finish_anchor(self):
        if self.anchor is None:
            return
        a = self.anchor_attrs or {}
        href = a.get("href") or ""
        text = "".join(self.anchor["buf"]).strip()
        rec = {"kind": "other", "href": href, "text": text,
               "from_abbr": a.get("data-attr-from") or "",
               "to_abbr": a.get("data-attr-to") or ""}
        # data-attr-from/to mark a franchise anchor even when the source omits
        # the href (real BBR shape: <a data-attr-from="DTF">Detroit Falcons</a>)
        if rec["from_abbr"] or rec["to_abbr"]:
            rec["kind"] = "team"
        m = PLAYER_HREF_RE.match(href)
        if m:
            rec["kind"] = "player"
            rec["slug"] = m.group(1)
        elif COACH_HREF_RE.match(href):
            rec["kind"] = "coach"
        elif rec["kind"] != "team":
            m = TEAM_HREF_RE.match(href)
            if m:
                rec["kind"] = "team"
                rec["abbr"] = m.group(1)
                rec["season"] = int(m.group(2))
        seg = {}
        if self.p_depth > 0:
            seg = self._cur_seg()
            seg["items"].append(("anchor", rec))
        elif self.in_li and self.p_depth == 0 and self.date_seen:
            pass  # date spans are handled via buf into the date text
        self.anchor = None
        self.anchor_attrs = None

    def _cur_seg(self):
        if self.cur is None:
            self.cur = {"date": "", "segments": [], "raw": ""}
        if not self.cur["segments"] or self.cur["segments"][-1] is not self._last_open_seg:
            seg = {"items": [], "text": ""}   # items: ("text", str) | ("anchor", dict)
            self.cur["segments"].append(seg)
            self._last_open_seg = seg
        return self.cur["segments"][-1]

    def handle_data(self, data):
        if not self.in_txn_area or self.cur is None:
            return
        if self.in_date:
            self.cur["date"] += data
        elif self.anchor is not None:
            self.anchor["buf"].append(data)
        elif self.p_depth > 0:
            seg = self._cur_seg()
            if seg["items"] and seg["items"][-1][0] == "text":
                seg["items"][-1] = ("text", seg["items"][-1][1] + data)
            else:
                seg["items"].append(("text", data))
        elif self.in_li:
            self.cur["raw"] += data

    def _finish_segment(self):
        if self.cur is None or not self.cur["segments"]:
            return
        seg = self.cur["segments"][-1]
        # items are RETAINED (needed by movement_legs downstream); only the
        # flattened text is normalized here
        if seg.get("items"):
            seg["text"] = re.sub(r"\s+", " ", " ".join(
                x[1] for x in seg["items"] if x[0] == "text")).strip()
            seg["text"] = re.sub(r"\[\s*\]", "", seg["text"]).strip()
        if self._last_open_seg is seg:
            self._last_open_seg = None

    def _finish_row(self):
        if self.cur is None:
            return
        self.cur["raw"] = re.sub(r"\s+", " ", self.cur["raw"]).strip()
        for seg in self.cur["segments"]:
            for a in seg.get("anchors", []):
                pass
        self.rows.append(self.cur)


EVENT_CLASSES = (
    ("trade", ("traded", "in a three-team trade", "trade that sent")),
    ("sale", ("sold", "sale of", "sold the player rights")),
    ("purchase", ("purchased", "bought")),
    ("claim", ("claimed off waivers", "claimed on waivers", "claim")),
    ("waive", ("waived",)),
    ("release", ("released",)),
    ("sign", ("signed",)),
    ("resign", ("re-signed", "resigned")),
    ("extension", ("signed an extension", "contract extension")),
    ("renounce", ("renounced",)),
    ("dispersal", ("selected", "dispersal")),
    ("coach-hire", ("hired",)),
    ("coach-fire", ("fired",)),
)


def classify_event(text, has_coach_only):
    low = text.lower()
    if has_coach_only:
        if "fired" in low:
            return "coach-fire"
        if "hired" in low:
            return "coach-hire"
        return "coach-other"
    # order matters: "re-signed" before "signed"; "claimed off waivers" before "signed"
    for cls, words in EVENT_CLASSES:
        for w in words:
            if w in low:
                return cls
    if "waiver" in low:
        return "claim"
    return "other"


def movement_legs(items, event_class):
    """(player_slug, name, depart_team_abbr|'', arrive_team_abbr|'') legs.

    `items` is the segment's ordered stream of ("text", str) and
    ("anchor", rec) pairs — text chunks retained so movement attribution is
    anchored in the source's own words, not guessed from anchor order alone.

    Rules (each verified against real BBR row shapes):
    * The prose is split into SENTENCES at '.', ';', and ' and the ' phrase
      boundaries in the text stream. Attr anchors (data-attr-from/to) open and
      close their own phrase; sentences reset.
    * For each player anchor:
      - depart = nearest team anchor with a from-attr BEFORE the player in the
        same sentence; when none (a 'claimed on waivers FROM the X' or
        'selected ... from the X' shape), the nearest from-anchor AFTER.
      - arrive = nearest to-attr anchor AFTER the player in the same sentence,
        stopping at a from-anchor; when none ('The X signed P' destination-
        first), the nearest to-anchor BEFORE.
      - trade FOR-clause: after a ' for ' text chunk inside a trade sentence,
        the sentence's direction REVERSES (the return pieces travel from the
        to-team back to the from-team) — real shape: 'The [from=ATL] traded
        [Gasol] to the [to=VAN] for [Abdur-Rahim] and [Tinsley]' gives
        Gasol ATL->VAN and Abdur-Rahim/Tinsley VAN->ATL.
      - a player with neither side resolvable in its sentence yields an empty
        leg ('', '') — the reconcile stage flags it (no-team-anchor-on-leg);
        it is never guessed from a different sentence.
    """
    legs = []

    # sentence split: accumulate (side, anchor, from_ctx, to_ctx); side tracks
    # 'for'-reversal within the sentence; from_ctx/to_ctx track the sentence's
    # initiator/destination team abbrs.
    # First pass: walk items; close sentence on text containing sentence-enders.
    sentence = []          # items of the current sentence
    sentences = []
    for kind, val in items:
        if kind == "text":
            # split text on ends, keeping trailing part as new carry-over
            parts = re.split(r"(?<=[.;])\s+", val)
            for k, part in enumerate(parts):
                if k > 0 and sentence:
                    sentences.append(sentence)
                    sentence = []
                if part:
                    sentence.append(("text", part))
        else:
            sentence.append((kind, val))
            # ' and the ' begins a new phrase in BBR trade chains only when the
            # next anchor carries a from-attr; handled by attrs, no split here
    if sentence:
        sentences.append(sentence)

    for sen in sentences:
        sen_anchors = [v for k, v in sen if k == "anchor"]
        # sentence-level from/to (the phrase initiator + destination); BBR gives
        # each sentence its own attrs, so these hold for every player in it
        sen_from = next((a["from_abbr"] for a in sen_anchors
                         if a["kind"] == "team" and a.get("from_abbr")), "")
        sen_to = next((a["to_abbr"] for a in sen_anchors
                       if a["kind"] == "team" and a.get("to_abbr")), "")
        # per player:
        for idx, (k, v) in enumerate(sen):
            if k != "anchor" or v["kind"] != "player":
                continue
            # 'for'-clause detection: any text chunk before this player that ends
            # the outgoing side (BBR: 'traded X to the B for Y and Z')
            seen_for = any(k2 == "text" and re.search(r"\bfor\s*$", v2.rstrip())
                           for k2, v2 in sen[:idx])
            # attr anchors around the player within the sentence
            back_from = back_to = fwd_from = fwd_to = ""
            for k2, v2 in reversed(sen[:idx]):
                if k2 != "anchor":
                    continue
                a = v2
                if a["kind"] == "team" and a.get("from_abbr"):
                    back_from = a["from_abbr"]
                    break
                if a["kind"] == "team" and a.get("to_abbr"):
                    back_to = a["to_abbr"]
                    break
            hard_stop = False
            for k2, v2 in sen[idx + 1:]:
                if k2 != "anchor":
                    continue
                a = v2
                if a["kind"] == "team" and a.get("to_abbr") and not hard_stop:
                    fwd_to = a["to_abbr"]
                    break
                if a["kind"] == "team" and a.get("from_abbr"):
                    fwd_from = a["from_abbr"]
                    hard_stop = True
                    break
            if seen_for and (sen_from or sen_to):
                # return piece: travels the sentence's destination -> initiator
                legs.append((v["slug"], v.get("text", ""), sen_to, sen_from))
                continue
            depart = back_from or fwd_from
            arrive = fwd_to or back_to
            legs.append((v["slug"], v.get("text", ""), depart, arrive))
    return legs


def parse_transactions_html(html, league, season):
    parser = TxnListParser()
    try:
        parser.feed(html)
        parser.close()
    except Exception:
        pass
    # page bounding window: "Transactions listed are from <strong>X</strong> to
    # <strong>Y</strong>." — tag-tolerant: strip tags in a window after the prose
    win_i = html.find("Transactions listed are from")
    window_from, window_to = ("", "")
    if win_i >= 0:
        prose = re.sub(r"<[^>]+>", " ", html[win_i:win_i + 400])
        prose = re.sub(r"\s+", " ", prose).strip()
        m = re.match(r"Transactions listed are from\s+(.+?)\s+to\s+([^.]*)\.", prose)
        if m:
            _, wf = parse_date_text(m.group(1))
            _, wt = parse_date_text(m.group(2))
            window_from, window_to = wf, wt
    out = []
    for li_index, row in enumerate(parser.rows):
        precision, iso = parse_date_text(row["date"])
        for seg in row["segments"]:
            text = seg.get("text", "")
            items = seg.get("items")
            anchors = [v for k, v in (items or []) if k == "anchor"]
            if not text and not anchors:
                continue
            has_coach = any(a["kind"] == "coach" for a in anchors)
            players = [a["slug"] for a in anchors if a["kind"] == "player"]
            if has_coach and not players:
                event_class = classify_event(text, True)
            else:
                event_class = classify_event(text, False)
            legs = movement_legs(items or [], event_class)
            out.append({
                "page": "%s_%d" % (league, season),
                "league": league,
                "season": season,
                "li_index": li_index,
                "date_text": row["date"].strip(),
                "date_precision": precision,
                "date_iso": iso,
                "page_window_from": window_from,
                "page_window_to": window_to,
                "event_class": event_class,
                "team_abbrs": ",".join(sorted({a.get("from_abbr") or a.get("to_abbr") or a.get("abbr", "")
                                               for a in anchors if a["kind"] == "team"})),
                "from_abbr": (next((a.get("from_abbr") for a in anchors
                                    if a["kind"] == "team" and a.get("from_abbr")), "")),
                "to_abbr": (next((a.get("to_abbr") for a in anchors
                                  if a["kind"] == "team" and a.get("to_abbr")), "")),
                "player_slugs": ",".join(sorted({a.get("slug", "") for a in anchors
                                                 if a["kind"] == "player" and a.get("slug")})),
                "player_names": ",".join(sorted({a.get("text", "") for a in anchors
                                                 if a["kind"] == "player"})),
                "legs": [{"slug": s, "name": n, "depart": d, "arrive": v} for s, n, d, v in legs],
                "text": text,
            })
        if not row["segments"]:
            out.append({
                "page": "%s_%d" % (league, season), "league": league, "season": season,
                "li_index": li_index, "date_text": row["date"].strip(),
                "date_precision": precision, "date_iso": iso,
                "page_window_from": window_from, "page_window_to": window_to,
                "event_class": "no-segment", "team_abbrs": "", "from_abbr": "", "to_abbr": "",
                "player_slugs": "", "player_names": "", "legs": [], "text": row["raw"],
            })
    return out, window_from, window_to


CSV_COLUMNS = ["page", "league", "season", "li_index", "date_text", "date_precision",
               "date_iso", "page_window_from", "page_window_to", "event_class",
               "team_abbrs", "from_abbr", "to_abbr", "player_slugs", "player_names",
               "legs_json", "text", "row_flags"]


def row_flag(rec):
    flags = []
    if not rec["date_iso"]:
        flags.append("no-date")
    if rec["date_precision"] in ("fuzzy", "unknown"):
        flags.append("fuzzy-or-unknown-date")
    wf, wt = rec["page_window_from"], rec["page_window_to"]
    if wf and wt and rec["date_precision"] == "precise":
        d = rec["date_iso"]
        # lexical compare works on ISO strings of equal width
        if len(d) == 10 and len(wf) == 10 and len(wt) == 10:
            if d < wf or d > wt:
                flags.append("date-outside-page-window")
    if not rec["player_slugs"] and rec["event_class"] not in (
            "coach-hire", "coach-fire", "coach-other", "other", "no-segment"):
        flags.append("no-player-anchor")
    return "|".join(flags)


# --------------------------------------------------------------------- main --
def main(argv):
    args = [a for a in argv[1:]]
    no_fetch = "--no-fetch" in args
    args = [a for a in args if not a.startswith("--")]
    data_dir = args[0] if args else None
    paths = resolve_paths(data_dir)
    os.makedirs(paths["cache_dir"], exist_ok=True)
    os.makedirs(paths["t4_data"], exist_ok=True)
    os.makedirs(T4_DIR, exist_ok=True)
    ledger_path = paths["ledger"]

    # pre-script manual probe (single BAA_1947 curl) recorded honestly once
    if not ledger_rows(ledger_path):
        manual = page_cache_path(paths, "BAA", 1947)
        if os.path.exists(manual):
            ledger_append(ledger_path, {
                "ts_utc": "2026-10-09 (pre-script session, exact minute not recorded)",
                "action": "manual-probe", "url": page_url("BAA", 1947), "status": 200,
                "bytes": os.path.getsize(manual), "attempt": 1,
                "note": "single curl probe preceding this script; cache-seeded",
            })

    state = {"fetched": 0, "cache_hits": 0, "fetch_failures": 0, "bytes": 0,
             "retries_ok": 0, "failures": {}, "last_request_start": 0.0, "min_spacing": THROTTLE_SECONDS}

    t0 = time.time()
    for league, year in PAGES:
        url = page_url(league, year)
        if no_fetch:
            cache = page_cache_path(paths, league, year)
            if os.path.exists(cache) and os.path.getsize(cache) > 0:
                with open(cache, "rb") as f:
                    body = f.read()
            else:
                state["fetch_failures"] += 1
                state["failures"]["%s_%d" % (league, year)] = {"status": None,
                                                               "error": "not-cached (--no-fetch)"}
                continue
        else:
            body, _how = fetch_page_throttled(paths, url, ledger_path, state)
        if body is None:
            continue
        html = body.decode("utf-8", "replace")
        rows, wf, wt = parse_transactions_html(html, league, year)
        state.setdefault("parsed_rows", 0)
        state["parsed_rows"] += len(rows)
        state.setdefault("pages_parsed", []).append(
            {"page": "%s_%d" % (league, year), "rows": len(rows),
             "window_from": wf, "window_to": wt})
    state["wall_seconds"] = round(time.time() - t0, 1)

    # parse every cached page again for the final artifact (incl. failures skipped above)
    all_rows = []
    parse_failures = []
    for league, year in PAGES:
        cache = page_cache_path(paths, league, year)
        if not (os.path.exists(cache) and os.path.getsize(cache) > 0):
            parse_failures.append({"page": "%s_%d" % (league, year), "reason": "not-cached"})
            continue
        with open(cache, "rb") as f:
            html = f.read().decode("utf-8", "replace")
        rows, wf, wt = parse_transactions_html(html, league, year)
        for r in rows:
            r["row_flags"] = row_flag(r)
            r["legs_json"] = json.dumps(r["legs"], sort_keys=True)
            del r["legs"]
            all_rows.append(r)
    # summary
    summary = {
        "generated_utc": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "pages_targeted": len(PAGES),
        "http_fetched": state["fetched"],
        "cache_hits": state["cache_hits"],
        "fetch_failures": state["fetch_failures"],
        "retries_that_then_succeeded": state["retries_ok"],
        "bytes_fetched": state["bytes"],
        "throttle_seconds": THROTTLE_SECONDS,
        "retry_backoff_seconds": RETRY_BACKOFF_SECONDS,
        "wall_seconds": state["wall_seconds"],
        "failures": state["failures"],
        "parsed_rows": len(all_rows),
        "parse_failures": parse_failures,
        "note": ("A recorded fetch failure is an UNRESOLVED FETCH. It is never proof "
                 "that a historical transaction record does not exist."),
    }
    with open(paths["summary"], "w", encoding="utf-8") as f:
        json.dump(summary, f, indent=2, sort_keys=True)
    with open(paths["parsed_pkl"], "wb") as f:
        pickle.dump({"rows": all_rows, "summary": summary}, f)
    # retained CSVs
    with open(os.path.join(T4_DIR, "bbr-transactions-parsed.csv"), "w",
              newline="", encoding="utf-8") as f:
        w = csv.DictWriter(f, fieldnames=CSV_COLUMNS, extrasaction="ignore")
        w.writeheader()
        for r in all_rows:
            w.writerow({k: r.get(k, "") for k in CSV_COLUMNS})
    led = ledger_rows(ledger_path)
    with open(os.path.join(T4_DIR, "bbr-request-ledger.csv"), "w",
              newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        w.writerow(["ts_utc", "action", "url", "status", "bytes", "attempt", "error", "note"])
        for r in led:
            w.writerow([r.get("ts_utc", ""), r.get("action", ""), r.get("url", ""),
                        r.get("status", ""), r.get("bytes", ""), r.get("attempt", ""),
                        r.get("error", ""), r.get("note", "")])
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
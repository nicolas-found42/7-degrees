#!/usr/bin/env python3
"""Canonical franchise seed for T3 cross-source reconciliation (ticket #5).

This module is the curated, review-controlled input for building the
franchise-alias crosswalk. Everything else is derived from the pinned
sources (S2 `Team Abbrev.csv`, S1 `game`/`team_history`/`team`) by
`t3_reconcile.py`, which verifies this seed against both sources and
lists any residual disagreement in the reconciliation report.

Lineage entry shape: (lg, team_name, abbr_bbr, abbr_s1, season_start, season_end)
- Seasons use the S2 convention: `season` = season starting year + 1, i.e.
  season "1947" is 1946-47, season "2026" is 2025-26. S1 game rows convert via
  `s1_season_id_to_season_year` ('21946' -> 1947): the leading digit of an S1
  `season_id` encodes the season-type family and the last four digits are the
  season starting year.
- abbr_bbr is the abbreviation/ident used by S2 (Basketball-Reference scrape);
  abbr_s1 is the S1 (stats.nba.com `game` table) abbreviation when it differs.
  S1 abbreviation changes over time for one franchise (e.g. PHL->PHI for the
  76ers) are modeled as two seed rows.
- Multiple (lg, name) eras share one canonical franchise id exactly when the
  NBA/BAA records treat them as one franchise line. ABA-lineage franchises that
  later joined the NBA (Spurs, Nets, Nuggets, Pacers) are one canonical id
  spanning both leagues; purely-ABA franchises get separate canonical ids (they
  exist so ABA-only players can be bucketed and a wrong NBA merge can never
  happen silently).
- S2 franchise coverage notes, checked against the raw tables:
  - No Baltimore roster gap exists for BAA 1947 (1946-47): `Player Season Info.csv`
    has no BLB rows in season 1947; Baltimore joined the BAA in 1947-48, where S2
    has 18 BLB player rows, and 21 in 1948-49. `Team Abbrev.csv` includes both seasons.
    BLB-1947 in `Draft Pick History.csv` is a draft-team entry, not roster coverage.
  - The BAA 1949-50 Tri-Cities Blackhawks season is labeled NBA by S2 (the NBA's
    first season was 1949-50 = season "1950"; BAA seasons are "1947"-"1949").

Franchises sharing a display name across distinct franchises are split by season:
- "Denver Nuggets": DNN season 1950 (defunct NBA, folded) vs the ABA Rockets/
  Nuggets line that became today's Denver Nuggets (one franchise, two ids).
- "Baltimore Bullets": 1948-1955 franchise (folded 1955) vs the 1963-1973
  (Chicago Packers/Zephyrs relocation) franchise that continues as Washington.
- "Washington Capitols": BAA/NBA 1946-1951 (folded) vs the ABA 1969-70
  "Washington Caps" (Oakland Oaks -> Virginia Squires line; display name in S2
  is also "Washington Capitols").
- "Charlotte Hornets": 1988-2002 CHH -> New Orleans -> Pelicans is one franchise
  (id HORNETS-PELICANS); the 2004 Bobcats -> 2014 renamed Hornets is a different
  franchise (id BOBCATS-HORNETS2). S1 abbreviates both with CHA (collision), S2
  uses CHH / CHA(Bobcats) / CHO(Hornets-2).
"""

# order in each tuple: lg, team_name, abbr_bbr, abbr_s1, season_start, season_end
FRANCHISES = {
    # --- continuous / relocated franchises -----------------------------------
    "HAWKS": [
        ("NBA", "Tri-Cities Blackhawks", "TRI", "TCB", 1950, 1951),
        ("NBA", "Milwaukee Hawks", "MLH", "MIH", 1952, 1955),
        ("NBA", "St. Louis Hawks", "STL", "STL", 1956, 1968),
        ("NBA", "Atlanta Hawks", "ATL", "ATL", 1969, 2026),
    ],
    "CELTICS": [
        ("BAA", "Boston Celtics", "BOS", "BOS", 1947, 1949),
        ("NBA", "Boston Celtics", "BOS", "BOS", 1950, 2026),
    ],
    "NETS": [
        ("ABA", "New Jersey Americans", "NJA", None, 1968, 1968),
        ("ABA", "New York Nets", "NYA", None, 1969, 1976),
        ("NBA", "New York Nets", "NYN", None, 1977, 1977),
        ("NBA", "New Jersey Nets", "NJN", "NJN", 1978, 2012),
        ("NBA", "Brooklyn Nets", "BRK", "BKN", 2013, 2026),
    ],
    "BOBCATS-HORNETS2": [
        ("NBA", "Charlotte Bobcats", "CHA", "CHA", 2005, 2014),
        ("NBA", "Charlotte Hornets", "CHO", "CHA", 2015, 2026),
    ],
    "HORNETS-PELICANS": [
        ("NBA", "Charlotte Hornets", "CHH", "CHH", 1989, 2002),
        ("NBA", "New Orleans Hornets", "NOH", "NOH", 2003, 2005),
        ("NBA", "New Orleans/Oklahoma City Hornets", "NOK", "NOK", 2006, 2007),
        ("NBA", "New Orleans Hornets", "NOH", "NOH", 2008, 2013),
        ("NBA", "New Orleans Pelicans", "NOP", "NOP", 2014, 2026),
    ],
    "BULLS": [("NBA", "Chicago Bulls", "CHI", "CHI", 1967, 2026)],
    "KNICKS": [
        ("BAA", "New York Knicks", "NYK", "NYK", 1947, 1949),
        ("NBA", "New York Knicks", "NYK", "NYK", 1950, 2026),
    ],
    "CAVALIERS": [("NBA", "Cleveland Cavaliers", "CLE", "CLE", 1971, 2026)],
    "MAVERICKS": [("NBA", "Dallas Mavericks", "DAL", "DAL", 1981, 2026)],
    "NUGGETS": [
        ("ABA", "Denver Rockets", "DNR", None, 1968, 1974),
        ("ABA", "Denver Nuggets", "DNA", None, 1975, 1976),
        ("NBA", "Denver Nuggets", "DEN", "DEN", 1977, 2026),
    ],
    "PISTONS": [
        ("BAA", "Fort Wayne Pistons", "FTW", "FTW", 1949, 1949),
        ("NBA", "Fort Wayne Pistons", "FTW", "FTW", 1950, 1957),
        ("NBA", "Detroit Pistons", "DET", "DET", 1958, 2026),
    ],
    "WARRIORS": [
        ("BAA", "Philadelphia Warriors", "PHW", "PHW", 1947, 1949),
        ("NBA", "Philadelphia Warriors", "PHW", "PHW", 1950, 1962),
        ("NBA", "San Francisco Warriors", "SFW", "SFW", 1963, 1971),
        ("NBA", "Golden State Warriors", "GSW", "GOS", 1972, 1996),
        ("NBA", "Golden State Warriors", "GSW", "GSW", 1997, 2026),
    ],
    "ROCKETS": [
        ("NBA", "San Diego Rockets", "SDR", "SDR", 1968, 1971),
        ("NBA", "Houston Rockets", "HOU", "HOU", 1972, 2026),
    ],
    "CLIPPERS": [
        ("NBA", "Buffalo Braves", "BUF", "BUF", 1971, 1978),
        ("NBA", "San Diego Clippers", "SDC", "SDC", 1979, 1984),
        ("NBA", "Los Angeles Clippers", "LAC", "LAC", 1985, 2026),
    ],
    "LAKERS": [
        ("BAA", "Minneapolis Lakers", "MNL", "MNL", 1949, 1949),
        ("NBA", "Minneapolis Lakers", "MNL", "MNL", 1950, 1960),
        ("NBA", "Los Angeles Lakers", "LAL", "LAL", 1961, 2026),
    ],
    "HEAT": [("NBA", "Miami Heat", "MIA", "MIA", 1989, 2026)],
    "BUCKS": [("NBA", "Milwaukee Bucks", "MIL", "MIL", 1969, 2026)],
    "TIMBERWOLVES": [("NBA", "Minnesota Timberwolves", "MIN", "MIN", 1990, 2026)],
    "PACERS": [
        ("ABA", "Indiana Pacers", "INA", None, 1968, 1976),
        ("NBA", "Indiana Pacers", "IND", "IND", 1977, 2026),
    ],
    "76ERS": [
        ("NBA", "Syracuse Nationals", "SYR", "SYR", 1950, 1963),
        ("NBA", "Philadelphia 76ers", "PHI", "PHL", 1964, 1996),
        ("NBA", "Philadelphia 76ers", "PHI", "PHI", 1997, 2026),
    ],
    "SUNS": [("NBA", "Phoenix Suns", "PHO", "PHX", 1969, 2026)],
    "TRAIL BLAZERS": [("NBA", "Portland Trail Blazers", "POR", "POR", 1971, 2026)],
    "KINGS": [
        ("BAA", "Rochester Royals", "ROC", "ROC", 1949, 1949),
        ("NBA", "Rochester Royals", "ROC", "ROC", 1950, 1957),
        ("NBA", "Cincinnati Royals", "CIN", "CIN", 1958, 1972),
        ("NBA", "Kansas City-Omaha Kings", "KCO", "KCK", 1973, 1975),
        ("NBA", "Kansas City Kings", "KCK", "KCK", 1976, 1985),
        ("NBA", "Sacramento Kings", "SAC", "SAC", 1986, 2026),
    ],
    "SPURS": [
        ("ABA", "Dallas Chaparrals", "DLC", None, 1968, 1970),
        ("ABA", "Texas Chaparrals", "TEX", None, 1971, 1971),
        ("ABA", "Dallas Chaparrals", "DLC", None, 1972, 1973),
        ("ABA", "San Antonio Spurs", "SAA", None, 1974, 1976),
        ("NBA", "San Antonio Spurs", "SAS", "SAS", 1977, 2026),
    ],
    "RAPTORS": [("NBA", "Toronto Raptors", "TOR", "TOR", 1996, 2026)],
    "JAZZ": [
        ("NBA", "New Orleans Jazz", "NOJ", "NOJ", 1975, 1979),
        ("NBA", "Utah Jazz", "UTA", "UTH", 1980, 1996),
        ("NBA", "Utah Jazz", "UTA", "UTA", 1997, 2026),
    ],
    "GRIZZLIES": [
        ("NBA", "Vancouver Grizzlies", "VAN", "VAN", 1996, 2001),
        ("NBA", "Memphis Grizzlies", "MEM", "MEM", 2002, 2026),
    ],
    "THUNDER": [
        ("NBA", "Seattle SuperSonics", "SEA", "SEA", 1968, 2008),
        ("NBA", "Oklahoma City Thunder", "OKC", "OKC", 2009, 2026),
    ],
    "WIZARDS": [
        ("NBA", "Chicago Packers", "CHP", "CHP", 1962, 1962),
        ("NBA", "Chicago Zephyrs", "CHZ", "CHZ", 1963, 1963),
        ("NBA", "Baltimore Bullets", "BLB", "BLT", 1964, 1973),
        ("NBA", "Capital Bullets", "CAP", "CAP", 1974, 1974),
        ("NBA", "Washington Bullets", "WSB", "WAS", 1975, 1997),
        ("NBA", "Washington Wizards", "WAS", "WAS", 1998, 2026),
    ],
    "MAGIC": [("NBA", "Orlando Magic", "ORL", "ORL", 1990, 2026)],

    # --- defunct BAA/NBA franchises (their NBA/BAA players stay in scope) -----
    "HUSKIES": [("BAA", "Toronto Huskies", "TRH", "HUS", 1947, 1947)],
    "REBELS": [("BAA", "Cleveland Rebels", "CLR", "CLR", 1947, 1947)],
    "IRONMEN": [("BAA", "Pittsburgh Ironmen", "PIT", "PIT", 1947, 1947)],
    "FALCONS": [("BAA", "Detroit Falcons", "DTF", "DEF", 1947, 1947)],
    "BOMBERS": [
        ("BAA", "St. Louis Bombers", "STB", "BOM", 1947, 1949),
        ("NBA", "St. Louis Bombers", "STB", "BOM", 1950, 1950),
    ],
    "STAGS": [
        ("BAA", "Chicago Stags", "CHS", "CHS", 1947, 1949),
        ("NBA", "Chicago Stags", "CHS", "CHS", 1950, 1950),
    ],
    "CAPITOLS": [
        ("BAA", "Washington Capitols", "WSC", "WAS", 1947, 1949),
        ("NBA", "Washington Capitols", "WSC", "WAS", 1950, 1951),
    ],
    "BULLETS-DEFUNCT": [
        ("BAA", "Baltimore Bullets", "BLB", "BAL", 1948, 1949),
        ("NBA", "Baltimore Bullets", "BLB", "BAL", 1950, 1955),
    ],
    "PACKERS-ANDERSON": [("NBA", "Anderson Packers", "AND", "AND", 1950, 1950)],
    "REDSKINS-SHEBOYGAN": [("NBA", "Sheboygan Red Skins", "SHE", "SHE", 1950, 1950)],
    "WATERLOO-HAWKS": [("NBA", "Waterloo Hawks", "WAT", "WAT", 1950, 1950)],
    "NUGGETS-DEFUNCT": [("NBA", "Denver Nuggets", "DNN", "DN", 1950, 1950)],
    "OLYMPIANS": [("NBA", "Indianapolis Olympians", "INO", "INO", 1950, 1953)],
    "JETS": [("BAA", "Indianapolis Jets", "INJ", "JET", 1949, 1949)],
    "STEAMROLLERS": [("BAA", "Providence Steamrollers", "PRO", "PRO", 1947, 1949)],

    # --- ABA-only franchises (out of NBA/BAA scope; used to bucket ABA-only
    #     players and to prove no accidental NBA merge happened) ----------------
    "ABA-AMIGOS": [("ABA", "Anaheim Amigos", "ANA", None, 1968, 1968)],
    "ABA-STARS": [
        ("ABA", "Los Angeles Stars", "LAS", None, 1969, 1970),
        ("ABA", "Utah Stars", "UTS", None, 1971, 1976),
    ],
    "ABA-OAKS-CAPS-SQUIRES": [
        ("ABA", "Oakland Oaks", "OAK", None, 1968, 1969),
        ("ABA", "Washington Capitols", "WSA", None, 1970, 1970),
        ("ABA", "Virginia Squires", "VIR", None, 1971, 1976),
    ],
    "ABA-PIPER-CONDOR": [
        ("ABA", "Pittsburgh Pipers", "PTP", None, 1968, 1968),
        ("ABA", "Minnesota Pipers", "MNP", None, 1969, 1969),
        ("ABA", "Pittsburgh Pipers", "PTP", None, 1970, 1970),
        ("ABA", "Pittsburgh Condors", "PTC", None, 1971, 1972),
    ],
    "ABA-MUSKIES-FLORIDIANS": [
        ("ABA", "Minnesota Muskies", "MNM", None, 1968, 1968),
        ("ABA", "Miami Floridians", "MMF", None, 1969, 1970),
        ("ABA", "The Floridians", "FLO", None, 1971, 1972),
    ],
    "ABA-MAVERICKS-COUGARS-SPIRITS": [
        ("ABA", "Houston Mavericks", "HSM", None, 1968, 1969),
        ("ABA", "Carolina Cougars", "CAR", None, 1970, 1974),
        ("ABA", "Spirits of St. Louis", "SSL", None, 1975, 1976),
    ],
    "ABA-BUCCANEERS-MEMPHIS": [
        ("ABA", "New Orleans Buccaneers", "NOB", None, 1968, 1970),
        ("ABA", "Memphis Pros", "MMP", None, 1971, 1972),
        ("ABA", "Memphis Tams", "MMT", None, 1973, 1974),
        ("ABA", "Memphis Sounds", "MMS", None, 1975, 1975),
    ],
    "ABA-CONQUISTADORS": [
        ("ABA", "San Diego Conquistadors", "SDA", None, 1973, 1975),
        ("ABA", "San Diego Sails", "SDS", None, 1976, 1976),
    ],
    "ABA-COLONELS": [("ABA", "Kentucky Colonels", "KEN", None, 1968, 1976)],
}

# S2 ABA abbreviation reuses ("DLC" appears in two eras of the same ABA
# franchise; "PTP" likewise) are handled by season-scoped resolution, so the
# two "DLC" eras above keep distinct keys only for the season-scope check.
# The ABA seed rows above use fresh abbr keys (DLC2/PTP2) where S2 genuinely
# repeats an abbreviation within one franchise's ABA-era line.

# --- S1 (stats.nba.com) game-table team-name aliases --------------------------
# S1 game team_name values that differ from the S2/BBR display name for the same
# franchise-season; canonical resolution is name-based.
S1_TEAM_NAME_FIXUPS = {
    "fort wayne zollner pistons": "Fort Wayne Pistons",
    "sheboygan redskins": "Sheboygan Red Skins",
    "la clippers": "Los Angeles Clippers",
}

# S1 season_id '21946' -> season 1947 (1946-47). Leading digit = season-type
# family, last four digits = season starting year; S2 `season` = starting+1.
def s1_season_id_to_season_year(season_id) -> int:
    return int(str(season_id)[1:5]) + 1

# S1 game abbreviations/names that are not franchises and are excluded from the
# crosswalk scope (spec: non-NBA/BAA team settings; global/exhibition games).
S1_EXCLUDED_ABBREVIATIONS = {
    "EST": "All-Star roster (East) - out of scope",
    "WST": "All-Star roster (West) - out of scope",
    "LBN": "All-Star roster (Team LeBron) - out of scope",
    "STP": "All-Star roster (Team Stephen) - out of scope",
    "CHN": "Global/preseason exhibition opponent - out of scope",
    "FBU": "Global/preseason exhibition opponent - out of scope",
    "MLN": "Global/preseason exhibition opponent - out of scope",
    "ADL": "Global/preseason exhibition opponent - out of scope",
    "ALB": "Global/preseason exhibition opponent - out of scope",
    "BNE": "Global/preseason exhibition opponent - out of scope",
    "FEN": "Global/preseason exhibition opponent - out of scope",
    "GUA": "Global/preseason exhibition opponent - out of scope",
    "KHI": "Global/preseason exhibition opponent - out of scope",
    "LAB": "Global/preseason exhibition opponent - out of scope",
    "MAC": "Global/preseason exhibition opponent - out of scope",
    "MAL": "Global/preseason exhibition opponent - out of scope",
    "MEL": "Global/preseason exhibition opponent - out of scope",
    "MOS": "Global/preseason exhibition opponent - out of scope",
    "PAN": "Global/preseason exhibition opponent - out of scope",
    "PAR": "Global/preseason exhibition opponent - out of scope",
    "RMA": "Global/preseason exhibition opponent - out of scope",
    "ROM": "Global/preseason exhibition opponent - out of scope",
    "SYD": "Global/preseason exhibition opponent - out of scope",
    "UBB": "Global/preseason exhibition opponent - out of scope",
}

# S1 `game`-table team_name values observed for global/exhibition opponents that
# also resolve via the exclusion list above (kept name-agnostic in code).

# --- curated player-name aliases (S1 display name -> S2 season-info name) -----
# Only for cases the automated name/DOB bridge demonstrably cannot resolve
# (nicknames BBR uses in place of legal names). Each entry is verified manually
# against the S2 'Player Career Info.csv' row with matching birth date before
# inclusion. Alias-bridged players stay listed in the reconciliation report
# (classification D1) for explicit review.
PLAYER_ALIASES_S1_TO_S2 = {
    # filled from evidence during the reconciliation run; see report section D1
}
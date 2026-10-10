"""Lightweight canonical franchise-era mapping shared by T3 and T4."""
from collections import defaultdict
from t3_franchise_seed import FRANCHISES


def franchise_era_index():
    """(league, season) -> canonical franchise and complete seed era tuple."""
    index = defaultdict(list)
    for canonical_id, eras in FRANCHISES.items():
        for era in eras:
            for season in range(era[4], era[5] + 1):
                index[(era[0], season)].append((canonical_id, era))
    return index


def resolve_s1_abbr(index, season, abbreviation):
    return {canonical_id for league in ("NBA", "BAA")
            for canonical_id, era in index.get((league, season), [])
            if era[3] == abbreviation}

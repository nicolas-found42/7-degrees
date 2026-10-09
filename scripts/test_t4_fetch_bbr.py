#!/usr/bin/env python3
"""Focused T4 fetch-ledger tests; no network or data files are used."""

import os
import sys
import unittest
from datetime import datetime, timezone

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

from t4_fetch_bbr import FetchOutcome, http_ledger_record  # noqa: E402


class TestRequestLedgerPrecision(unittest.TestCase):
    def test_http_events_record_fractional_utc_and_measured_spacing(self):
        started = datetime(2026, 10, 9, 12, 34, 56, 123456, tzinfo=timezone.utc)
        record = http_ledger_record(
            "http-get", "https://example.invalid/page", FetchOutcome(200, b"ok"),
            1, started, 5.234567)
        self.assertEqual(record["ts_utc"], "2026-10-09T12:34:56.123456Z")
        self.assertEqual(record["elapsed_since_previous_start_seconds"], 5.234567)
        self.assertEqual(record["start_spacing_seconds"], 5.234567)
        self.assertEqual(record["status"], 200)


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
"""Reconcile saved wCOP privilege events with independent transaction logs."""
from collections import Counter, defaultdict
import hashlib
import json
from urllib.parse import quote
from urllib.request import Request, urlopen
import sys

from wcop_history import TOPICS
from wcop_trace import TOKEN, WORK, private_write, read

BASE = "https://eth.blockscout.com/api/v2/transactions/"
KINDS = {"upgraded", "initialized", "role_granted", "role_revoked"}
UA = "Toupera-Agent/1.0 forensic-event-reconciliation"


def get_json(url):
    request = Request(url, headers={"User-Agent": UA, "Accept": "application/json"})
    with urlopen(request, timeout=15) as response:
        if response.status != 200:
            raise ValueError("unexpected explorer HTTP status")
        return json.load(response)


def transaction_logs(tx):
    logs = []
    params = None
    for _ in range(10):
        url = BASE + quote(tx, safe="") + "/logs"
        if params:
            from urllib.parse import urlencode
            url += "?" + urlencode(params)
        page = get_json(url)
        items = page.get("items")
        if not isinstance(items, list):
            raise ValueError("transaction logs missing")
        logs.extend(items)
        params = page.get("next_page_params")
        if params is None:
            return logs
        if not isinstance(params, dict) or not params:
            raise ValueError("invalid explorer log cursor")
    raise ValueError("transaction log pagination limit")


def run(job_id):
    if len(job_id) != 16 or any(c not in "0123456789abcdef" for c in job_id):
        raise ValueError("invalid job ID")
    job = WORK / job_id
    history, roles, path_map, engine = (read(job / name) for name in
        ("history.json", "roles.json", "path-map.json", "engine-impl.json"))
    block = int(history["through_block"])
    code_hash = hashlib.sha256(bytes.fromhex(engine["normalized_hex"].removeprefix("0x"))).hexdigest()
    if (history.get("address") != TOKEN or not history.get("complete_event_range") or
            path_map.get("block") != block or path_map.get("engine_code_sha256") != code_hash or
            roles.get("event_replay_matches_current") is not True):
        raise ValueError("saved history identity or role replay mismatch")
    expected = defaultdict(list)
    for row in history["timeline"]:
        if row["kind"] in KINDS:
            tx = row["tx"].lower()
            if len(tx) != 66 or not tx.startswith("0x") or any(c not in "0123456789abcdef" for c in tx[2:]):
                raise ValueError("invalid saved transaction hash")
            expected[tx].append(row)
    if history["creation_tx"].lower() not in expected:
        raise ValueError("creation transaction absent from significant events")
    rows = []
    for tx, events in sorted(expected.items(), key=lambda pair: min(x["block"] for x in pair[1])):
        detail = get_json(BASE + quote(tx, safe=""))
        if (detail.get("hash", "").lower() != tx or detail.get("status") != "ok" or
                int(detail.get("block_number")) != events[0]["block"]):
            raise ValueError("transaction identity or status mismatch: " + tx)
        logs = transaction_logs(tx)
        index = {}
        for item in logs:
            if item.get("transaction_hash", "").lower() != tx:
                raise ValueError("transaction log hash mismatch")
            log_index = int(item["index"])
            if log_index in index:
                raise ValueError("duplicate transaction log index")
            index[log_index] = item
        for event in events:
            item = index.get(int(event["log_index"]))
            if item is None or int(item["block_number"]) != event["block"]:
                raise ValueError("saved event missing in transaction logs")
            address = item.get("address") or {}
            address = address.get("hash") if isinstance(address, dict) else address
            topics = item.get("topics") or []
            if (not isinstance(address, str) or address.lower() != TOKEN or
                    not topics or topics[0].lower() != TOPICS[event["kind"]]):
                raise ValueError("saved event address or topic mismatch")
            if event["kind"] == "upgraded" and (len(topics) < 2 or
                    "0x" + topics[1][-40:].lower() != event["implementation"].lower()):
                raise ValueError("upgraded address differs")
            if event["kind"] in ("role_granted", "role_revoked") and (len(topics) < 3 or
                    topics[1].lower() != event["role"].lower() or
                    "0x" + topics[2][-40:].lower() != event["account"].lower()):
                raise ValueError("role event details differ")
        raw_input = detail.get("raw_input") or ""
        rows.append({"tx": tx, "block": events[0]["block"], "status": "ok",
                     "events_matched": len(events), "event_kinds": dict(Counter(x["kind"] for x in events)),
                     "input_selector": raw_input[:10] if len(raw_input) >= 10 else None,
                     "explorer": BASE + tx})
    report = {"format": 1, "job_id": job_id, "block": block, "contract": TOKEN,
              "engine_code_sha256": code_hash,
              "history_sha256": hashlib.sha256((job / "history.json").read_bytes()).hexdigest(),
              "source": "Blockscout transaction details and transaction logs",
              "transaction_count": len(rows), "matched_significant_events": sum(x["events_matched"] for x in rows),
              "event_kinds": dict(Counter(x["kind"] for events in expected.values() for x in events)),
              "transactions": rows,
              "historical_storage_status": "not_verified_by_archive_rpc",
              "limits": ["Transaction receipts/logs corroborate event emission, not all past storage values.",
                         "The old-block archive RPC was unavailable; no storage state is inferred from logs alone.",
                         "Only deployment, upgrade, initialization and role-change transactions are reconciled here."]}
    private_write(job / "historical-reconciliation.json", report)
    return {k: report[k] for k in ("job_id", "transaction_count", "matched_significant_events",
                                    "event_kinds", "historical_storage_status")}


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("usage: wcop_historical_reconcile.py <job-id>")
    print(json.dumps(run(sys.argv[1]), ensure_ascii=False))

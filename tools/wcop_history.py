"""Bounded, provenance-preserving wCOP event history from Blockscout and pinned RPC."""
from collections import Counter
from urllib.parse import urlencode

EXPLORER = "https://eth.blockscout.com"
ZERO = "0x" + "0" * 40
TOPICS = {
    "transfer": "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
    "approval": "0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925",
    "upgraded": "0xbc7cd75a20ee27fd9adebab32041f755214dbc6bffa90cc0225b39da2e5c2d3b",
    "role_granted": "0x2f8788117e7eff1d82e926ec794901d17c78024a50270940304540a733656f0d",
    "role_revoked": "0xf6391f5c32d9c69d2a47ea670b442974b53935d1edc7fd64eb21e047a839171b",
    "initialized": "0xc7f505b2f371ae2175ee4913f4499e1f2633a7b5936321eed1cdaeb6115181d2",
    "paused": "0x62e78cea01bee320cd4e420270b5ea74000d11b0c9f74754ebdbfc544b05a258",
    "unpaused": "0x5db9ee0a495bf2e6ff9c91a7834c1ba4fdd244a5e8aa4e537bd38aeae4b073aa",
}

def number(value):
    return int(value, 16) if isinstance(value, str) and value.startswith("0x") else int(value)

def addr(topic):
    if not isinstance(topic, str) or len(topic) != 66:
        raise ValueError("endereço de evento inválido")
    return "0x" + topic[-40:].lower()

def collect_logs_v1(address, first, block, request_json):
    events = []
    seen = set()
    pages = 0
    while pages < 30:
        pages += 1
        query = urlencode({"module": "logs", "action": "getLogs", "address": address,
                           "fromBlock": first, "toBlock": block, "page": pages, "offset": 1000})
        response = request_json(EXPLORER + "/api?" + query)
        raw = response.get("result")
        if response.get("status") != "1" or not isinstance(raw, list):
            raise ValueError("consulta de eventos incompleta na página " + str(pages))
        for item in raw:
            key = (item.get("transactionHash"), item.get("logIndex"))
            n = number(item["blockNumber"])
            if not first <= n <= block or key in seen or item.get("address", "").lower() != address:
                raise ValueError("sequência de eventos inconsistente")
            seen.add(key)
            events.append(item)
        if len(raw) < 1000:
            break
    else:
        raise ValueError("limite de páginas do histórico atingido")
    return events, pages

def collect_logs_v2(address, first, block, request_json):
    base = EXPLORER + "/api/v2/addresses/" + address + "/logs"
    params = None
    events = []
    seen = set()
    for page in range(1, 31):
        url = base + ("?" + urlencode(params) if params else "")
        response = request_json(url)
        raw = response.get("items")
        if not isinstance(raw, list):
            raise ValueError("consulta v2 de eventos incompleta na página " + str(page))
        for item in raw:
            n = number(item["block_number"])
            if n > block:
                continue
            if n < first:
                raise ValueError("evento anterior à criação")
            location = item.get("address") or {}
            logged_address = location.get("hash") if isinstance(location, dict) else location
            key = (item.get("transaction_hash"), number(item["index"]))
            if not isinstance(logged_address, str) or logged_address.lower() != address or key in seen:
                raise ValueError("sequência v2 de eventos inconsistente")
            seen.add(key)
            events.append({"transactionHash": key[0], "logIndex": key[1],
                           "blockNumber": n, "address": address,
                           "topics": item.get("topics") or [], "data": item.get("data")})
        params = response.get("next_page_params")
        if params is None:
            return events, page
        if not isinstance(params, dict) or not params:
            raise ValueError("cursor v2 de eventos inválido")
    raise ValueError("limite de páginas v2 do histórico atingido")

def collect(address, block, request_json, rpc):
    metadata = request_json(EXPLORER + "/api/v2/addresses/" + address)
    creation_tx = metadata.get("creation_transaction_hash")
    if not creation_tx:
        raise ValueError("transação de criação indisponível")
    creation = request_json(EXPLORER + "/api/v2/transactions/" + creation_tx)
    first = number(creation["block_number"])
    if first > block:
        raise ValueError("criação posterior ao bloco fixado")
    try:
        events, pages = collect_logs_v2(address, first, block, request_json)
    except (KeyError, ValueError, TypeError, OSError):
        events, pages = collect_logs_v1(address, first, block, request_json)
    events.sort(key=lambda row: (number(row["blockNumber"]), number(row["logIndex"])))
    counts = Counter()
    minted = burned = transfer_volume = 0
    recipients = Counter()
    timeline = []
    movements = []
    for item in events:
        topics = [t.lower() for t in item.get("topics", []) if isinstance(t, str)]
        kind = next((name for name, topic in TOPICS.items() if topics and topics[0] == topic), "other")
        counts[kind] += 1
        where = {"block": number(item["blockNumber"]), "tx": item["transactionHash"],
                 "log_index": number(item["logIndex"]), "kind": kind}
        if kind == "transfer":
            if len(topics) < 3:
                raise ValueError("Transfer sem contas indexadas")
            sender, receiver = addr(topics[1]), addr(topics[2])
            amount = number(item["data"])
            if sender == ZERO:
                minted += amount
                movement_kind = "mint"
            elif receiver == ZERO:
                burned += amount
                movement_kind = "burn"
            else:
                transfer_volume += amount
                movement_kind = "transfer"
            where.update({"from": sender, "to": receiver, "amount_raw": str(amount), "flow": movement_kind})
            movements.append(where)
            if receiver != ZERO:
                recipients[receiver] += amount
        else:
            if kind == "approval" and len(topics) > 2:
                where["owner"] = addr(topics[1])
                where["spender"] = addr(topics[2])
                where["amount_raw"] = str(number(item["data"]))
            if kind == "upgraded" and len(topics) > 1:
                where["implementation"] = addr(topics[1])
            if kind in ("role_granted", "role_revoked") and len(topics) > 2:
                where["role"] = topics[1]
                where["account"] = addr(topics[2])
            timeline.append(where)
    supply = number(rpc("eth_call", [{"to": address, "data": "0x18160ddd"}, hex(block)]))
    computed = minted - burned
    holder = None
    for account, _ in recipients.most_common(24):
        value = number(rpc("eth_call", [{"to": address, "data": "0x70a08231" + account[2:].rjust(64, "0")}, hex(block)]))
        if value > 0:
            holder = {"address": account, "balance_raw": str(value)}
            break
    return {"address": address, "creation_tx": creation_tx, "creation_block": first,
            "through_block": block, "source": EXPLORER, "pages": pages,
            "complete_event_range": True, "event_count": len(events), "event_counts": dict(counts),
            "minted_raw": str(minted), "burned_raw": str(burned),
            "ordinary_transfer_volume_raw": str(transfer_volume),
            "supply_from_events_raw": str(computed), "supply_on_chain_raw": str(supply),
            "supply_matches_events": computed == supply, "funded_holder": holder,
            "timeline": timeline, "movements": movements}

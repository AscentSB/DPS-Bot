#!/usr/bin/env python3
"""Regenerate pgo/train.json.gz, the training input for the profile-guided Docker build.

Run from the rust/ directory, with the (gitignored) testdata/ directory present:
    python3 pgo/make-train.py

It packs the game-data resource, the price snapshot, a handful of anonymous member
snapshots (item/level/pet data only, no names or UUIDs) and their DpsJobs into one
gzipped JSON file. pgo_train (crates/calc/examples) replays it through run_dps.
"""
import gzip, json, os, sys

# Optional: make-train.py [out.json.gz [profile ...]] (e.g. a held-out set to time a PGO build on).
PROFILES = ["BattleDay", "Ben_MC", "MR_R3dh3ad", "Scraamss", "recticc", "Igrag", "Boll3n"]

def strip(o):
    """Drop per-item instance ids (the calc never reads them)."""
    if isinstance(o, dict):
        return {k: strip(v) for k, v in o.items() if k not in ("uuid", "uniqueId")}
    if isinstance(o, list):
        return [strip(v) for v in o]
    return o

OUT = sys.argv[1] if len(sys.argv) > 1 else "pgo/train.json.gz"
if len(sys.argv) > 2:
    PROFILES = sys.argv[2:]

snap = json.load(open("testdata/snapshot.json"))
cases = json.load(open("testdata/cases.json"))
members, index, out_cases = [], {}, []
for c in cases:
    p = c["profile"]
    if p not in PROFILES:
        continue
    if p not in index:
        index[p] = len(members)
        members.append(strip(json.load(open(f"testdata/members/{p}.json"))))
    out_cases.append({"member": index[p], "job": c["job"]})
out = {
    "items": snap["items"],
    "bazaar": snap["bazaar"],
    "auctions": snap["auctions"],
    "members": members,
    "cases": out_cases,
}
with gzip.GzipFile(OUT, "wb", mtime=0) as f:
    f.write(json.dumps(out, separators=(",", ":")).encode())
print(len(members), "members,", len(out_cases), "cases,", os.path.getsize(OUT), "bytes")

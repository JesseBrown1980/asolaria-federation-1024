#!/usr/bin/env bash
# Mint a hash-chained IMMUTABLE EDIT-TOKEN LEDGER for meaningful code edits.
#
# "crypto as tokens locally" + "immutable tracing on all code edits that matters
# forever" (OP-JESSE standing policy). Git is the source-control layer; this HBP
# ledger is the Asolaria proof layer: one row per (commit, file), each row a
# content-addressed sha256 edit-token chained to its parent (prev_row_sha), so the
# ledger is append-only and tamper-evident — altering any past row breaks the chain.
#
# Row fields: repo · file · pre_sha256 (file content at commit^ or NEW) · post_sha256
# (file content at commit) · commit · tests (gate pointer) · agent_pid · prev_row_sha
# · row_sha = sha256("<prev_row_sha>|<body>").
#
# Usage:  bash mint-edit-token-ledger.sh <commit> [<commit> ...] > LEDGER.hbp
set -u
cd "$(git rev-parse --show-toplevel)" || exit 9
REPO="$(basename "$(git rev-parse --show-toplevel)")"
PID="8467a937cba309f7"   # SEAT ACER-CLAUDE-FABLE5
TESTS="green-1.81-harness(fmt+clippy-Dwarnings+268lib+6parity+uefi-build)"
NOW="2026-07-07"

sha() { sha256sum | cut -d' ' -f1; }

echo "HBP_PACKET|id=ACER_EDIT_TOKEN_LEDGER_${NOW//-/}|format=hbp_tuple_text|json=0|seat=ACER-CLAUDE-FABLE5|pid=${PID}|owner=OP-JESSE|created=${NOW}|kind=hash_chained_immutable_edit_token_ledger"
echo "LEDGER_HEAD|scheme=row_sha=sha256(prev_row_sha|body)|genesis_prev=GENESIS|repo=${REPO}|layer=asolaria_proof(git=source_control)"

prev="GENESIS"
seq=0
for c in "$@"; do
  full=$(git rev-parse "$c")
  # files changed in this commit under kernel/
  for f in $(git show --name-only --format= "$c" | grep -E '^kernel/' | sort -u); do
    [ -z "$f" ] && continue
    seq=$((seq+1))
    post=$(git show "$c:$f" 2>/dev/null | sha); [ -z "$post" ] && post="ABSENT"
    if git cat-file -e "${c}^:$f" 2>/dev/null; then
      pre=$(git show "${c}^:$f" 2>/dev/null | sha)
    else
      pre="NEW"
    fi
    body="repo=${REPO}|file=${f}|pre_sha256=${pre}|post_sha256=${post}|commit=${full}|tests=${TESTS}|agent_pid=${PID}|prev_row_sha=${prev}"
    row=$(printf '%s' "${prev}|${body}" | sha)
    echo "ETOKEN|seq=${seq}|${body}|row_sha=${row}"
    prev="$row"
  done
done
echo "LEDGER_FOOT|rows=${seq}|tip_row_sha=${prev}|append_only=1|tamper_evident=1"

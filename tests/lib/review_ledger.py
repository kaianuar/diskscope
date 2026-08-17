#!/usr/bin/env python3
"""
review_ledger.py - omp-agent's structured issue-ledger logic.

Tracks every finding the adversarial critic raises, keyed by a STABLE SYMBOL
(e.g. `test_incremental_scan`, a fn/struct name) so re-phrasings of the same
issue match. This breaks the "re-raise forever" deadlock where a critic keeps
re-flagging an item even after the fix landed.

Statuses:
  OPEN     - a P0/P1 finding currently outstanding (blocks until fixed)
  RESOLVED - a prior OPEN P0/P1 whose symbol was NOT re-raised this round (fix landed)
  BACKLOG  - a P2/P3/P4 finding (non-blocking; surfaced to the human, not gating)

This module is used by tests/review_gate.sh (per-phase ledger) and exercised by
tests/pipeline_self_test.sh. It lives ONLY in omp-agent and is NOT scaffolded
into consumer projects.
"""

import os
import re

# Regexes - keep identical to the inline versions in review_gate.sh heredoc.
LEDGER_LINE = re.compile(r'\[P(\d)\]\s+(\w+)\s+(.+)')
FINDING = re.compile(r'\[P(\d)\]\s*(.{0,120})')
IDENT = re.compile(r'\b((?:test_)?[a-z][a-z0-9_]{2,})\b')
BACKTICK = re.compile(r'`([A-Za-z_][A-Za-z0-9_:]*)`')


def key_of(text):
    """Return a stable key for a finding: first backtick-quoted symbol, else the
    first test_/function-like token, else a truncated lowercase text."""
    m = BACKTICK.search(text or '')
    if m:
        return m.group(1).lower()
    m = IDENT.search(text or '')
    return m.group(1).lower() if m else (text or '').lower()[:60]


def _parse_ledger(path):
    prior = {}
    if os.path.exists(path):
        for line in open(path, encoding='utf-8', errors='replace'):
            m = LEDGER_LINE.match(line.strip())
            if m:
                prior[key_of(m.group(3))] = (m.group(1), m.group(3), m.group(2))
    return prior


def _parse_verdict(verdict_text):
    out = []
    for m in FINDING.finditer(verdict_text or ''):
        sev, txt = m.group(1), m.group(2).strip()
        if txt and sev in ('0', '1', '2', '3', '4'):
            out.append((sev, txt, key_of(txt)))
    return out


def update_ledger(ledger_path, verdict_text):
    """Merge a round's verdict into the ledger file. Returns the new ledger text."""
    prior = _parse_ledger(ledger_path)
    cur = _parse_verdict(verdict_text)

    new_lines, seen = [], set()
    for sev, txt, key in cur:
        seen.add(key)
        if key in prior and prior[key][0] == sev:
            status = prior[key][2]
        else:
            status = 'OPEN' if sev in ('0', '1') else 'BACKLOG'
        new_lines.append('[P%s] %s %s' % (sev, status, txt.strip()[:120]))

    # Prior OPEN P0/P1 whose symbol was NOT re-raised -> RESOLVED.
    for key, (psev, ptxt, pstatus) in prior.items():
        if psev in ('0', '1') and key not in seen:
            new_lines.append('[P%s] RESOLVED %s' % (psev, ptxt.strip()[:120]))

    out_lines = sorted(set(new_lines),
                       key=lambda x: (x[1:3], x[4:9]))
    out = '\n'.join(out_lines)
    if out_lines:
        out += '\n'
    with open(ledger_path, 'w', encoding='utf-8') as f:
        f.write(out)
    return out


if __name__ == '__main__':
    import sys
    if len(sys.argv) != 3:
        print('usage: review_ledger.py <ledger_path> <verdict_file>', file=sys.stderr)
        sys.exit(2)
    ledger_path, verdict_path = sys.argv[1], sys.argv[2]
    text = open(verdict_path, encoding='utf-8', errors='replace').read()
    update_ledger(ledger_path, text)

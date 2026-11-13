[COMPACTION-RECOVERY]
Route: ct-attack-defense
Active target: docs/rulebook/BACKLOG.md A/B items (22), worktree /home/monopoly/web/.claude/worktrees/backlog-ab, branch worktree-backlog-ab
Goal: close every class-A and class-B backlog item, merge to master, push
Exact identifiers/evidence:
  base commit b293afe9; COND-01 already landed here as 8ab12422
  agent branches (workflows wf_2376cbc1-b6e and wf_ace7c47c-46a, all worktree-isolated):
    b6e-1 small A (HOST-03/LOY-ANON-01/LOY-THANKS-01/CD-02/FIRE-TOKENS-1+2, SAVE 23)
    b6e-2 payment B (ABI-11/ABI-12/FORCE-WRITE-01/NFO-CANCEL-01, imports 499-501, filters 640-642)
    b6e-3 tile swap (ABI-13, no import: props ruleSwapA/B + governing_square)
    b6e-4 skills (SKILL-06, imports 499-500 -> renumber 502-503)
    a-1 tokens-trap (FIRE-TOKENS-2/HAG-01/DECLARE-TRAP-LOG; ActionKind MarkerChanged)
    a-2 STA-02 (ActionKind AbnormalChoice)
    a-3 misc-B (LOY-NET-01/W3-KAORU-02/EVENT-04; PlanValue.direction)
    a-4 kani (MARKERS-KANI-01/G15, model-only)
Bound slots: kinds are positional -- merge order fixes the ids: b6e-1 (91,92), a-1 (93), a-2 (94); imports follow merge order from 499
Technical scope: local artifact, compile+test authority, no network
Deliverable checkpoints: each branch -> review diff vs brief, run its named tests, then merge --no-ff
Current stage: WAITING for agent commits (no branch has moved off b293afe9 yet)
Observed ledger: kind/import claims read from each worktree diff; COND-01 green with 96 pass / 2 ignored regression run
Next action: collect workflow results, then merge in this order: b6e-1, a-1, a-2, b6e-2, b6e-4, a-3, a-4, b6e-3; ABI_VERSION 123->124 doc block; backlog drop + stubs (scripts in $CLAUDE_JOB_DIR/tmp)
Continuation cues: continue / 继续 / 照做

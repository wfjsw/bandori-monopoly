# Working in this repository

All changes must be made in a separate Git worktree on a task branch. Keep the
primary checkout clean; do not edit implementation files there.

Before editing, check `git status --short` and `git worktree list`. Create a new
worktree for the task unless the user has already assigned one. Use a directory
outside the primary checkout, and keep all source, documentation, asset, and
verification changes in that worktree. Do not merge into the primary branch or
remove another worktree unless the user requests it.

Report the branch and worktree path when delivering the work. Run the checks
appropriate to the change from that worktree.

Skip the Kani proofs and the fuzz targets in the default check pass — they are
opt-in only. Run them when the task is itself about proofs or fuzzing, or when
the user asks for them. The opt-in suites are `#[ignore]`d, so the default
`cargo test` pass skips them; run one with `-- --ignored`:

```sh
tools/kani/run.sh                                                    # proofs
cargo test -p rules-kani -- --ignored                                # kani conformance
FUZZ_ITERS=2000 cargo test -p ruleset-tests --test fuzz_interactions -- --ignored
cargo test -p ruleset-tests --test fuzz_found -- --ignored
```

Offload (only) complex implementations to subagents, with comprehensive 
instructions, guidance, and in-progress assistance. The main agent should 
periodically check on the subagent's progress. After subagent completed, 
the main agent should verify the subagent's work, and clean up temporary files.

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

Offload (only) complex implementations to subagents, with comprehensive 
instructions, guidance, and in-progress assistance.

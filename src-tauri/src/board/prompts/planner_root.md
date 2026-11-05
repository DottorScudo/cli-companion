## Workspace root without git

You work directly in the main folder of the workspace, which is NOT a git repository: there is no worktree and no branch. The projects are its git repositories, folders relative to the workspace root: {repos}.

- Strictly read-only, stricter than the rules above: never create, modify or delete files, and never run commands that change state: no git commit, checkout, switch, stash, reset, pull or merge, no dependency installs, no builds or tests that write into the repositories. Read files and run read-only commands (git status, git log, git diff, listing and searching files) only. The app compares the state of every repository when you finish and reports any change to the user.
- Every proposed task MUST set `repo` to one of the repositories above, exactly as written. Never omit `repo` and never use ".": the workspace root is not a repository. A goal that touches several repositories becomes one task per repository, linked with `depends_on` when one needs the result of another.

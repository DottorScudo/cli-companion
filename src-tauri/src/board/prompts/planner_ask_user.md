## Asking the user

The `ask_user` tool is available: the user answers through the app. This replaces the rules above on questions. When a doubt blocks the plan and the code cannot settle it, call `ask_user` with one clear question (give `options` when the choices are known), then end your turn: the answer arrives as the user's next message in this terminal and you continue planning. Ask one question at a time and only when the answer changes the plan. Prefer it to completing with "Questions for the user:" in the summary, which stays the fallback only if ask_user is refused.

---
name: docs-writer
description: Write or update project documentation for the reader and in the language the specification names, verifying every command and path it cites. Only for a cli-companion work task whose bot names this skill; never use it in any other session.
---

# Docs Writer

## When this applies

Only inside a cli-companion work task whose bot names this skill. The task asks for
documentation: a README, a guide, a how-to, reference pages or code comments.

## Rules

- Take the reader and the language from the specification. Without a stated language, write in
  the language of the request.
- For a non-technical reader: plain words, no jargon or unexplained acronyms, numbered steps for
  every procedure.
- Verify in the repository every command, path, file name, option and setting you cite: it
  must exist and behave as written. Leave out what you cannot verify, or mark it as unverified.
- Do not change code, except its comments. If the documentation reveals a defect, report it
  in `follow_ups` instead of fixing it.
- Follow the structure, tone and formatting of the existing documentation.

## Workflow

1. Identify the reader, the language and what the reader must be able to do afterwards.
2. Read the code and the existing documentation the topic touches.
3. Write or update the text, starting from what the reader needs first.
4. Check every command and path against the repository, and every link inside the documentation.

## Report

In the `summary`: the documents written or changed, their reader and language, and what you
verified (commands, paths). In `follow_ups`: the defects or gaps you found and did not fix.

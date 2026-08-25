# Security

## Reporting a vulnerability

Please report security problems **privately** — not as a public issue.

Use GitHub's private reporting form:
**[Report a vulnerability](https://github.com/zahidhossin39/Silent-Voice/security/advisories/new)**

That opens a private channel visible only to the maintainer.

Please include what the problem is, how to reproduce it, and what an attacker
could do with it. You will get a first response within a week.

Please do not post details publicly until a fix has shipped.

## What is in scope

Silent Voice runs entirely on your machine, so the interesting areas are:

- code execution through a downloaded model, or through the frontend
- the auto-updater accepting a release it should not
- exposure of `settings.json`, which holds API keys in plain text
- the inline proofreader reading text it should not (for example, password
  fields)

## What is not

- API keys being readable by someone who already has access to your user
  account — that is expected, and noted in [PRIVACY.md](PRIVACY.md)
- anything requiring physical access to an unlocked machine

## How releases are protected

Every release is cryptographically signed, and the updater refuses any build
whose signature does not verify. The build pipeline fails rather than publish
an unsigned release.

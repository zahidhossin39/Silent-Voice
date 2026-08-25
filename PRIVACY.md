# Privacy

Silent Voice is local-first. This page says exactly what that means, and where
it stops. Everything here is checkable in the source.

## Your voice never leaves your computer

Audio is recorded to disk, transcribed by a model running on your own machine,
and stays there. It is never uploaded, and there is no account to sign in to.

Everything lives in two folders you control:

```
%APPDATA%\SilentVoice\
├── models/       downloaded speech models
├── llm/          downloaded language models
├── tts/          downloaded voices
├── audio/        recordings kept with each transcript
├── history.json  your transcripts
└── logs/         diagnostic logs

%APPDATA%\app.silentvoice.desktop\
└── settings.json your settings, including any API keys
```

Delete those two folders and nothing of yours remains.

## There is no tracking

No analytics, no telemetry, no crash reporting, no usage statistics, no unique
identifier. Nothing reports back that you launched the app, dictated anything,
or which features you use.

## The only two servers it contacts

| Server | When | What it sends |
| --- | --- | --- |
| `huggingface.co` | You download a model | The name of the file you asked for. No account, no key. |
| `github.com` | Checking for updates | Nothing but the request itself. It reads a small file listing the newest version. |

Both are ordinary downloads. As with any download, the server sees your IP
address — that is unavoidable for any app that fetches a file, and neither
request carries anything identifying you.

## Cloud AI is off unless you turn it on

Silent Voice can optionally send text to a cloud provider (OpenAI, OpenRouter,
Anthropic, or anything compatible) to clean up your dictation. **This is off by
default and does nothing until you add your own API key.**

If you switch it on, then and only then:

- the transcribed **text** is sent to the provider *you* chose, at the address
  *you* entered
- your **audio** is still not sent, unless you also explicitly pick a cloud
  speech model
- what that provider does with it is governed by their privacy policy, not this
  one

Remove the key and it stops immediately.

## Two things to be aware of

- **API keys are stored in plain text** in `settings.json`, like most desktop
  apps. Anyone with access to your user account can read them. Treat that file
  as a secret.
- **Your transcripts and recordings are stored unencrypted**, so they are as
  private as your computer is.

## Inline proofreading

The spell and grammar checking that underlines words in other applications
reads the text of the field you are typing in, using Windows' built-in
accessibility system. That text is checked **on your machine**, is never sent
anywhere, and is not stored.

"""Custom-vocabulary benchmark for Parakeet hotwords.

Synthesizes spoken sentences with the installed Kokoro voice, runs them through
the same Parakeet model the app uses under several hotword configs, and reports:
  - WER on sentences that contain NO vocab word (does biasing hurt normal speech?)
  - false insertions: vocab words that appear in the output but weren't spoken
  - recall: vocab words that were spoken and came out spelled right
  - speed

Run:  python scripts/vocab_bench.py
Needs: pip install sherpa-onnx numpy  (and the app's Parakeet + Kokoro models)
"""
import os, re, sys, time, json, hashlib
import numpy as np
import sherpa_onnx

ROOT = os.path.join(os.environ["APPDATA"], "SilentVoice")
STT = os.path.join(ROOT, "models", "parakeet-tdt-0.6b-v2")
TTS = os.path.join(ROOT, "tts", "kokoro-en-v0_19")
CACHE = os.path.join(os.path.dirname(__file__), ".vocab_bench_cache")

# The user's real list (duplicates included, as typed).
VOCAB = ["Claude", "discord", "task", "Anti-gravity", "n8n", "Claude Code", "Gemini", "task", "ask"]

# (spoken text for TTS, reference transcript). Spoken differs where TTS would
# mispronounce the written form (n8n is said "n eight n").
TARGETS = [
    ("I asked Claude to fix the bug.", "I asked Claude to fix the bug."),
    ("Open Claude Code in the terminal.", "Open Claude Code in the terminal."),
    ("Gemini wrote the first draft.", "Gemini wrote the first draft."),
    ("Let Gemini and Claude compare their answers.", "Let Gemini and Claude compare their answers."),
    ("I built the workflow in n eight n yesterday.", "I built the workflow in n8n yesterday."),
    ("The n eight n server keeps crashing.", "The n8n server keeps crashing."),
    ("Anti-gravity is the new editor from Google.", "Anti-gravity is the new editor from Google."),
    ("Send the link on Discord please.", "Send the link on discord please."),
    ("Claude Code finished the refactor in ten minutes.", "Claude Code finished the refactor in ten minutes."),
    ("Ask Gemini to research the topic first.", "Ask Gemini to research the topic first."),
    ("My next task is to update the app.", "My next task is to update the app."),
    ("Can you ask Claude about the error?", "Can you ask Claude about the error?"),
]
CONTROLS = [
    "I could not find the file anywhere.",
    "The cloud looks dark today.",
    "We should probably leave before it rains.",
    "Can you send me the report by Friday?",
    "The meeting went better than expected.",
    "I think the design needs a little more space.",
    "Please turn off the lights when you leave.",
    "He said the code was ready for review.",
    "The gravity of the situation was clear.",
    "She asked a question about the budget.",
    "My desk is covered in papers.",
    "Could you call me back in an hour?",
    "The new version loads much faster now.",
    "I want to go for a walk after lunch.",
    "We need to buy milk, eggs and bread.",
    "The cat jumped over the fence.",
    "Let me know if you have any questions.",
    "This feature should be easy to build.",
    "The train was twenty minutes late.",
    "I forgot my password again.",
    "The weather is nice this weekend.",
    "Our team shipped three updates this month.",
    "Turn left at the next street.",
    "The sound quality is much better with this microphone.",
]

def norm(s):
    return re.sub(r"[^a-z0-9' ]+", " ", s.lower().replace("-", " ")).split()

def wer(ref, hyp):
    r, h = norm(ref), norm(hyp)
    d = list(range(len(h) + 1))
    for i in range(1, len(r) + 1):
        prev, d[0] = d[0], i
        for j in range(1, len(h) + 1):
            cur = min(d[j] + 1, d[j - 1] + 1, prev + (r[i - 1] != h[j - 1]))
            prev, d[j] = d[j], cur
    return d[len(h)], len(r)

def vocab_counts(text, vocab):
    t = " " + " ".join(norm(text)) + " "
    return {v: t.count(" " + " ".join(norm(v)) + " ") for v in set(vocab)}

def synth(sentences):
    os.makedirs(CACHE, exist_ok=True)
    tts = None
    out = []
    for i, text in enumerate(sentences):
        for sid in (0, 3, 6):  # three different voices
            p = os.path.join(CACHE, f"{hashlib.md5(f'{sid}|{text}'.encode()).hexdigest()[:16]}.npy")
            if not os.path.exists(p):
                if tts is None:
                    tts = sherpa_onnx.OfflineTts(sherpa_onnx.OfflineTtsConfig(
                        model=sherpa_onnx.OfflineTtsModelConfig(kokoro=sherpa_onnx.OfflineTtsKokoroModelConfig(
                            model=os.path.join(TTS, "model.onnx"), voices=os.path.join(TTS, "voices.bin"),
                            tokens=os.path.join(TTS, "tokens.txt"), data_dir=os.path.join(TTS, "espeak-ng-data")),
                            num_threads=4)))
                a = tts.generate(text, sid=sid, speed=1.0)
                np.save(p, np.array([a.sample_rate] + list(a.samples), dtype=np.float32))
            arr = np.load(p)
            out.append((int(arr[0]), arr[1:]))
    return out

def recognizer(hotwords, score):
    kw = dict(
        encoder=os.path.join(STT, "encoder.int8.onnx"), decoder=os.path.join(STT, "decoder.int8.onnx"),
        joiner=os.path.join(STT, "joiner.int8.onnx"), tokens=os.path.join(STT, "tokens.txt"),
        num_threads=4, model_type="nemo_transducer",
    )
    if hotwords is None:
        return sherpa_onnx.OfflineRecognizer.from_transducer(decoding_method="greedy_search", **kw)
    hw = os.path.join(CACHE, "hotwords.txt")
    with open(hw, "w", encoding="utf-8") as f:
        f.write("\n".join(hotwords) + "\n")
    return sherpa_onnx.OfflineRecognizer.from_transducer(
        decoding_method="modified_beam_search", max_active_paths=4,
        hotwords_file=hw if hotwords else "", hotwords_score=score,
        modeling_unit="bpe" if hotwords else "", bpe_vocab=os.path.join(STT, "bpe.vocab") if hotwords else "",
        **kw)

def run(name, rec, items, vocab):
    errs = words = ins = hits = spoken = 0
    ctrl_errs = ctrl_words = 0
    t0 = time.perf_counter()
    bad = []
    for (sr, samples), ref, is_target in items:
        s = rec.create_stream()
        s.accept_waveform(sr, samples)
        rec.decode_stream(s)
        hyp = s.result.text
        e, n = wer(ref, hyp)
        errs += e; words += n
        if not is_target:
            ctrl_errs += e; ctrl_words += n
        rc, hc = vocab_counts(ref, vocab), vocab_counts(hyp, vocab)
        for v in rc:
            spoken += rc[v]
            hits += min(rc[v], hc[v])
            if hc[v] > rc[v]:
                ins += hc[v] - rc[v]
                bad.append(f"    + {v!r}: {hyp}")
    dt = time.perf_counter() - t0
    row = dict(name=name, wer=round(100 * errs / words, 1), ctrl_wer=round(100 * ctrl_errs / ctrl_words, 1),
               insertions=ins, recall=f"{hits}/{spoken}", secs=round(dt, 1))
    print(f"{name:34} WER {row['wer']:5}%  normal-speech WER {row['ctrl_wer']:5}%  "
          f"false inserts {ins:3}  vocab recall {hits}/{spoken}  {dt:5.1f}s")
    for b in bad[:6]:
        print(b)
    return row

def main():
    vocab = [v for v in dict.fromkeys(VOCAB)]
    texts = [t for t, _ in TARGETS] + CONTROLS
    refs = [r for _, r in TARGETS] + CONTROLS
    audio = synth(texts)
    items = [(audio[i * 3 + k], refs[i], i < len(TARGETS)) for i in range(len(texts)) for k in range(3)]

    configs = [("greedy (no vocab)", None, 0)]
    filtered = sys.argv[1].split("|") if len(sys.argv) > 1 else None
    configs += [("beam + all words @2.0 (current)", vocab, 2.0)]
    if filtered:
        for sc in (1.0, 1.5, 2.0, 3.0):
            configs.append((f"beam + filtered @{sc}", filtered, sc))
    rows = [run(n, recognizer(h, s), items, vocab) for n, h, s in configs]
    json.dump(rows, open(os.path.join(CACHE, "last.json"), "w"), indent=1)

if __name__ == "__main__":
    main()

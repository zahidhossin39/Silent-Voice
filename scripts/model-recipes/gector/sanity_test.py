import numpy as np, onnxruntime as ort
from tokenizers import Tokenizer

d = r"D:\Vibe-coding\Silent voice\gector-work"
tok = Tokenizer.from_file(d + r"\tokenizer.json")
labels = open(d + r"\labels.txt", encoding="utf-8").read().splitlines()
sess = ort.InferenceSession(d + r"\gector-int8.onnx", providers=["CPUExecutionProvider"])

def check(sentence):
    words = ["$START"] + sentence.split()
    enc = tok.encode("$START " + sentence)
    ids, wids = enc.ids, enc.word_ids
    first = {}
    for i, w in enumerate(wids):
        if w is not None and w not in first:
            first[w] = i
    lg, dg = sess.run(None, {
        "input_ids": np.array([ids], dtype=np.int64),
        "attention_mask": np.ones((1, len(ids)), dtype=np.int64)})
    out = []
    keep_id = labels.index("$KEEP")
    for w_idx, word in enumerate(words):
        if w_idx not in first: continue
        logits = lg[0, first[w_idx]]
        p = np.exp(logits - logits.max()); p /= p.sum()
        p[keep_id] += 0.2
        best = int(p.argmax())
        if labels[best] != "$KEEP" and p[best] >= 0.5:
            out.append((word, labels[best], float(p[best])))
    return out

import time
for s in ["We should do it step by stet .",
          "What are you guy doing ?",
          "This sentence is perfectly fine .",
          "He go to school every day ."]:
    t = time.time()
    r = check(s)
    print(f"{(time.time()-t)*1000:.0f}ms | {s} -> {r}")

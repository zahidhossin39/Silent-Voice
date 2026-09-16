import numpy as np, onnxruntime as ort
from tokenizers import Tokenizer

d = r"D:\Vibe-coding\Silent voice\gector-work"
tok = Tokenizer.from_file(d + r"\tokenizer.json")
labels = open(d + r"\labels.txt", encoding="utf-8").read().splitlines()
sess = ort.InferenceSession(d + r"\gector-int8.onnx", providers=["CPUExecutionProvider"])

s = "He go to school every day ."
enc = tok.encode("$START " + s)
print("tokens:", enc.tokens)
print("ids:", enc.ids)
print("word_ids:", enc.word_ids)

lg, dg = sess.run(None, {
    "input_ids": np.array([enc.ids], dtype=np.int64),
    "attention_mask": np.ones((1, len(enc.ids)), dtype=np.int64)})
print("label_logits shape:", lg.shape)

words = ["$START"] + s.split()
first = {}
for i, w in enumerate(enc.word_ids):
    if w is not None and w not in first:
        first[w] = i
for w_idx, word in enumerate(words):
    if w_idx not in first: continue
    logits = lg[0, first[w_idx]]
    p = np.exp(logits - logits.max()); p /= p.sum()
    top = np.argsort(p)[::-1][:3]
    print(f"{word:10s} pos={first[w_idx]:2d} " + "  ".join(f"{labels[t]}={p[t]:.3f}" for t in top))

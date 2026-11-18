"""Fetch only the SentencePiece tokenizer out of a remote .nemo (plain tar) with
HTTP range requests, then write bpe.vocab (piece<TAB>score) like
scripts/generate_parakeet_bpe_vocab.py does.
Usage: python -I scripts/fetch_nemo_bpe_vocab.py <nemo url> <out dir>
  e.g. https://huggingface.co/nvidia/parakeet-tdt_ctc-110m/resolve/main/parakeet-tdt_ctc-110m.nemo
"""
import sys, os, urllib.request

url, out = sys.argv[1], sys.argv[2]
os.makedirs(out, exist_ok=True)

def get(start, n):
    req = urllib.request.Request(url, headers={"Range": f"bytes={start}-{start + n - 1}", "User-Agent": "sv-tokenizer-fetch"})
    with urllib.request.urlopen(req) as r:
        assert r.status == 206, r.status
        return r.read()

pos, found = 0, []
while True:
    h = get(pos, 512)
    if h == b"\0" * 512 or not h.strip(b"\0"):
        break
    name = h[0:100].rstrip(b"\0").decode()
    prefix = h[345:500].rstrip(b"\0").decode()
    if prefix:
        name = prefix + "/" + name
    size = int(h[124:136].rstrip(b"\0 ").decode() or "0", 8)
    kind = h[156:157]
    print(f"{name}  {size} bytes  type={kind!r}")
    if name.endswith(".model") and kind in (b"0", b"\0"):
        data = get(pos + 512, size)
        p = os.path.join(out, os.path.basename(name))
        open(p, "wb").write(data)
        found.append(p)
    pos += 512 + (size + 511) // 512 * 512

import sentencepiece as spm
for p in found:
    try:
        sp = spm.SentencePieceProcessor(model_file=p)
    except Exception as e:
        print("not a tokenizer:", p, e); continue
    with open(os.path.join(out, "bpe.vocab"), "w", encoding="utf-8") as f:
        for i in range(sp.get_piece_size()):
            f.write(f"{sp.id_to_piece(i)}\t{sp.get_score(i)}\n")
    print("wrote bpe.vocab from", p, sp.get_piece_size(), "pieces")
    break

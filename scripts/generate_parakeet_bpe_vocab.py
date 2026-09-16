#!/usr/bin/env python
# Generate `bpe.vocab` for a NeMo Parakeet model, for sherpa-onnx hotwords.
#
# WHY: sherpa-onnx contextual biasing (hotwords) on a transducer needs a
# `bpe.vocab` (SentencePiece pieces + merge scores) to tokenize the hotwords
# into the model's sub-words. k2-fsa's exported ONNX archive ships only
# tokens.txt (pieces + ids, NO scores), and scores can't be recovered from it,
# so the vocab must come from NVIDIA's original tokenizer.
#
# The official sherpa script (scripts/nemo/generate_bpe_vocab.py) loads the
# whole NeMo toolkit (torch + nemo_toolkit, multi-GB). We don't need it: a
# .nemo is just a tar, and its tokenizer IS a SentencePiece .model. So we
# extract that one file and emit the exact same `piece<TAB>score` format
# sherpa's ssentencepiece parser reads.
#
# Usage:
#   python scripts/generate_parakeet_bpe_vocab.py \
#       --nemo parakeet-tdt-0.6b-v2.nemo \
#       --output src-tauri/resources/parakeet-tdt-0.6b-v2.bpe.vocab
#
# Needs only `sentencepiece` (pip install sentencepiece). Run once per model;
# the output is tiny (~tens of KB) and committed / embedded via include_bytes!.

import argparse
import tarfile
import tempfile
from pathlib import Path

import sentencepiece as spm


def extract_tokenizer_model(nemo_path: str, dest_dir: str) -> str:
    """Pull the SentencePiece tokenizer .model out of a .nemo tar.

    The ASR weights are `model_weights.ckpt`; the only loadable SentencePiece
    model is the tokenizer. We extract every `*.model` member and return the
    first one SentencePiece accepts.
    """
    candidates = []
    with tarfile.open(nemo_path, "r:*") as tar:
        for member in tar.getmembers():
            if member.isfile() and member.name.endswith(".model"):
                # Flatten any leading "./" or nested dirs to a safe basename.
                out = Path(dest_dir) / Path(member.name).name
                with tar.extractfile(member) as src, open(out, "wb") as dst:
                    dst.write(src.read())
                candidates.append(str(out))

    for path in candidates:
        try:
            spm.SentencePieceProcessor(model_file=path)
            return path
        except Exception:
            continue
    raise SystemExit(
        f"no loadable SentencePiece tokenizer found in {nemo_path} "
        f"(found .model members: {candidates})"
    )


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--nemo", required=True, help="path to the .nemo file")
    ap.add_argument("--output", required=True, help="path to write bpe.vocab")
    args = ap.parse_args()

    with tempfile.TemporaryDirectory() as tmp:
        model_file = extract_tokenizer_model(args.nemo, tmp)
        sp = spm.SentencePieceProcessor(model_file=model_file)
        vocab_size = sp.get_piece_size()
        Path(args.output).parent.mkdir(parents=True, exist_ok=True)
        with open(args.output, "w", encoding="utf-8") as f:
            for i in range(vocab_size):
                # Exact format sherpa's ssentencepiece reads: piece<TAB>score.
                f.write(f"{sp.id_to_piece(i)}\t{sp.get_score(i)}\n")
        print(f"wrote {vocab_size} pieces to {args.output}")


if __name__ == "__main__":
    main()

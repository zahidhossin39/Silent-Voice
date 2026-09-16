import onnx, os, time
from onnxruntime.quantization import quantize_dynamic, QuantType

D = os.path.join(os.path.dirname(__file__), "coedit_onnx")

def io_names(path):
    m = onnx.load(path, load_external_data=False)
    ins = [(i.name, [d.dim_value if (d.dim_value) else d.dim_param for d in i.type.tensor_type.shape.dim]) for i in m.graph.input]
    outs = [(o.name, [d.dim_value if (d.dim_value) else d.dim_param for d in o.type.tensor_type.shape.dim]) for o in m.graph.output]
    return ins, outs

for name in ["encoder_model.onnx", "decoder_model.onnx"]:
    ins, outs = io_names(os.path.join(D, name))
    print(f"\n=== {name} ===")
    print("INPUTS:")
    for n, s in ins: print(f"   {n}  {s}")
    print("OUTPUTS:")
    for n, s in outs: print(f"   {n}  {s}")

print("\n=== quantizing (int8 dynamic) ===", flush=True)
for name in ["encoder_model.onnx", "decoder_model.onnx"]:
    src = os.path.join(D, name)
    dst = os.path.join(D, name.replace(".onnx", "_int8.onnx"))
    t = time.time()
    quantize_dynamic(src, dst, weight_type=QuantType.QInt8)
    mb = os.path.getsize(dst) / 1e6
    print(f"   {os.path.basename(dst)}: {mb:.0f} MB  ({time.time()-t:.0f}s)", flush=True)

print("\n=== verify (int8) ===", flush=True)
from optimum.onnxruntime import ORTModelForSeq2SeqLM
from transformers import AutoTokenizer
tok = AutoTokenizer.from_pretrained(D)
model = ORTModelForSeq2SeqLM.from_pretrained(
    D, encoder_file_name="encoder_model_int8.onnx",
    decoder_file_name="decoder_model_int8.onnx", use_cache=False)
tests = [
    "Fix grammatical errors in this sentence: When I grows up, I wants to be a doctor.",
    "Fix the grammar: She dont has no money for the bus yesterday.",
    "Fix grammatical errors in this sentence: their going to they're house over there.",
    "Fix grammatical errors in this sentence: i think that the meeting is on tuesday and we should discuss the new project",
]
for t in tests:
    ids = tok(t, return_tensors="pt")
    out = model.generate(**ids, max_new_tokens=128, num_beams=1)
    print(f"IN : {t}")
    print(f"OUT: {tok.decode(out[0], skip_special_tokens=True)}\n", flush=True)

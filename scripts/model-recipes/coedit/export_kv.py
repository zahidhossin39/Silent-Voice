# Re-export CoEdiT-large WITH KV-cache (merged decoder), then int8-quantize.
# The original export used use_cache=False -> no past_key_values -> O(n^2) decode.
import os, time, onnx
from optimum.onnxruntime import ORTModelForSeq2SeqLM
from onnxruntime.quantization import quantize_dynamic, QuantType

OUT = os.path.join(os.path.dirname(__file__), "coedit_kv")
os.makedirs(OUT, exist_ok=True)

t = time.time()
print("=== exporting grammarly/coedit-large with cache (this downloads ~3GB) ===", flush=True)
m = ORTModelForSeq2SeqLM.from_pretrained("grammarly/coedit-large", export=True, use_cache=True, use_merged=True)
m.save_pretrained(OUT)
print(f"exported in {time.time()-t:.0f}s", flush=True)

print("\n=== files ===", flush=True)
for f in sorted(os.listdir(OUT)):
    p = os.path.join(OUT, f)
    if os.path.isfile(p):
        print(f"  {f}  ({os.path.getsize(p)/1e6:.0f} MB)")

def io_names(path):
    g = onnx.load(path, load_external_data=False).graph
    ins = [(i.name, [d.dim_value or d.dim_param for d in i.type.tensor_type.shape.dim]) for i in g.input]
    outs = [(o.name, [d.dim_value or d.dim_param for d in o.type.tensor_type.shape.dim]) for o in g.output]
    return ins, outs

for name in ["encoder_model.onnx", "decoder_model_merged.onnx"]:
    p = os.path.join(OUT, name)
    if not os.path.exists(p):
        continue
    ins, outs = io_names(p)
    print(f"\n=== {name} IO ===")
    print("INPUTS:")
    for n, s in ins:
        print(f"   {n}  {s}")
    print("OUTPUTS:")
    for n, s in outs:
        print(f"   {n}  {s}")

print("\n=== quantizing int8 ===", flush=True)
for name in ["encoder_model.onnx", "decoder_model_merged.onnx"]:
    src = os.path.join(OUT, name)
    if not os.path.exists(src):
        continue
    dst = os.path.join(OUT, name.replace(".onnx", "_int8.onnx"))
    big = os.path.getsize(src) >= 2 * 1024**3
    quantize_dynamic(src, dst, weight_type=QuantType.QInt8, use_external_data_format=big)
    print(f"   {os.path.basename(dst)}: {os.path.getsize(dst)/1e6:.0f} MB", flush=True)

print("\nDONE", flush=True)

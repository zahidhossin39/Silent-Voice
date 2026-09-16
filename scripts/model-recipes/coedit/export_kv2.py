# Re-export UNMERGED (use_merged=False): two flat decoder graphs that quantize
# properly (the merged decoder hid weights in an If-subgraph -> int8 no-op).
import os, time, onnx
from optimum.onnxruntime import ORTModelForSeq2SeqLM
from onnxruntime.quantization import quantize_dynamic, QuantType

OUT = os.path.join(os.path.dirname(__file__), "coedit_kv2")
os.makedirs(OUT, exist_ok=True)

t = time.time()
print("=== exporting unmerged (cached weights, no re-download) ===", flush=True)
m = ORTModelForSeq2SeqLM.from_pretrained("grammarly/coedit-large", export=True, use_cache=True, use_merged=False)
m.save_pretrained(OUT)
print(f"exported in {time.time()-t:.0f}s", flush=True)

print("\n=== files ===", flush=True)
for f in sorted(os.listdir(OUT)):
    p = os.path.join(OUT, f)
    if os.path.isfile(p):
        print(f"  {f}  ({os.path.getsize(p)/1e6:.0f} MB)")

targets = ["encoder_model.onnx", "decoder_model.onnx", "decoder_with_past_model.onnx"]
print("\n=== quantizing int8 ===", flush=True)
for name in targets:
    src = os.path.join(OUT, name)
    if not os.path.exists(src):
        print(f"  MISSING {name}"); continue
    dst = os.path.join(OUT, name.replace(".onnx", "_int8.onnx"))
    quantize_dynamic(src, dst, weight_type=QuantType.QInt8)
    print(f"   {os.path.basename(dst)}: {os.path.getsize(dst)/1e6:.0f} MB", flush=True)

# IO of the with-past decoder (the one used for steps 1+)
g = onnx.load(os.path.join(OUT, "decoder_with_past_model.onnx"), load_external_data=False).graph
print("\n=== decoder_with_past_model.onnx ===")
print("INPUTS:", [i.name for i in g.input][:6], "...", len(g.input), "total")
print("OUTPUTS:", [o.name for o in g.output][:4], "...", len(g.output), "total")
g2 = onnx.load(os.path.join(OUT, "decoder_model.onnx"), load_external_data=False).graph
print("=== decoder_model.onnx (step 0) ===")
print("INPUTS:", [i.name for i in g2.input])
print("OUTPUTS:", [o.name for o in g2.output][:4], "...", len(g2.output), "total")
print("\nDONE", flush=True)

import os
from onnxruntime.quantization import quantize_dynamic, QuantType

def main():
    model_dir = "coedit_onnx"
    onnx_models = ["encoder_model.onnx", "decoder_model.onnx", "decoder_with_past_model.onnx", "decoder_model_merged.onnx"]

    for model_name in onnx_models:
        model_input = os.path.join(model_dir, model_name)
        if not os.path.exists(model_input):
            continue
            
        model_output = os.path.join(model_dir, model_name.replace(".onnx", "_int8.onnx"))
        
        print(f"Quantizing {model_input} to {model_output}...")
        
        has_external_data = os.path.exists(model_input + "_data") or os.path.exists(model_input.replace(".onnx", ".onnx_data"))
        if os.path.getsize(model_input) >= 2 * 1024 * 1024 * 1024:
            has_external_data = True
            
        try:
            quantize_dynamic(
                model_input=model_input,
                model_output=model_output,
                weight_type=QuantType.QInt8,
                use_external_data_format=has_external_data
            )
            print(f"Done quantizing {model_input}.")
        except Exception as e:
            print(f"Failed to quantize {model_input}: {e}")

if __name__ == "__main__":
    main()

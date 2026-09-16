import os
from transformers import AutoTokenizer
from optimum.onnxruntime import ORTModelForSeq2SeqLM

def main():
    model_dir = "coedit_onnx"

    print("Loading tokenizer...")
    tokenizer = AutoTokenizer.from_pretrained(model_dir)

    print("Loading INT8 model...")
    # Check which decoder int8 file exists
    if os.path.exists(os.path.join(model_dir, "decoder_model_merged_int8.onnx")):
        model = ORTModelForSeq2SeqLM.from_pretrained(
            model_dir,
            encoder_file_name="encoder_model_int8.onnx",
            decoder_file_name="decoder_model_merged_int8.onnx"
        )
    else:
        model = ORTModelForSeq2SeqLM.from_pretrained(
            model_dir,
            encoder_file_name="encoder_model_int8.onnx",
            decoder_file_name="decoder_model_int8.onnx",
            decoder_with_past_file_name="decoder_with_past_model_int8.onnx"
        )

    texts = [
        "Fix grammatical errors in this sentence: When I grows up, I wants to be a doctor.",
        "Fix the grammar: She dont has no money for the bus yesterday.",
        "Fix grammatical errors in this sentence: their going to they're house over there."
    ]

    for text in texts:
        inputs = tokenizer(text, return_tensors="pt")
        outputs = model.generate(**inputs, num_beams=1, max_new_tokens=128)
        corrected = tokenizer.decode(outputs[0], skip_special_tokens=True)
        print(f"INPUT:  {text}")
        print(f"OUTPUT: {corrected}\n")

if __name__ == "__main__":
    main()

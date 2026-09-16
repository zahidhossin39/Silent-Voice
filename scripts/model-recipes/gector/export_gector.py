import os
import json
import torch
import torch.nn as nn
from transformers import AutoConfig, RobertaModel, AutoTokenizer
from huggingface_hub import snapshot_download
from onnxruntime.quantization import quantize_dynamic, QuantType

class GECToROnnxWrapper(nn.Module):
    def __init__(self, config_dict):
        super().__init__()
        # Load the base model configuration
        roberta_config = AutoConfig.from_pretrained(config_dict["model_id"])
        
        # The bert model vocab size must include the $START token (+1)
        # Note: gotutiyan/gector resizes token embeddings with vocab_size + 1
        roberta_config.vocab_size += 1
        
        # If config says has_add_pooling_layer=True, the wrapped model has add_pooling_layer=False!
        has_add_pooling = config_dict.get("has_add_pooling_layer", False)
        self.bert = RobertaModel(roberta_config, add_pooling_layer=not has_add_pooling)
        
        n_labels = config_dict.get("n_labels", 5002)
        n_d_labels = config_dict.get("n_d_labels", 3)
        
        # The linear heads subtract 1 for the <PAD> token
        self.label_proj_layer = nn.Linear(roberta_config.hidden_size, n_labels - 1)
        self.d_proj_layer = nn.Linear(roberta_config.hidden_size, n_d_labels - 1)

    def forward(self, input_ids, attention_mask):
        outputs = self.bert(input_ids, attention_mask=attention_mask)
        sequence_output = outputs[0]  # last_hidden_state
        
        logits_labels = self.label_proj_layer(sequence_output)
        logits_d = self.d_proj_layer(sequence_output)
        
        return logits_labels, logits_d

def extract_labels(config_dict, key_prefix, n_labels):
    id2label = config_dict.get(f"{key_prefix}id2label", {})
    label2id = config_dict.get(f"{key_prefix}label2id", {})
    
    labels = [""] * n_labels
    
    for k, v in label2id.items():
        if isinstance(v, int) and v < n_labels:
            labels[v] = k
        elif isinstance(k, str) and k.isdigit() and isinstance(v, str):
            idx = int(k)
            if idx < n_labels:
                labels[idx] = v
                
    for k, v in id2label.items():
        if isinstance(k, str) and k.isdigit():
            idx = int(k)
            if idx < n_labels:
                labels[idx] = v
                
    return labels

def main():
    repo_id = "gotutiyan/gector-roberta-base-5k"
    export_dir = os.path.dirname(os.path.abspath(__file__))
    os.makedirs(export_dir, exist_ok=True)
    
    print(f"Downloading {repo_id}...")
    snapshot_path = snapshot_download(repo_id)
    
    with open(os.path.join(snapshot_path, "config.json"), "r", encoding="utf-8") as f:
        config_dict = json.load(f)
        
    print("Building model...")
    model = GECToROnnxWrapper(config_dict)
    
    safetensors_path = os.path.join(snapshot_path, "model.safetensors")
    bin_path = os.path.join(snapshot_path, "pytorch_model.bin")
    
    print("Loading weights...")
    if os.path.exists(safetensors_path):
        from safetensors.torch import load_file
        state_dict = load_file(safetensors_path)
    else:
        state_dict = torch.load(bin_path, map_location="cpu")
        
    # Using strict=False to catch and print mismatches without crashing
    missing, unexpected = model.load_state_dict(state_dict, strict=False)
    
    if not missing and not unexpected:
        print("Weights loaded perfectly (strict match).")
    else:
        print("Missing keys:", missing)
        print("Unexpected keys:", unexpected)
        
        # If there are unexpectedly missing keys that are actually crucial, this will warn the user.
        # Often HF models have extra stuff like loss_fn in buffers, which would show up in unexpected.
    
    model.eval()
    
    onnx_path = os.path.join(export_dir, "gector.onnx")
    onnx_int8_path = os.path.join(export_dir, "gector-int8.onnx")
    
    print(f"Exporting ONNX to {onnx_path}...")
    dummy_input_ids = torch.zeros(1, 16, dtype=torch.long)
    dummy_attention_mask = torch.ones(1, 16, dtype=torch.long)
    
    torch.onnx.export(
        model,
        (dummy_input_ids, dummy_attention_mask),
        onnx_path,
        export_params=True,
        opset_version=17,
        do_constant_folding=True,
        input_names=["input_ids", "attention_mask"],
        output_names=["label_logits", "detect_logits"],
        dynamic_axes={
            "input_ids": {0: "batch_size", 1: "sequence_length"},
            "attention_mask": {0: "batch_size", 1: "sequence_length"},
            "label_logits": {0: "batch_size", 1: "sequence_length"},
            "detect_logits": {0: "batch_size", 1: "sequence_length"}
        }
    )
    
    print(f"Quantizing to INT8: {onnx_int8_path}...")
    quantize_dynamic(
        model_input=onnx_path,
        model_output=onnx_int8_path,
        weight_type=QuantType.QUInt8
    )
    
    print("Saving tokenizer and vocab...")
    tokenizer = AutoTokenizer.from_pretrained(snapshot_path)
    tokenizer.save_pretrained(export_dir)
    
    num_labels = config_dict.get("n_labels", 5002) - 1
    num_d_labels = config_dict.get("n_d_labels", 3) - 1
    
    labels = extract_labels(config_dict, "", num_labels)
    d_labels = extract_labels(config_dict, "d_", num_d_labels)
    
    with open(os.path.join(export_dir, "labels.txt"), "w", encoding="utf-8") as f:
        for lbl in labels:
            f.write(f"{lbl}\n")
            
    with open(os.path.join(export_dir, "dtags.txt"), "w", encoding="utf-8") as f:
        for lbl in d_labels:
            f.write(f"{lbl}\n")
            
    print("\n--- File Sizes ---")
    for fname in ["gector.onnx", "gector-int8.onnx", "tokenizer.json", "labels.txt", "dtags.txt"]:
        fpath = os.path.join(export_dir, fname)
        if os.path.exists(fpath):
            size_mb = os.path.getsize(fpath) / (1024 * 1024)
            print(f"{fname}: {size_mb:.2f} MB")

if __name__ == "__main__":
    main()

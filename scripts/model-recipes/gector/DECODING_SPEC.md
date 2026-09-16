# GECToR Tag Decoding Specification (Rust Implementation)

## 1. Preprocessing
GECToR frames Grammatical Error Correction as a sequence tagging task.
- **$START Token**: A special token `$START` must be prepended to the word list before processing. This allows the model to predict insertions at the very beginning of the sentence.
- **Word-Level Tagging**: GECToR operates at the word level. When a word is split into multiple subtokens by the tokenizer (e.g., RoBERTa BPE), only the **FIRST subtoken** of each word receives the tag prediction. The logits for the remaining subtokens of that word are ignored during inference.
- **Special Tokens & CLS Handling**: 
  - For RoBERTa, the sequence is framed as `<s> $START word1 word2 ... </s>`.
  - The model outputs logits for all tokens. We extract the predictions corresponding to the first subtoken of `$START` and each subsequent word, entirely ignoring the `<s>`, `</s>`, and intermediate/trailing subtokens.

## 2. Tag Families & Exact Apply Semantics
GECToR uses a vocabulary of exactly 5,000 tags. Each tag specifies an edit operation on the current word $w_i$.
- **$KEEP**: No change. The word is retained as is.
- **$DELETE**: The word $w_i$ is removed from the sequence.
- **$APPEND_{token}**: The string `{token}` is inserted immediately *after* the current word $w_i$. The word $w_i$ itself is kept. (e.g., sequence becomes `w_i`, `{token}`).
- **$REPLACE_{token}**: The current word $w_i$ is completely replaced by `{token}`.
- **$TRANSFORM_CASE_{CAPITAL, CAPITAL_1, LOWER, UPPER}**:
  - `CAPITAL`: Converts $w_i$ to title case (e.g., "apple" -> "Apple").
  - `CAPITAL_1`: Capitalizes only the first letter, keeping the rest unchanged.
  - `LOWER`: Converts $w_i$ to lowercase (e.g., "Apple" -> "apple").
  - `UPPER`: Converts $w_i$ to uppercase (e.g., "apple" -> "APPLE").
- **$TRANSFORM_AGREEMENT_{SINGULAR, PLURAL}**:
  - `SINGULAR`: Converts a plural noun to singular.
  - `PLURAL`: Converts a singular noun to plural.
  *(Note: This uses a rule-based inflection or dictionary approach under the hood).*
- **$TRANSFORM_VERB_{from_to}** (e.g., `$TRANSFORM_VERB_VB_VBZ`): Transforms the verb $w_i$ from one grammatical form to another. Requires the verb-form dictionary.
- **$MERGE_SPACE**: Merges $w_i$ with the *next* word $w_{i+1}$ by removing the space between them (e.g., "can", "not" -> "cannot"). The edit is functionally equivalent to replacing $w_i$ and $w_{i+1}$ with a single concatenated token.
- **$MERGE_HYPHEN**: Merges $w_i$ with the *next* word $w_{i+1}$ by joining them with a hyphen (e.g., "state", "of" -> "state-of").
- **$TRANSFORM_SPLIT_HYPHEN**: Splits a hyphenated word $w_i$ into two words separated by a space (e.g., "pre-requisite" -> "pre requisite").

## 3. Verb-Form Dictionary
- **URL**: `https://raw.githubusercontent.com/grammarly/gector/master/data/verb-form-vocab.txt`
- **Format**: Each line is of the form `sourceWord_targetWord:fromTag_toTag`.
  Example: `abandon_abandoned:VB_VBD`
- **Usage**: When the model predicts a `$TRANSFORM_VERB_X_Y` tag for word $w_i$, the system looks up the entry where `sourceWord` is $w_i$ and the tag suffix is `X_Y`. The word $w_i$ is then replaced by `targetWord`. If no entry is found, the tag is treated as `$KEEP` (ignored).

## 4. Inference Algorithm
GECToR uses an **iterative correction loop** because some complex corrections require multiple consecutive passes.
1. Tokenize the text into words. Add `$START` to the beginning of the word list.
2. Run model forward pass to get logits.
3. For the `$KEEP` class logit at each word position, add the **confidence bias**.
4. Convert logits to probabilities via softmax. Find the argmax tag (max probability) for each word.
5. If the max probability tag is not `$KEEP`, check if its probability is $\ge$ **min_error_probability**. If it is, accept the edit.
6. Apply all valid edits from left to right to form the new text.
7. If the text has changed, go to step 1. Repeat for a maximum of **5 iterations**. Stop early if no edits are predicted.

**Recommended Values for RoBERTa-base (from GECToR README)**:
- **Confidence bias**: 0.2 (Added directly to the `$KEEP` logit before softmax).
- **Min error prob**: 0.5 (Threshold for non-KEEP tags).

## 5. Mapping Edits to Char Offsets
To produce a UI diff, you must map the word-level predictions back to the original text's character ranges:
1. When initially splitting the text into words, store the `(start_char, end_char)` offsets for each word $w_i$.
2. When applying an edit (e.g., $REPLACE, $APPEND, $TRANSFORM), record the char range `[start_char, end_char]` of $w_i$ and the `replacement_string`.
   - `$DELETE`: range `[start_char, end_char]`, replacement `""`. (Also consider removing trailing/leading space).
   - `$APPEND_X`: range `[end_char, end_char]`, replacement `" X"`.
   - `$MERGE_SPACE`: range `[end_char of w_i, start_char of w_{i+1}]`, replacement `""`.
3. **Iterative Mapping**: Since GECToR runs iteratively, you can either:
   - Calculate offsets over multiple iterations by keeping a cumulative mapping of `(original_start, original_end)`.
   - Or just take the final output text and compute a diff (e.g., using a Myers diff algorithm) against the original string to yield the exact character replacements. The latter is often much simpler and less bug-prone for multi-iteration changes.

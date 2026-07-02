import type { Mode } from "../types";

// Built-in AI processing modes — build plan §8
export const BUILTIN_MODES: Mode[] = [
  {
    id: "raw",
    name: "Raw Transcription",
    icon: "mic",
    system_prompt: "",
    model_source: "none",
    model_id: "",
    builtin: true,
  },
  {
    id: "clean_up",
    name: "Clean Up",
    icon: "sparkles",
    system_prompt:
      "Clean up the following transcribed speech. Remove filler words like 'um', 'uh', 'like', 'you know'. Fix grammar and punctuation. Keep the meaning and tone exactly the same. Output ONLY the cleaned text, nothing else.",
    model_source: "local",
    model_id: "llama-3.2-1b-instruct-q4",
    builtin: true,
  },
  {
    id: "formal",
    name: "Formal",
    icon: "briefcase",
    system_prompt:
      "Rewrite the following transcribed speech in a professional, formal tone. Fix grammar and punctuation. Output ONLY the rewritten text, nothing else.",
    model_source: "local",
    model_id: "llama-3.2-1b-instruct-q4",
    builtin: true,
  },
  {
    id: "casual",
    name: "Casual",
    icon: "smile",
    system_prompt:
      "Rewrite the following transcribed speech in a casual, friendly tone. Keep it natural and conversational. Output ONLY the rewritten text, nothing else.",
    model_source: "local",
    model_id: "llama-3.2-1b-instruct-q4",
    builtin: true,
  },
  {
    id: "email",
    name: "Email",
    icon: "mail",
    system_prompt:
      "Format the following transcribed speech as a clear, well-structured email. Add an appropriate greeting and sign-off if missing. Output ONLY the email, nothing else.",
    model_source: "local",
    model_id: "llama-3.2-1b-instruct-q4",
    builtin: true,
  },
  {
    id: "summary",
    name: "Summary",
    icon: "list",
    system_prompt:
      "Summarize the following transcribed speech into concise bullet points capturing the key information. Output ONLY the bullet points, nothing else.",
    model_source: "local",
    model_id: "llama-3.2-1b-instruct-q4",
    builtin: true,
  },
  {
    id: "translate",
    name: "Translate",
    icon: "globe",
    system_prompt:
      "Translate the following transcribed speech into the target language. Output ONLY the translation, nothing else.",
    model_source: "local",
    model_id: "llama-3.2-1b-instruct-q4",
    builtin: true,
  },
  {
    id: "code_comment",
    name: "Code Comment",
    icon: "code",
    system_prompt:
      "Format the following transcribed speech as a clear code comment. Use concise technical language. Output ONLY the comment text, nothing else.",
    model_source: "local",
    model_id: "llama-3.2-1b-instruct-q4",
    builtin: true,
  },
];

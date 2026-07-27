import type { Mode } from "../types";

export const SAMPLE_INPUT =
  "um so i think we should like push the release to friday you know cause the uh login thing is still broken";

// Built-in AI processing modes — build plan §8
export const BUILTIN_MODES: Mode[] = [
  {
    id: "raw",
    name: "Raw Transcription",
    icon: "mic",
    system_prompt: "",
    description: "Pastes exactly what you said, with no AI rewriting.",
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
    description: "Removes 'um' and 'uh', fixes grammar and punctuation, keeps your wording.",
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
    description: "Rewrites what you said in a professional tone, ready for work.",
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
    description: "Rewrites what you said in a relaxed, conversational tone.",
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
    description: "Shapes what you said into an email, adding a greeting and sign-off.",
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
    description: "Condenses what you said into short bullet points.",
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
    description: "Translates what you said into your target language.",
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
    description: "Turns what you said into a concise technical code comment.",
    model_source: "local",
    model_id: "llama-3.2-1b-instruct-q4",
    builtin: true,
  },
];

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
    id: "flow",
    name: "Whisper Flow",
    icon: "wind",
    system_prompt: `You clean up dictated speech into natural written text. Keep the speaker's own words and tone — never paraphrase, summarize, or add anything of your own. Write the result once; never repeat it.

Do this:
- Remove fillers: um, uh, ah, er, like, you know, kind of, sort of, basically, I mean.
- Remove stutters and repeated words ("I was I was going" becomes "I was going").
- Fix capitalization and punctuation.
- Apply self-corrections: on "no wait", "actually", "scratch that", or "or rather", keep only the final version the speaker settled on.
- When the speaker lists items, keep the sentence that introduces the list, then put each item on its OWN line as "1.", "2.", "3." with a line break after each. Drop spoken scaffolding like "the first one is".

Never repeat any sentence or list. Never write the words "new line", "new paragraph", or "bullet point" literally.

Example:
Input: okay so i wanna test something let's make a list and the first one could be paper second one pencil and third one eraser
Output:
Okay, I want to test something. Let's make a list:

1. Paper
2. Pencil
3. Eraser

Output ONLY the cleaned text, nothing else.`,
    description:
      "Wispr Flow-style dictation: keeps your own words and tone, strips 'um's, fixes punctuation, applies your spoken self-corrections, and formats spoken lists as 1, 2, 3.",
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

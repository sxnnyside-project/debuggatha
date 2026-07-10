import type { ReviewPolicy } from "@debuggatha/knowledge-system";
import type { Finding } from "@debuggatha/review-engine";

export function reviewFiles(_paths: string[], _policy: ReviewPolicy): Finding[] {
  // We aren't implementing the real LLM engine yet, so we just return an empty array for now.
  // The goal is just to ensure the architectural boundary is ready.
  return [];
}

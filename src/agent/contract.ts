export type AgentTask = "classify_mail" | "evaluate_reply" | "generate_reply";

export interface AgentRequest {
  task: AgentTask;
  schema_version: "classification_v1" | "reply_review_v1" | "reply_candidate_v1";
  input: Record<string, unknown>;
  allowed_context_ids: string[];
  max_input_chars: number;
}

export interface AgentProvider {
  readonly id: string;
  readonly display_name: string;
  run(request: AgentRequest): Promise<Record<string, unknown>>;
}

/** Provider-neutral boundary: agents receive data, never A-mail handles or send capabilities. */
export function isAgentRequest(value: unknown): value is AgentRequest {
  if (!value || typeof value !== "object") return false;
  const request = value as Partial<AgentRequest>;
  const maxInputChars = request.max_input_chars;
  const schemaMatches = request.task === "classify_mail" && request.schema_version === "classification_v1"
    || request.task === "evaluate_reply" && request.schema_version === "reply_review_v1"
    || request.task === "generate_reply" && request.schema_version === "reply_candidate_v1";
  if (!schemaMatches || !request.input || typeof request.input !== "object") return false;
  if (!Array.isArray(request.allowed_context_ids) || !request.allowed_context_ids.every((id) => typeof id === "string")) return false;
  if (typeof maxInputChars !== "number" || !Number.isInteger(maxInputChars)) return false;
  return maxInputChars > 0 && maxInputChars <= 20_000;
}

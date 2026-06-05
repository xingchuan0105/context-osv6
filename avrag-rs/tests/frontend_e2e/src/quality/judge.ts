/**
 * LLM judge client — calls DashScope (or any OpenAI-compatible endpoint)
 * to score an answer against a golden-set criterion.
 */

export interface JudgeResult {
  score: number;
  dimensions?: Record<string, number>;
  reasoning: string;
}

export interface GoldenEntry {
  id: string;
  scenario: string;
  query?: string;
  turns?: string[];
  document?: string;
  expected: Record<string, unknown>;
  judge_prompt: string;
}

export async function judgeAnswer(
  answer: string,
  golden: GoldenEntry,
  model = "qwen-plus"
): Promise<JudgeResult> {
  const apiKey = process.env.DASHSCOPE_API_KEY ?? "";
  const baseUrl =
    process.env.AGENT_LLM_BASE_URL ?? "https://dashscope.aliyuncs.com/compatible-mode/v1";

  const body = {
    model,
    messages: [
      {
        role: "system",
        content:
          "You are an objective evaluator. Respond ONLY with the requested JSON. Do not add markdown fences or extra text.",
      },
      {
        role: "user",
        content: `Question: ${golden.query}\n\nAnswer: ${answer}\n\n${golden.judge_prompt}`,
      },
    ],
    temperature: 0.0,
  };

  const res = await fetch(`${baseUrl}/chat/completions`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${apiKey}`,
    },
    body: JSON.stringify(body),
  });

  if (!res.ok) {
    const text = await res.text();
    throw new Error(`Judge API error ${res.status}: ${text}`);
  }

  const json = (await res.json()) as any;
  const content: string = json.choices?.[0]?.message?.content ?? "";

  // Try to extract JSON from the response (some models wrap in markdown)
  const jsonMatch = content.match(/\{[\s\S]*\}/);
  const raw = jsonMatch ? jsonMatch[0] : content;

  try {
    return JSON.parse(raw) as JudgeResult;
  } catch {
    // Fallback: if the model didn't return valid JSON, treat the whole text as reasoning
    return { score: 0, reasoning: `Judge returned non-JSON: ${content}` };
  }
}

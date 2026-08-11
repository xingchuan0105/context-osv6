/**
 * P0 probe: pi tool_call hard-block.
 * Blocks any tool whose name contains "forbidden" or bash with "P0_BLOCK_ME".
 * Return shape is the official pi API: { block: true, reason?, terminate? }.
 */
// Installed global package is still @mariozechner/* at 0.73.1 (deprecated rename to @earendil-works).
import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";

export default function (pi: ExtensionAPI) {
  pi.on("tool_call", async (event) => {
    const name = event.toolName ?? "";
    const input = event.input as Record<string, unknown> | undefined;

    if (name.includes("forbidden")) {
      return {
        block: true,
        reason: "P0 hard-block-probe: tool name contains 'forbidden'",
        terminate: false,
      };
    }

    if (name === "bash") {
      const cmd = String(input?.command ?? "");
      if (cmd.includes("P0_BLOCK_ME")) {
        return {
          block: true,
          reason: "P0 hard-block-probe: bash command contains P0_BLOCK_ME",
          terminate: true,
        };
      }
    }
  });

  // Soft observation path for card-keeper-shaped feedback.
  pi.on("tool_result", async (event) => {
    if (event.isError) {
      // Observation-style: do not rewrite result; product would inject third-person note via agentd.
      return;
    }
  });
}

import { formatUiMessage } from "../../../lib/i18n/messages";

export function arraysEqual(left: string[], right: string[]) {
  if (left.length !== right.length) {
    return false;
  }

  return left.every((value, index) => value === right[index]);
}

export function getNoteMutationErrorMessage(
  locale: "zh-CN" | "en",
  action: "save" | "promote",
  error: unknown,
) {
  if (action === "promote" && error instanceof Error) {
    const message = error.message.trim();

    if (/cannot promote an empty note/i.test(message)) {
      return formatUiMessage(locale, "workspaceRightRail.promoteNoteEmptyError");
    }
  }

  return formatUiMessage(
    locale,
    action === "save" ? "workspaceRightRail.saveNoteError" : "workspaceRightRail.promoteNoteError",
  );
}

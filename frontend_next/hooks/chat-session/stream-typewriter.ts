import {
  STREAM_TYPEWRITER_CHARS_PER_TICK,
  STREAM_TYPEWRITER_INTERVAL_MS,
  STREAM_TYPEWRITER_MAX_DRAIN_CHARS_AFTER_DONE,
  getStreamingDisplayText,
  getPrefersReducedStreamingMotion,
} from "./helpers";
import type { PendingDoneEvent, UiProgressSnapshot } from "./types";

export type StreamTypewriterDeps = {
  appendStreamingDisplayText: (chunk: string) => void;
  finalizeStreamingDone: (
    event: PendingDoneEvent,
    progressSnapshot?: UiProgressSnapshot | null,
  ) => void;
};

type PendingDone = {
  event: PendingDoneEvent;
  progressSnapshot: UiProgressSnapshot | null;
};

export function createStreamTypewriter(deps: StreamTypewriterDeps) {
  const streamTypewriterTimerRef: { current: ReturnType<typeof setTimeout> | null } = { current: null };
  const streamTypewriterQueueRef = { current: "" };
  const streamDisplayedTextRef = { current: "" };
  const streamReceivedTokenRef = { current: false };
  const streamReduceMotionRef = { current: false };
  const pendingDoneRef: { current: PendingDone | null } = { current: null };

  function stopStreamingTypewriter() {
    if (streamTypewriterTimerRef.current !== null) {
      clearTimeout(streamTypewriterTimerRef.current);
      streamTypewriterTimerRef.current = null;
    }
  }

  function resetStreamingTypewriter() {
    stopStreamingTypewriter();
    streamTypewriterQueueRef.current = "";
    streamDisplayedTextRef.current = "";
    streamReceivedTokenRef.current = false;
    streamReduceMotionRef.current = getPrefersReducedStreamingMotion();
    pendingDoneRef.current = null;
  }

  function finalizePendingDoneIfReady() {
    if (streamTypewriterQueueRef.current.length > 0 || !pendingDoneRef.current) {
      return;
    }

    const pending = pendingDoneRef.current;
    pendingDoneRef.current = null;
    deps.finalizeStreamingDone(pending.event, pending.progressSnapshot);
  }

  function flushStreamingTypewriterQueue() {
    streamTypewriterTimerRef.current = null;

    const nextChunk = streamTypewriterQueueRef.current.slice(0, STREAM_TYPEWRITER_CHARS_PER_TICK);
    streamTypewriterQueueRef.current = streamTypewriterQueueRef.current.slice(STREAM_TYPEWRITER_CHARS_PER_TICK);
    streamDisplayedTextRef.current += nextChunk;
    deps.appendStreamingDisplayText(nextChunk);

    if (streamTypewriterQueueRef.current.length > 0) {
      scheduleStreamingTypewriter();
      return;
    }

    finalizePendingDoneIfReady();
  }

  function scheduleStreamingTypewriter() {
    if (streamTypewriterTimerRef.current !== null) {
      return;
    }

    streamTypewriterTimerRef.current = setTimeout(flushStreamingTypewriterQueue, STREAM_TYPEWRITER_INTERVAL_MS);
  }

  function enqueueStreamingText(text: string) {
    if (!text) {
      finalizePendingDoneIfReady();
      return;
    }

    if (streamReduceMotionRef.current) {
      streamDisplayedTextRef.current += text;
      deps.appendStreamingDisplayText(text);
      finalizePendingDoneIfReady();
      return;
    }

    streamTypewriterQueueRef.current += text;
    scheduleStreamingTypewriter();
  }

  function shouldDrainTypewriterQueueAfterDone(event: PendingDoneEvent) {
    if (!streamReceivedTokenRef.current || streamReduceMotionRef.current) {
      return false;
    }

    const queuedText = streamTypewriterQueueRef.current;

    if (!queuedText) {
      return false;
    }

    if (queuedText.length > STREAM_TYPEWRITER_MAX_DRAIN_CHARS_AFTER_DONE) {
      return false;
    }

    const answer = getStreamingDisplayText(event.payload.answer ?? "", event.payload.answer_blocks ?? []);

    if (!answer) {
      return true;
    }

    const queuedAnswer = `${streamDisplayedTextRef.current}${queuedText}`;

    if (!answer.startsWith(queuedAnswer)) {
      return false;
    }

    return answer.length - queuedAnswer.length <= STREAM_TYPEWRITER_MAX_DRAIN_CHARS_AFTER_DONE;
  }

  function handleDoneWithTypewriter(
    event: PendingDoneEvent,
    progressSnapshot?: UiProgressSnapshot | null,
  ) {
    const snap = progressSnapshot ?? null;

    // If the backend never streamed tokens (common for short RAG/search), still
    // typewriter the final answer so the UI does not "pop" the whole bubble at once.
    if (!streamReceivedTokenRef.current && !streamReduceMotionRef.current) {
      const answer = getStreamingDisplayText(
        event.payload.answer ?? "",
        event.payload.answer_blocks ?? [],
      );
      if (answer && streamDisplayedTextRef.current.length === 0) {
        pendingDoneRef.current = { event, progressSnapshot: snap };
        streamTypewriterQueueRef.current = answer;
        scheduleStreamingTypewriter();
        return;
      }
    }

    if (!shouldDrainTypewriterQueueAfterDone(event)) {
      deps.finalizeStreamingDone(event, snap);
      return;
    }

    pendingDoneRef.current = { event, progressSnapshot: snap };
    scheduleStreamingTypewriter();
  }

  function markTokenReceived() {
    streamReceivedTokenRef.current = true;
  }

  return {
    resetStreamingTypewriter,
    enqueueStreamingText,
    handleDoneWithTypewriter,
    markTokenReceived,
    stopStreamingTypewriter,
  };
}

export type StreamTypewriter = ReturnType<typeof createStreamTypewriter>;

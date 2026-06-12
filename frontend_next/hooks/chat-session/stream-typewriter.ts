import {
  STREAM_TYPEWRITER_CHARS_PER_TICK,
  STREAM_TYPEWRITER_INTERVAL_MS,
  STREAM_TYPEWRITER_MAX_DRAIN_CHARS_AFTER_DONE,
  getStreamingDisplayText,
  getPrefersReducedStreamingMotion,
} from "./helpers";
import type { PendingDoneEvent } from "./types";

export type StreamTypewriterDeps = {
  appendStreamingDisplayText: (chunk: string) => void;
  finalizeStreamingDone: (event: PendingDoneEvent) => void;
};

export function createStreamTypewriter(deps: StreamTypewriterDeps) {
  const streamTypewriterTimerRef: { current: ReturnType<typeof setTimeout> | null } = { current: null };
  const streamTypewriterQueueRef = { current: "" };
  const streamDisplayedTextRef = { current: "" };
  const streamReceivedTokenRef = { current: false };
  const streamReduceMotionRef = { current: false };
  const pendingDoneEventRef: { current: PendingDoneEvent | null } = { current: null };

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
    pendingDoneEventRef.current = null;
  }

  function finalizePendingDoneIfReady() {
    if (streamTypewriterQueueRef.current.length > 0 || !pendingDoneEventRef.current) {
      return;
    }

    deps.finalizeStreamingDone(pendingDoneEventRef.current);
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

  function handleDoneWithTypewriter(event: PendingDoneEvent) {
    if (!shouldDrainTypewriterQueueAfterDone(event)) {
      deps.finalizeStreamingDone(event);
      return;
    }

    pendingDoneEventRef.current = event;
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

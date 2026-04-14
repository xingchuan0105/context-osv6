import assert from "node:assert/strict";
import test from "node:test";

class FakeCanvasContext {
  measureText(text) {
    return { width: String(text).length * 8 };
  }
}

globalThis.OffscreenCanvas = class {
  getContext(kind) {
    return kind === "2d" ? new FakeCanvasContext() : null;
  }
};

test("text layout predictor wrapper executes browser helpers", async () => {
  const mod = await import("../pkg/text_layout_predictor.js");
  await mod.clearTextLayoutCaches();
  const prediction = await mod.predictTextHeight({
    text: "hello world",
    fontCss: "16px Inter",
    locale: "en",
    maxWidthPx: 320,
    lineHeightPx: 24,
  });

  assert.equal(Object.hasOwn(mod, "default"), false);
  assert.equal(typeof mod.predictTextHeight, "function");
  assert.equal(typeof mod.clearTextLayoutCaches, "function");
  assert.ok(prediction.textHeightPx > 0);
  assert.ok(prediction.lineCount > 0);
});

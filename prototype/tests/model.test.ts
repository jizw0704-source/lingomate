import assert from "node:assert/strict";
import { test } from "node:test";
import { insertText, normalizePinyin } from "../src/model.ts";

test("English words have a boundary, while Chinese text remains adjacent", () => {
  assert.equal(insertText("study", 5, 5, "English").value, "study English");
  assert.equal(insertText("学习", 2, 2, "英语").value, "学习英语");
  assert.equal(insertText("I ", 2, 2, "learn").value, "I learn");
});
test("replacement respects the selection and word boundaries on both sides", () => {
  assert.deepEqual(insertText("I study English", 2, 7, "learn"), {
    value: "I learn English",
    cursor: 7,
  });
  assert.equal(insertText("ab", 1, 1, "work").value, "a work b");
  assert.equal(insertText("", 0, 0, "learning").cursor, 8);
});
test("pinyin normalization supports separators and bounds the engine input", () => {
  assert.equal(normalizePinyin("Xue'Xi 学习123"), "xue'xi");
  assert.equal(normalizePinyin("a".repeat(100)).length, 80);
});

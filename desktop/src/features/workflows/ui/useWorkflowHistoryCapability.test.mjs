import assert from "node:assert/strict";
import test from "node:test";
import { supportsWorkflowHistory } from "./useWorkflowHistoryCapability.ts";

test("only the versioned advertised capability enables workflow history", () => {
  assert.equal(
    supportsWorkflowHistory({
      supported_extensions: ["nip-er", "buzz.workflow-history.v1"],
    }),
    true,
  );
  for (const document of [
    null,
    {},
    [],
    { supported_extensions: "buzz.workflow-history.v1" },
    { supported_extensions: ["buzz.workflow-history.v2"] },
    { supported_nips: [11] },
  ]) {
    assert.equal(supportsWorkflowHistory(document), false);
  }
});

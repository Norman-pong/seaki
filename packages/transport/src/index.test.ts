import { describe, expect, it } from "vitest";

import { SCHEMA_HASH, SCHEMA_VERSION } from "@seaki/dto";

import { TransportSchemaMismatchError, createMockTransportClient } from "./index";
import type { FrontendTransportEvent } from "./index";

function event(seq: number, eventType: string): FrontendTransportEvent {
  return {
    actor_id: "test",
    causation_id: `cause_${seq}`,
    correlation_id: "corr_1",
    event_id: `evt_${seq}`,
    idempotency_key: `idem_${seq}`,
    occurred_at: `2026-04-28T00:00:0${seq}.000Z`,
    payload: {},
    payload_schema_hash: SCHEMA_HASH,
    replayable: true,
    revision: "wiki_rev_0",
    schema_version: SCHEMA_VERSION,
    scope: "workspace:ws_1",
    seq,
    task_id: `task_${seq}`,
    transaction_id: `tx_${seq}`,
    type: eventType,
    workspace_id: "ws_1",
  };
}

describe("createMockTransportClient", () => {
  it("replays frontend envelopes after the requested sequence", async () => {
    const client = createMockTransportClient({
      events: [event(2, "workspace.init.completed"), event(1, "daemon.ready")],
    });
    const seen: number[] = [];

    const lastSeq = await client.replay(1, (nextEvent) => {
      seen.push(Number(nextEvent.seq));
    });

    expect(seen).toEqual([2]);
    expect(lastSeq).toBe(2);
  });

  it("records typed request calls and returns mock responses", async () => {
    const client = createMockTransportClient({
      responder: {
        "workspace.init": {
          workspaceId: "ws_1",
        },
      },
    });

    await expect(
      client.request("workspace.init", {
        rootUri: "file:///tmp/seaki",
      }),
    ).resolves.toEqual({
      workspaceId: "ws_1",
    });
    expect(client.getRequests()).toEqual([
      {
        input: {
          rootUri: "file:///tmp/seaki",
        },
        method: "workspace.init",
      },
    ]);
  });

  it("rejects stale schema replay without dispatching the invalid event", async () => {
    const stale = {
      ...event(1, "workspace.init.completed"),
      payload_schema_hash: "stale-schema",
    };
    const client = createMockTransportClient({
      events: [stale, event(2, "daemon.ready")],
    });
    const seen: number[] = [];

    await expect(
      client.replay(0, (nextEvent) => {
        seen.push(nextEvent.seq);
      }),
    ).rejects.toBeInstanceOf(TransportSchemaMismatchError);

    expect(seen).toEqual([]);
  });

  it("skips non-replayable events without advancing replay position", async () => {
    const nonReplayable = {
      ...event(2, "import.stage.changed"),
      replayable: false,
    };
    const client = createMockTransportClient({
      events: [event(1, "daemon.ready"), nonReplayable],
    });
    const seen: number[] = [];

    const lastSeq = await client.replay(0, (nextEvent) => {
      seen.push(nextEvent.seq);
    });

    expect(seen).toEqual([1]);
    expect(lastSeq).toBe(1);
  });
});

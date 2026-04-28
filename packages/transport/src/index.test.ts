import { describe, expect, it } from "vitest";

import { createMockTransportClient } from "./index";
import type { TransportEvent } from "./index";

describe("createMockTransportClient", () => {
  it("replays events after the requested sequence", async () => {
    const events: TransportEvent[] = [
      {
        eventId: "evt_2",
        occurredAt: "2026-04-28T00:00:02.000Z",
        payload: {},
        replayable: true,
        seq: 2,
        type: "workspace.ready",
      },
      {
        eventId: "evt_1",
        occurredAt: "2026-04-28T00:00:01.000Z",
        payload: {},
        replayable: true,
        seq: 1,
        type: "daemon.ready",
      },
    ];
    const client = createMockTransportClient({ events });
    const seen: number[] = [];

    const lastSeq = await client.replay(1, (event) => {
      seen.push(event.seq);
    });

    expect(seen).toEqual([2]);
    expect(lastSeq).toBe(2);
  });
});

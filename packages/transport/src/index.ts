export type DaemonConnectionStatus = "daemon.connecting" | "daemon.ready" | "daemon.unavailable";

export interface DaemonHeartbeat {
  readonly status: DaemonConnectionStatus;
  readonly checkedAt: string;
  readonly workspaceId?: string;
}

export interface TransportEvent<TPayload = unknown> {
  readonly eventId: string;
  readonly seq: number;
  readonly type: string;
  readonly occurredAt: string;
  readonly replayable: boolean;
  readonly payload: TPayload;
}

export type TransportEventHandler = (event: TransportEvent) => void;

export interface TransportClient {
  connect(): Promise<DaemonHeartbeat>;
  replay(fromSeq: number, onEvent: TransportEventHandler): Promise<number>;
}

export interface MockTransportClientOptions {
  readonly heartbeat?: DaemonHeartbeat;
  readonly events?: readonly TransportEvent[];
}

export function createMockTransportClient(
  options: MockTransportClientOptions = {},
): TransportClient {
  const heartbeat =
    options.heartbeat ??
    ({
      checkedAt: new Date(0).toISOString(),
      status: "daemon.unavailable",
    } satisfies DaemonHeartbeat);
  const events = [...(options.events ?? [])].sort((left, right) => left.seq - right.seq);

  return {
    async connect() {
      return heartbeat;
    },
    async replay(fromSeq, onEvent) {
      let lastSeq = fromSeq;

      for (const event of events) {
        if (event.seq > fromSeq) {
          onEvent(event);
          lastSeq = event.seq;
        }
      }

      return lastSeq;
    },
  };
}

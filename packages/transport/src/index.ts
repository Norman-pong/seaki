import { SCHEMA_HASH, SCHEMA_VERSION } from "@seaki/dto";
import type { FrontendEventEnvelope } from "@seaki/dto";

export type FrontendTransportEvent = FrontendEventEnvelope;

export type FrontendEventHandler = (event: FrontendTransportEvent) => void;

export interface TransportRequestRecord<TInput = unknown> {
  readonly method: string;
  readonly input: TInput;
}

export interface TransportClient {
  request<TOutput = unknown, TInput = unknown>(method: string, input: TInput): Promise<TOutput>;
  replay(fromSeq: number, onEvent: FrontendEventHandler): Promise<number>;
}

export type MockTransportResponder =
  | Record<string, unknown>
  | ((record: TransportRequestRecord) => Promise<unknown> | unknown);

export interface MockTransportClient extends TransportClient {
  readonly requests: readonly TransportRequestRecord[];
  getRequests(): readonly TransportRequestRecord[];
}

export interface MockTransportClientOptions {
  readonly events?: readonly FrontendTransportEvent[];
  readonly responder?: MockTransportResponder;
}

export class TransportSchemaMismatchError extends Error {
  constructor(
    readonly eventId: string,
    readonly foundSchemaVersion: number,
    readonly foundSchemaHash: string,
  ) {
    super(`event ${eventId} does not match generated DTO schema`);
    this.name = "TransportSchemaMismatchError";
  }
}

function eventSeq(event: FrontendTransportEvent): number {
  return Number(event.seq);
}

function shouldReplay(event: FrontendTransportEvent): boolean {
  if (!event.replayable) {
    return false;
  }

  if (event.schema_version !== SCHEMA_VERSION || event.payload_schema_hash !== SCHEMA_HASH) {
    throw new TransportSchemaMismatchError(
      event.event_id,
      Number(event.schema_version),
      event.payload_schema_hash,
    );
  }

  return true;
}

async function resolveMockResponse(
  responder: MockTransportResponder | undefined,
  record: TransportRequestRecord,
): Promise<unknown> {
  if (!responder) {
    return undefined;
  }

  if (typeof responder === "function") {
    return responder(record);
  }

  return responder[record.method];
}

export function createMockTransportClient(
  options: MockTransportClientOptions = {},
): MockTransportClient {
  const events = [...(options.events ?? [])].sort(
    (left, right) => eventSeq(left) - eventSeq(right),
  );
  const requests: TransportRequestRecord[] = [];

  return {
    get requests() {
      return [...requests];
    },
    getRequests() {
      return [...requests];
    },
    async request<TOutput = unknown, TInput = unknown>(method: string, input: TInput) {
      const record: TransportRequestRecord = {
        input,
        method,
      };

      requests.push(record);

      return (await resolveMockResponse(options.responder, record)) as TOutput;
    },
    async replay(fromSeq, onEvent) {
      let lastSeq = fromSeq;

      for (const event of events) {
        const seq = eventSeq(event);

        if (seq > fromSeq && shouldReplay(event)) {
          onEvent(event);
          lastSeq = seq;
        }
      }

      return lastSeq;
    },
  };
}

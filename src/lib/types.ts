export interface KeyValue {
  key: string;
  value: string;
}

/** Where a variable rides. "auto" = query for GET/raw, multipart field otherwise. */
export type VariableTransport = "auto" | "query" | "header";

export interface EndpointVariable {
  key: string;
  label: string;
  required: boolean;
  default_value: string;
  transport: VariableTransport;
}

export interface Endpoint {
  id: string;
  name: string;
  url: string;
  method: "POST" | "GET";
  body_mode: "multipart" | "raw" | "none";
  file_field: string;
  extra_fields: KeyValue[];
  headers: KeyValue[];
  variables: EndpointVariable[];
  created_at: string;
  updated_at: string;
}

export interface JobSend {
  endpoint_id: string;
  endpoint_name: string;
  at: string;
  ok: boolean;
  http_status: number | null;
  duration_ms: number;
  response_head: string | null;
  error: string | null;
}

export interface Job {
  id: string;
  title: string;
  user: string | null;
  format: string;
  bytes: number;
  received_at: string;
  spool_path: string;
  status: "pending" | "sent" | "dismissed";
  sends: JobSend[];
}

export interface Settings {
  printer_name: string;
  port: number;
  allow_lan: boolean;
  autostart: boolean;
  retention_max_jobs: number;
  retention_max_days: number;
  printer_uuid: string;
}

export interface CaptureStatus {
  running: boolean;
  paused: boolean;
  port: number;
  printer_name: string;
  error: string | null;
}

export interface DispatchResult {
  job_id: string;
  endpoint_id: string;
  ok: boolean;
  http_status: number | null;
  duration_ms: number;
  response_head: string | null;
  error: string | null;
}

export interface UpdateInfo {
  current: string;
  latest: string | null;
  update_available: boolean;
  url: string;
}

export interface LogEntry {
  seq: number;
  ts: string;
  level: "debug" | "info" | "warn" | "error";
  area: string;
  msg: string;
  detail: unknown;
}

export function emptyEndpoint(): Endpoint {
  return {
    id: "",
    name: "",
    url: "",
    method: "POST",
    body_mode: "multipart",
    file_field: "file",
    extra_fields: [],
    headers: [],
    variables: [],
    created_at: "",
    updated_at: "",
  };
}

export function emptyVariable(): EndpointVariable {
  return { key: "", label: "", required: false, default_value: "", transport: "auto" };
}

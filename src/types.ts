export interface Identity {
  id: string;
  email: string;
  name: string;
  signature: string;
}
export interface Attachment {
  id: string;
  filename: string;
  content_type: string;
  size: number;
  cached: boolean;
}
export interface Mail {
  id: string;
  provider_id: string;
  direction: "incoming" | "outgoing";
  from: string;
  to: string[];
  cc: string[];
  bcc: string[];
  reply_to: string[];
  subject: string;
  text: string;
  html: string;
  created_at: string;
  message_id: string;
  headers: Record<string, string>;
  attachments: Attachment[];
  status: string;
  folder: string;
  is_read: boolean;
  starred: boolean;
  labels: string[];
  hydrated: boolean;
  links: string[];
}
export interface Draft {
  id: string;
  revision: number;
  identity_id: string;
  to: string;
  cc: string;
  bcc: string;
  subject: string;
  text: string;
  html: string;
  attachments: Attachment[];
  headers: Record<string, string>;
  updated_at: string;
}
export interface Intent {
  id: string;
  draft_id: string;
  subject: string;
  to: string[];
  state: string;
  error: string | null;
  first_attempt: number | null;
  provider_id: string | null;
}
export interface Profile {
  id: string;
  connected: boolean;
  identities: Identity[];
  settings: { theme?: string };
  last_sync: string | null;
}
export interface Snapshot {
  profile: Profile;
  messages: Mail[];
  drafts: Draft[];
  outbox: Intent[];
  pending_bodies: number;
  history_complete: boolean;
  sync_error: string | null;
  inbox_unread: number;
  labels: string[];
}
export interface Query {
  domain?: string;
  include_subdomains?: boolean;
  folder: string;
  identity: string;
  search: string;
  limit: number;
  offset: number;
}
export interface Probe {
  incoming: boolean;
  outgoing: boolean;
  incoming_error: string | null;
  outgoing_error: string | null;
}

import {
  useCallback,
  useEffect,
  useRef,
  useState,
  lazy,
  Suspense,
  type ComponentType,
} from "react";
import {
  Archive,
  ArrowDown,
  ArrowLeft,
  ArrowRight,
  ArrowUpRight,
  Check,
  CheckCheck,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  CircleHelp,
  FileText,
  Inbox,
  Layers,
  Loader2,
  Mail,
  MailOpen,
  MoreHorizontal,
  PanelRightClose,
  Paperclip,
  PenLine,
  Plus,
  RefreshCw,
  Reply,
  ReplyAll,
  Search,
  Send,
  Settings as SettingsIcon,
  ShieldCheck,
  Star,
  Tag,
  Trash2,
  Undo2,
  WifiOff,
  X,
} from "lucide-react";
import { command, desktop, confirmAction } from "./backend";
import {
  blankDraft,
  emailAddress,
  formatBytes,
  initials,
  personName,
  readerDocument,
  relatedMessages,
  respond,
} from "./mail";
import type { Draft, Mail as Message, Snapshot } from "./types";
const Composer = lazy(() => import("./Composer"));
const Settings = lazy(() => import("./Settings"));

const folders: {
  id: string;
  label: string;
  icon: ComponentType<{ size?: number }>;
}[] = [
  { id: "inbox", label: "Inbox", icon: Inbox },
  { id: "starred", label: "Starred", icon: Star },
  { id: "sent", label: "Sent", icon: Send },
  { id: "drafts", label: "Drafts", icon: FileText },
  { id: "outbox", label: "Outbox", icon: ArrowUpRight },
  { id: "archive", label: "Archive", icon: Archive },
  { id: "trash", label: "Trash", icon: Trash2 },
  { id: "all", label: "All mail", icon: Layers },
];
const prettyDate = (date: string) => {
  const d = new Date(date);
  return Number.isNaN(d.getTime())
    ? ""
    : d.toDateString() === new Date().toDateString()
      ? d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })
      : d.toLocaleDateString([], { month: "short", day: "numeric" });
};
const colorFor = (s: string) =>
  ["sage", "clay", "lavender", "sand", "blue"][
    Array.from(s).reduce((n, c) => n + c.charCodeAt(0), 0) % 5
  ];

export default function App() {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [folder, setFolder] = useState("inbox");
  const [identity, setIdentity] = useState("");
  const [search, setSearch] = useState("");
  const [query, setQuery] = useState("");
  const [offset, setOffset] = useState(0);
  const [selected, setSelected] = useState("");
  const [detail, setDetail] = useState<Message | null>(null);
  const [loading, setLoading] = useState(true);
  const [reading, setReading] = useState(false);
  const [readerError, setReaderError] = useState("");
  const [refreshing, setRefreshing] = useState(false);
  const [settings, setSettings] = useState(false);
  const [composer, setComposer] = useState<Draft | null>(null);
  const [toast, setToast] = useState<{
    text: string;
    error: boolean;
    undo?: () => void;
  } | null>(null);
  const [online, setOnline] = useState(navigator.onLine);
  const [plain, setPlain] = useState(false);
  const [labelEdit, setLabelEdit] = useState(false);
  const [labelValue, setLabelValue] = useState("");
  const [systemDark, setSystemDark] = useState(
    window.matchMedia("(prefers-color-scheme: dark)").matches,
  );
  const searchRef = useRef<HTMLInputElement>(null);
  const selectedRef = useRef("");
  const profileRef = useRef("");
  const requestRef = useRef(0);
  const demoSelected = useRef(false);
  const loadGeneration = useRef(0);
  const notify = useCallback(
    (text: string, error = false) => setToast({ text, error }),
    [],
  );
  const theme = snapshot?.profile.settings.theme || "system";
  const dark = theme === "dark" || (theme === "system" && systemDark);
  const reload = useCallback(async () => {
    const generation = ++loadGeneration.current;
    const s = await command<Snapshot>("get_snapshot");
    if (generation !== loadGeneration.current) return;
    if (profileRef.current && profileRef.current !== s.profile.id) {
      setSelected("");
      setDetail(null);
      setIdentity("");
      setComposer(null);
    }
    profileRef.current = s.profile.id;
    setSnapshot(s);
    if (!["drafts", "outbox"].includes(folder)) {
      const mails = await command<Message[]>("list_messages", {
        query: { folder, identity, search: query, limit: 100, offset },
      });
      if (generation !== loadGeneration.current) return;
      setMessages(mails);
    }
    setLoading(false);
  }, [folder, identity, query, offset]);
  useEffect(() => {
    let alive = true;
    setLoading(true);
    void reload().catch((e) => alive && notify(String(e), true));
    const timer = setInterval(() => {
      void reload().catch(() => {});
    }, 5000);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, [reload, notify]);
  useEffect(() => {
    const timer = setTimeout(() => {
      setQuery(search);
      setOffset(0);
    }, 200);
    return () => clearTimeout(timer);
  }, [search]);
  useEffect(() => {
    document.documentElement.dataset.theme = dark ? "dark" : "light";
  }, [dark]);
  useEffect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const change = () => setSystemDark(mq.matches);
    mq.addEventListener("change", change);
    return () => mq.removeEventListener("change", change);
  }, []);
  useEffect(() => {
    const update = () => setOnline(navigator.onLine);
    window.addEventListener("online", update);
    window.addEventListener("offline", update);
    return () => {
      window.removeEventListener("online", update);
      window.removeEventListener("offline", update);
    };
  }, []);
  useEffect(() => {
    if (!toast) return;
    const timer = setTimeout(() => setToast(null), toast.undo ? 9000 : 6000);
    return () => clearTimeout(timer);
  }, [toast]);
  useEffect(() => {
    selectedRef.current = selected;
  }, [selected]);
  useEffect(() => {
    if (
      !desktop &&
      !demoSelected.current &&
      !selected &&
      messages[0] &&
      folder === "inbox" &&
      innerWidth > 850
    ) {
      demoSelected.current = true;
      openMessage(messages[0]);
    }
  }, [messages]);
  async function openMessage(m: Message) {
    const request = ++requestRef.current;
    selectedRef.current = m.id;
    setSelected(m.id);
    setDetail(m);
    setReaderError("");
    setReading(true);
    setPlain(false);
    setLabelEdit(false);
    try {
      const full = await command<Message>("read_message", { id: m.id });
      if (request === requestRef.current) {
        setDetail(full);
        setMessages((items) =>
          items.map((x) => (x.id === m.id ? { ...x, is_read: true } : x)),
        );
      }
    } catch (e) {
      if (request === requestRef.current) setReaderError(String(e));
    } finally {
      if (request === requestRef.current) setReading(false);
    }
  }
  function navigate(id: string) {
    setFolder(id);
    setSelected("");
    selectedRef.current = "";
    setDetail(null);
    setOffset(0);
    setLabelEdit(false);
    requestRef.current++;
  }
  function compose(draft?: Draft) {
    if (!snapshot) return;
    if (!snapshot.profile.identities.length) {
      setSettings(true);
      notify("Add a sender identity to start writing.");
      return;
    }
    setComposer(
      draft
        ? { ...draft, revision: draft.revision + 1 }
        : blankDraft(snapshot.profile.identities),
    );
  }
  async function refresh() {
    setRefreshing(true);
    try {
      await command("refresh_connection");
      await reload();
      notify("Available mail refreshed.");
    } catch (e) {
      notify(String(e), true);
    } finally {
      setRefreshing(false);
    }
  }
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        if (!composer && !settings) searchRef.current?.focus();
        return;
      }
      if (
        composer ||
        settings ||
        (e.target instanceof HTMLElement &&
          (["INPUT", "TEXTAREA", "SELECT"].includes(e.target.tagName) ||
            e.target.isContentEditable))
      )
        return;
      if (e.key.toLowerCase() === "n") {
        e.preventDefault();
        compose();
      }
      if (["j", "k"].includes(e.key.toLowerCase())) {
        const index = messages.findIndex((m) => m.id === selectedRef.current);
        const next = messages[index + (e.key.toLowerCase() === "j" ? 1 : -1)];
        if (next) {
          e.preventDefault();
          void openMessage(next);
        }
      }
      if (e.key === "Escape") {
        setSelected("");
        setDetail(null);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  });
  async function flags(m: Message, patch: Record<string, unknown>) {
    await command("set_local_flags", { id: m.id, ...patch });
    if (selectedRef.current === m.id)
      setDetail((old) =>
        old
          ? {
              ...old,
              ...patch,
              ...(patch.isRead !== undefined
                ? { is_read: patch.isRead as boolean }
                : {}),
            }
          : old,
      );
    await reload();
  }
  async function move(destination: string) {
    if (!detail) return;
    const m = detail;
    try {
      await flags(m, { folder: destination });
      setSelected("");
      setDetail(null);
      setToast({
        text:
          destination === "archive"
            ? "Message archived."
            : destination === "trash"
              ? "Moved to Trash."
              : "Message restored.",
        error: false,
        undo: () => {
          void flags(m, { folder: m.folder }).catch((e) =>
            notify(String(e), true),
          );
          setToast(null);
        },
      });
    } catch (e) {
      notify(String(e), true);
    }
  }
  async function star(m: Message) {
    try {
      await flags(m, { starred: !m.starred });
    } catch (e) {
      notify(String(e), true);
    }
  }
  const allMessages = snapshot?.messages || [];
  const identities = snapshot?.profile.identities || [];
  const profile = snapshot?.profile;
  const unread = snapshot?.inbox_unread || 0;
  const labels = snapshot?.labels || [];
  const title = identity
    ? emailAddress(identity)
    : folders.find((f) => f.id === folder)?.label ||
      (folder === "unmatched" ? "Unmatched addresses" : "Mail");
  const activeIntents = new Set(snapshot?.outbox.map((i) => i.draft_id) || []);
  const drafts = snapshot?.drafts.filter((d) => !activeIntents.has(d.id)) || [];
  const thread = detail
    ? relatedMessages(detail, [
        ...allMessages.filter((m) => m.id !== detail.id),
        detail,
      ])
    : [];
  const resetSelection = () => {
    setSelected("");
    setDetail(null);
    selectedRef.current = "";
    requestRef.current++;
  };
  return (
    <div className={`app-shell ${selected ? "reader-visible" : ""}`}>
      <aside className="sidebar" aria-label="Mail navigation">
        <div className="brand">
          <svg viewBox="0 0 34 34" aria-hidden="true">
            <path
              d="M6 9h14L10 19H4L14 9M15 17h14L19 27h-7l10-10"
              fill="currentColor"
            />
          </svg>
          <span>
            resend<span className="brand-divider">/</span>
            <small>mail</small>
          </span>
        </div>
        <button
          className="workspace-switch"
          onClick={() => setSettings(true)}
          aria-label="Open workspace settings"
        >
          <span className="workspace-icon">
            {initials(identities[0]?.name || "Your workspace")}
          </span>
          <span>
            <strong>{identities[0]?.name || "Your workspace"}</strong>
            <small>{desktop ? "Personal workspace" : "Demo workspace"}</small>
          </span>
          <ChevronDown size={15} />
        </button>
        <button className="compose-button" onClick={() => compose()}>
          <PenLine size={17} />
          <span>New message</span>
          <kbd>N</kbd>
        </button>
        <nav className="folder-nav" aria-label="Folders">
          {folders.map(({ id, label, icon: Icon }) => {
            const count =
              id === "inbox"
                ? unread
                : id === "drafts"
                  ? drafts.length
                  : id === "outbox"
                    ? snapshot?.outbox.length || 0
                    : 0;
            return (
              <button
                key={id}
                className={`nav-item ${folder === id && !identity ? "selected" : ""}`}
                aria-current={folder === id && !identity ? "page" : undefined}
                onClick={() => {
                  setIdentity("");
                  navigate(id);
                }}
              >
                <Icon size={17} />
                <span>{label}</span>
                {count > 0 && <span className="count-badge">{count}</span>}
              </button>
            );
          })}
        </nav>
        <div className="sidebar-heading">
          <span>YOUR ADDRESSES</span>
          <button
            className="icon-button"
            aria-label="Add sender identity"
            onClick={() => setSettings(true)}
          >
            <Plus size={14} />
          </button>
        </div>
        <nav className="identity-nav" aria-label="Addresses">
          {identities.map((i, index) => (
            <button
              key={i.id}
              className={`nav-item identity-nav-item ${identity === i.email ? "selected" : ""}`}
              onClick={() => {
                navigate("all");
                setIdentity(i.email);
              }}
              title={i.email}
            >
              <span className={`identity-dot ${index % 2 ? "clay" : "sage"}`} />
              <span>{i.email}</span>
            </button>
          ))}
          {!identities.length && (
            <button
              className="text-button add-address"
              onClick={() => setSettings(true)}
            >
              + Add your first address
            </button>
          )}
          <button
            className={`nav-item ${folder === "unmatched" ? "selected" : ""}`}
            onClick={() => {
              setIdentity("");
              navigate("unmatched");
            }}
          >
            <Mail size={15} />
            <span>Unmatched</span>
          </button>
        </nav>
        <div className="sidebar-heading">
          <span>LABELS</span>
          <button
            className="icon-button"
            aria-label="Manage labels on selected mail"
            disabled={!detail}
            onClick={() => {
              if (detail) {
                setLabelValue(detail.labels.join(", "));
                setLabelEdit(true);
              }
            }}
          >
            <Plus size={14} />
          </button>
        </div>
        <nav className="label-nav" aria-label="Labels">
          {labels.map((l, index) => (
            <button
              className="nav-item"
              key={l}
              onClick={() => {
                navigate("all");
                setIdentity("");
                setSearch(`label:"${l}"`);
              }}
            >
              <span
                className={`label-dot ${["sage", "clay", "lavender", "sand"][index % 4]}`}
              />
              <span>{l}</span>
            </button>
          ))}
          {!labels.length && (
            <p className="sidebar-hint">A little order, your way.</p>
          )}
        </nav>
        <div className="sidebar-bottom">
          <div className="local-note">
            <ShieldCheck size={15} />
            <span>Local by design. Yours to keep.</span>
          </div>
          <button className="nav-item" onClick={() => setSettings(true)}>
            <SettingsIcon size={17} />
            <span>Settings</span>
            <span className="version">v0.1</span>
          </button>
        </div>
      </aside>
      <main className="workspace">
        <header className="topbar">
          <div className="breadcrumb">
            Your workspace <ChevronRight size={13} /> <strong>{title}</strong>
          </div>
          <div className="search-box">
            <Search size={16} />
            <input
              ref={searchRef}
              aria-label="Search downloaded mail"
              placeholder="Search your mail"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
            {search ? (
              <button
                className="icon-button"
                aria-label="Clear search"
                onClick={() => setSearch("")}
              >
                <X size={13} />
              </button>
            ) : (
              <kbd>⌘ K</kbd>
            )}
          </div>
          <button
            className={`icon-button refresh-button ${refreshing ? "spinning" : ""}`}
            aria-label="Refresh mail"
            disabled={refreshing || !profile?.connected}
            onClick={() => void refresh()}
          >
            <RefreshCw size={17} />
          </button>
          <button
            className="profile-avatar"
            aria-label="Open account settings"
            onClick={() => setSettings(true)}
          >
            {initials(identities[0]?.name || "You")}
          </button>
        </header>
        {!desktop && (
          <div className="demo-banner">
            <span className="status-dot" />
            <strong>Demo workspace</strong>
            <span>Synthetic mail. Explore freely; nothing is sent.</span>
            <span className="demo-desktop">
              Connect real mail in the desktop app <ArrowUpRight size={12} />
            </span>
          </div>
        )}
        {!online && (
          <div className="offline-banner" role="status">
            <WifiOff size={15} /> You’re offline. Downloaded mail and local
            drafts are still here.
            {profile?.last_sync && (
              <span>Last refresh {prettyDate(profile.last_sync)}</span>
            )}
          </div>
        )}
        {snapshot?.sync_error && online && (
          <div className="offline-banner" role="status">
            <CircleHelp size={15} />
            <span>Refresh paused: {snapshot.sync_error}</span>
          </div>
        )}
        <div className="mail-workspace">
          <section className="message-pane" aria-label="Message list">
            <header className="list-heading">
              <div>
                <h1>{title}</h1>
                <p>
                  {folder === "drafts"
                    ? "A thought in progress."
                    : folder === "outbox"
                      ? "Every send, accounted for."
                      : search
                        ? "Search your downloaded mail."
                        : identity
                          ? "One address, a little more focus."
                          : "A little space for correspondence."}
                </p>
              </div>
              <span className="heading-mark">
                {folder === "inbox" ? <Inbox size={21} /> : <Mail size={21} />}
              </span>
            </header>
            <div className="list-toolbar">
              <span>
                {folder === "drafts"
                  ? `${drafts.length} saved drafts`
                  : folder === "outbox"
                    ? `${snapshot?.outbox.length || 0} pending`
                    : search
                      ? `${messages.length}${messages.length === 100 ? "+" : ""} results`
                      : `${unread} unread`}
              </span>
              {!["drafts", "outbox"].includes(folder) && (
                <button
                  className="text-button"
                  onClick={() =>
                    setSearch(search === "is:unread" ? "" : "is:unread")
                  }
                  aria-pressed={search === "is:unread"}
                >
                  {search === "is:unread" ? "Show all" : "Unread"}
                  <ChevronDown size={12} />
                </button>
              )}
            </div>
            {search && (
              <p className="search-scope">
                Downloaded text only ·{" "}
                <span title="Combine from:, to:, subject:, label:, after:YYYY-MM-DD, before:, has:attachment, is:unread">
                  filter syntax available
                </span>
                {snapshot?.pending_bodies
                  ? ` · ${snapshot.pending_bodies} bodies pending`
                  : ""}
              </p>
            )}
            <div
              className="message-list"
              role="list"
              aria-label={
                folder === "drafts"
                  ? "Saved drafts"
                  : folder === "outbox"
                    ? "Queued sends"
                    : "Messages"
              }
            >
              {loading && !snapshot && (
                <div className="list-empty">
                  <Loader2 size={24} className="spin" />
                  <p>Opening your workspace…</p>
                </div>
              )}
              {folder === "drafts" &&
                drafts.map((d) => (
                  <button
                    role="listitem"
                    className="message-card draft-card"
                    key={d.id}
                    onClick={() => compose(d)}
                  >
                    <span className="message-row">
                      <span className="draft-tag">Draft</span>
                      <time>{prettyDate(d.updated_at)}</time>
                    </span>
                    <strong>{d.subject || "An unwritten subject"}</strong>
                    <p>{d.to || "No recipient yet"}</p>
                    <p>{d.text || "A fresh page, waiting for you."}</p>
                  </button>
                ))}
              {folder === "outbox" &&
                snapshot?.outbox.map((item) => (
                  <article
                    role="listitem"
                    className="outbox-card"
                    key={item.id}
                  >
                    <div className="message-row">
                      <strong>{item.subject || "(No subject)"}</strong>
                      <span className={`send-state ${item.state}`}>
                        {item.state.replaceAll("_", " ")}
                      </span>
                    </div>
                    <p>{item.to.join(", ")}</p>
                    <p className="outbox-explanation">
                      {item.state === "unknown"
                        ? "Send outcome unknown—checking status. The original payload and key are preserved."
                        : item.state === "submitting"
                          ? "Submitting to Resend. This cannot be recalled."
                          : item.error ||
                            "Waiting to submit while the application is running."}
                    </p>
                    <div className="outbox-actions">
                      {item.first_attempt === null &&
                        item.state === "queued" && (
                          <button
                            className="text-button"
                            onClick={() =>
                              void command("cancel_intent", { id: item.id })
                                .then(reload)
                                .catch((e) => notify(String(e), true))
                            }
                          >
                            Cancel queued send
                          </button>
                        )}
                      {["failed", "unknown"].includes(item.state) && (
                        <button
                          className="text-button"
                          onClick={() =>
                            void command("retry_intent", { id: item.id })
                              .then(reload)
                              .catch((e) => notify(String(e), true))
                          }
                        >
                          Retry same attempt
                        </button>
                      )}
                      {["failed", "needs_review", "unknown"].includes(
                        item.state,
                      ) && (
                        <button
                          className="text-button"
                          onClick={async () => {
                            if (
                              await confirmAction(
                                "Check Resend first. Creating a new send may duplicate the original. Continue with a new draft?",
                              )
                            ) {
                              void command<Draft>("make_intent_copy", {
                                id: item.id,
                              })
                                .then(async (draft) => {
                                  await reload();
                                  compose(draft);
                                })
                                .catch((e) => notify(String(e), true));
                            }
                          }}
                        >
                          Make a copy
                        </button>
                      )}
                    </div>
                  </article>
                ))}
              {!["drafts", "outbox"].includes(folder) &&
                messages.map((m) => (
                  <article
                    key={m.id}
                    role="listitem"
                    className={`message-card ${selected === m.id ? "selected" : ""} ${!m.is_read ? "unread" : ""}`}
                  >
                    <button
                      className="message-select"
                      aria-label={`Read ${m.subject || "untitled message"} from ${personName(m.from)}`}
                      aria-current={selected === m.id ? true : undefined}
                      onClick={() => void openMessage(m)}
                    >
                      <div className="message-row">
                        <span className={`avatar ${colorFor(m.from)}`}>
                          {initials(m.from)}
                        </span>
                        <span className="message-sender">
                          {personName(m.from) || m.from}
                        </span>
                        <time>{prettyDate(m.created_at)}</time>
                      </div>
                      <div className="message-subject">
                        {!m.is_read && <span className="unread-dot" />}
                        <strong>{m.subject || "(No subject)"}</strong>
                      </div>
                      <p className="message-preview">
                        {m.text ||
                          (!m.hydrated
                            ? "Body not downloaded yet"
                            : "Open to read the message")}
                      </p>
                      <div className="message-meta">
                        <span className="recipient-address">
                          {m.direction === "outgoing" ? "To: " : ""}
                          {m.direction === "outgoing" ? m.to[0] : m.to[0]}
                        </span>
                        {m.attachments.length > 0 && <Paperclip size={12} />}
                        <span className="message-labels">
                          {m.labels.slice(0, 1).map((l) => (
                            <span key={l}>{l}</span>
                          ))}
                        </span>
                        {m.direction === "outgoing" && (
                          <span>
                            {m.status === "accepted"
                              ? "Accepted by Resend"
                              : m.status}
                          </span>
                        )}
                      </div>
                    </button>
                    <button
                      className={`message-star ${m.starred ? "starred" : ""}`}
                      aria-label={`${m.starred ? "Unstar" : "Star"} ${m.subject}`}
                      aria-pressed={m.starred}
                      onClick={() => void star(m)}
                    >
                      <Star
                        size={14}
                        fill={m.starred ? "currentColor" : "none"}
                      />
                    </button>
                  </article>
                ))}
              {!loading &&
                ((folder === "drafts" && !drafts.length) ||
                  (folder === "outbox" && !snapshot?.outbox.length) ||
                  (!["drafts", "outbox"].includes(folder) &&
                    !messages.length)) && (
                  <div className="list-empty">
                    <Inbox size={30} strokeWidth={1.2} />
                    <h3>
                      {search
                        ? "No matching mail"
                        : folder === "drafts"
                          ? "Room for a new thought"
                          : folder === "outbox"
                            ? "All clear here"
                            : "A quiet corner"}
                    </h3>
                    <p>
                      {search
                        ? "Try another word or remove a filter."
                        : !profile?.connected
                          ? "Connect your account to bring your mail home."
                          : "Your messages will appear here."}
                    </p>
                    {!profile?.connected && (
                      <button
                        className="secondary-button"
                        onClick={() => setSettings(true)}
                      >
                        Connect Resend
                      </button>
                    )}
                  </div>
                )}
            </div>
            <footer className="list-footer">
              <span>
                <span className={`status-dot ${online ? "" : "offline"}`} />
                {refreshing
                  ? "Refreshing…"
                  : profile?.last_sync
                    ? `Refreshed ${prettyDate(profile.last_sync)}`
                    : profile?.connected
                      ? "Ready to refresh"
                      : "Not connected"}
              </span>
              {!["drafts", "outbox"].includes(folder) && (
                <div>
                  <button
                    className="icon-button"
                    aria-label="Previous page"
                    disabled={offset === 0}
                    onClick={() => {
                      setOffset(Math.max(0, offset - 100));
                      resetSelection();
                    }}
                  >
                    <ChevronLeft size={13} />
                  </button>
                  <button
                    className="icon-button"
                    aria-label="Next page"
                    disabled={messages.length < 100}
                    onClick={() => {
                      setOffset(offset + 100);
                      resetSelection();
                    }}
                  >
                    <ChevronRight size={13} />
                  </button>
                </div>
              )}
            </footer>
          </section>
          <section
            className={`reader-pane ${detail ? "has-message" : ""}`}
            aria-label="Mail reader"
          >
            {detail && !["drafts", "outbox"].includes(folder) ? (
              <>
                <header className="reader-toolbar">
                  <div>
                    <button
                      className="icon-button mobile-back"
                      aria-label="Back to message list"
                      onClick={resetSelection}
                    >
                      <ArrowLeft size={18} />
                    </button>
                    <button
                      className="icon-button"
                      aria-label="Archive message"
                      onClick={() => void move("archive")}
                    >
                      <Archive size={18} />
                    </button>
                    <button
                      className="icon-button"
                      aria-label={
                        detail.folder === "trash"
                          ? "Restore message"
                          : "Move to Trash"
                      }
                      onClick={() =>
                        void move(
                          detail.folder === "trash"
                            ? detail.direction === "incoming"
                              ? "inbox"
                              : "sent"
                            : "trash",
                        )
                      }
                    >
                      {detail.folder === "trash" ? (
                        <Undo2 size={17} />
                      ) : (
                        <Trash2 size={17} />
                      )}
                    </button>
                    <span className="divider" />
                    <button
                      className="icon-button"
                      aria-label={detail.is_read ? "Mark unread" : "Mark read"}
                      onClick={() =>
                        void flags(detail, { isRead: !detail.is_read }).catch(
                          (e) => notify(String(e), true),
                        )
                      }
                    >
                      {detail.is_read ? (
                        <Mail size={18} />
                      ) : (
                        <MailOpen size={18} />
                      )}
                    </button>
                    <button
                      className={`icon-button ${detail.starred ? "starred" : ""}`}
                      aria-label="Toggle star"
                      onClick={() => void star(detail)}
                    >
                      <Star
                        size={18}
                        fill={detail.starred ? "currentColor" : "none"}
                      />
                    </button>
                    <div className="label-control">
                      <button
                        className="icon-button"
                        aria-label="Edit message labels"
                        aria-expanded={labelEdit}
                        onClick={() => {
                          setLabelValue(detail.labels.join(", "));
                          setLabelEdit(!labelEdit);
                        }}
                      >
                        <Tag size={17} />
                      </button>
                      {labelEdit && (
                        <form
                          className="label-popover"
                          onSubmit={(e) => {
                            e.preventDefault();
                            void flags(detail, {
                              labels: Array.from(
                                new Set(
                                  labelValue
                                    .split(",")
                                    .map((x) => x.trim())
                                    .filter(Boolean),
                                ),
                              ),
                            })
                              .then(() => setLabelEdit(false))
                              .catch((e) => notify(String(e), true));
                          }}
                        >
                          <label>
                            Labels <small>Separate with commas</small>
                            <input
                              autoFocus
                              value={labelValue}
                              onChange={(e) => setLabelValue(e.target.value)}
                            />
                          </label>
                          <button className="primary-button" type="submit">
                            Save labels
                          </button>
                        </form>
                      )}
                    </div>
                  </div>
                  <div>
                    {reading && <Loader2 size={15} className="spin" />}
                    <button
                      className="icon-button"
                      aria-label="Reply to message"
                      onClick={() =>
                        compose(respond(detail, "reply", identities))
                      }
                    >
                      <Reply size={18} />
                    </button>
                    <button
                      className="icon-button"
                      aria-label="Close reader"
                      onClick={resetSelection}
                    >
                      <PanelRightClose size={17} />
                    </button>
                  </div>
                </header>
                <div className="reader-scroll">
                  <div className="reader-content">
                    <div className="reader-eyebrow">
                      <span>{detail.folder.toUpperCase()}</span>
                      {detail.labels.map((l) => (
                        <span className="label-pill" key={l}>
                          {l}
                        </span>
                      ))}
                      <button
                        className="text-button plain-toggle"
                        onClick={() => setPlain(!plain)}
                      >
                        {plain ? "HTML view" : "Plain text"}
                      </button>
                    </div>
                    <h2 className="reader-subject">
                      {detail.subject || "(No subject)"}
                    </h2>
                    <div className="sender-details">
                      <span className={`avatar large ${colorFor(detail.from)}`}>
                        {initials(detail.from)}
                      </span>
                      <div>
                        <strong>{personName(detail.from)}</strong>
                        <span className="sender-email">
                          {emailAddress(detail.from)}
                        </span>
                        <details>
                          <summary>
                            to{" "}
                            {detail.to
                              .map((a) =>
                                emailAddress(a) === identity ? "me" : a,
                              )
                              .join(", ")}{" "}
                            <ChevronDown size={11} />
                          </summary>
                          <dl>
                            <dt>From</dt>
                            <dd>{detail.from}</dd>
                            <dt>To</dt>
                            <dd>{detail.to.join(", ")}</dd>
                            {!!detail.cc.length && (
                              <>
                                <dt>Cc</dt>
                                <dd>{detail.cc.join(", ")}</dd>
                              </>
                            )}
                            {!!detail.reply_to.length && (
                              <>
                                <dt>Reply-To</dt>
                                <dd>{detail.reply_to.join(", ")}</dd>
                              </>
                            )}
                            <dt>Date</dt>
                            <dd>
                              {new Date(detail.created_at).toLocaleString()}
                            </dd>
                            {detail.direction === "outgoing" && (
                              <>
                                <dt>Status</dt>
                                <dd>
                                  {detail.status === "accepted"
                                    ? "Accepted by Resend"
                                    : detail.status}
                                </dd>
                              </>
                            )}
                          </dl>
                        </details>
                      </div>
                      <time dateTime={detail.created_at}>
                        {prettyDate(detail.created_at)}
                      </time>
                    </div>
                    {thread.length > 1 && (
                      <details className="conversation">
                        <summary>
                          {thread.length} messages in this conversation
                        </summary>
                        {thread.map((m) => (
                          <button
                            key={m.id}
                            onClick={() => void openMessage(m)}
                          >
                            <span>{personName(m.from)}</span>
                            <time>{prettyDate(m.created_at)}</time>
                            {m.id === detail.id && <Check size={13} />}
                          </button>
                        ))}
                      </details>
                    )}
                    {readerError && (
                      <div className="inline-error" role="alert">
                        {readerError}
                        <button
                          className="text-button"
                          onClick={() => void openMessage(detail)}
                        >
                          Try again
                        </button>
                      </div>
                    )}
                    {!detail.hydrated && reading ? (
                      <div className="body-loading">
                        <Loader2 className="spin" size={20} /> Downloading
                        message…
                      </div>
                    ) : plain || !detail.html ? (
                      <div className="plain-message" dir="auto">
                        {detail.text || "No text body is available."}
                      </div>
                    ) : (
                      <iframe
                        className="mail-frame"
                        title="Email message content"
                        sandbox=""
                        referrerPolicy="no-referrer"
                        srcDoc={readerDocument(detail.html, dark)}
                      />
                    )}
                    {!!detail.links?.length && (
                      <details className="message-links">
                        <summary>
                          Links in this message ({detail.links.length})
                        </summary>
                        <p>
                          Review the actual destination before opening in your
                          browser.
                        </p>
                        {detail.links.map((url) => (
                          <button
                            key={url}
                            onClick={() =>
                              void command("open_external", { url }).catch(
                                (e) => notify(String(e), true),
                              )
                            }
                          >
                            <ArrowUpRight size={13} />
                            <span>{url}</span>
                          </button>
                        ))}
                      </details>
                    )}
                    {detail.attachments.length > 0 && (
                      <div className="reader-attachments">
                        <h4>
                          <Paperclip size={14} /> {detail.attachments.length}{" "}
                          attachment{detail.attachments.length === 1 ? "" : "s"}
                        </h4>
                        {detail.attachments.map((a) => (
                          <button
                            className="attachment-tile"
                            key={a.id}
                            onClick={() =>
                              void command<boolean>("save_attachment", {
                                messageId: detail.id,
                                attachmentId: a.id,
                              })
                                .then((saved) => {
                                  if (saved) notify("Attachment saved.");
                                })
                                .catch((e) => notify(String(e), true))
                            }
                          >
                            <FileText size={23} />
                            <span>
                              <strong>{a.filename}</strong>
                              <small>
                                {formatBytes(a.size)} ·{" "}
                                {a.cached
                                  ? "Available offline"
                                  : "Download on demand"}
                              </small>
                            </span>
                            <ArrowDown size={16} />
                          </button>
                        ))}
                      </div>
                    )}
                    <div className="reply-actions">
                      <button
                        className="secondary-button"
                        onClick={() =>
                          compose(respond(detail, "reply", identities))
                        }
                      >
                        <Reply size={16} /> Reply
                      </button>
                      <button
                        className="secondary-button"
                        onClick={() =>
                          compose(respond(detail, "replyAll", identities))
                        }
                      >
                        <ReplyAll size={16} /> Reply all
                      </button>
                      <button
                        className="text-button"
                        onClick={() =>
                          compose(respond(detail, "forward", identities))
                        }
                      >
                        <ArrowRight size={16} /> Forward
                      </button>
                    </div>
                    {detail.folder === "trash" && (
                      <button
                        className="danger-button permanent-delete"
                        onClick={async () => {
                          if (
                            await confirmAction(
                              "Permanently remove this local copy? Refresh will not bring it back. The provider copy remains in Resend.",
                            )
                          )
                            void command("permanently_remove", {
                              id: detail.id,
                            })
                              .then(() => {
                                resetSelection();
                                return reload();
                              })
                              .catch((e) => notify(String(e), true));
                        }}
                      >
                        Permanently remove local copy
                      </button>
                    )}
                  </div>
                </div>
                <footer className="reader-footer">
                  <ShieldCheck size={13} />
                  <span>
                    Remote content blocked. Read with a little peace of mind.
                  </span>
                  <span className="footer-dots">
                    <MoreHorizontal size={16} />
                  </span>
                </footer>
              </>
            ) : (
              <div className="welcome">
                <div className="welcome-illustration">
                  <svg
                    width="180"
                    height="150"
                    viewBox="0 0 180 150"
                    fill="none"
                    aria-hidden="true"
                  >
                    <path
                      d="M34 112h115M58 108V52a9 9 0 019-9h47a9 9 0 019 9v56"
                      stroke="currentColor"
                      strokeWidth="1.5"
                    />
                    <path
                      d="M51 57l40 29 40-29M51 57v51h80V57H51z"
                      fill="var(--panel)"
                      stroke="currentColor"
                      strokeWidth="1.5"
                    />
                    <path
                      d="M51 108l29-29m51 29l-29-29M74 31l4-9m36 10l5-8M40 82l-12-4m116 12l10-3"
                      stroke="currentColor"
                      strokeWidth="1.5"
                      strokeLinecap="round"
                    />
                    <path
                      d="M84 55h16m-16 7h9"
                      stroke="var(--accent)"
                      strokeWidth="2"
                      strokeLinecap="round"
                    />
                    <circle cx="146" cy="44" r="3" fill="var(--accent)" />
                  </svg>
                </div>
                <span className="eyebrow">YOUR DOMAINS. ONE INBOX.</span>
                <h2>
                  A little more room
                  <br />
                  for your correspondence.
                </h2>
                <p>
                  {profile?.connected
                    ? "Choose a message, catch up on a conversation, or put a new thought into words."
                    : "Bring your Resend mail into a calm, familiar workspace. Connected to your domains. Saved on your device."}
                </p>
                <button
                  className="primary-button"
                  onClick={() =>
                    profile?.connected ? compose() : setSettings(true)
                  }
                >
                  {profile?.connected ? (
                    <PenLine size={16} />
                  ) : (
                    <ShieldCheck size={16} />
                  )}{" "}
                  {profile?.connected
                    ? "Write a message"
                    : "Connect your Resend account"}
                </button>
                <div className="welcome-footnote">
                  <ShieldCheck size={14} /> Private by default. Local by design.
                </div>
              </div>
            )}
          </section>
        </div>
        <footer className="workspace-status">
          <span>
            {desktop ? "Resend Client" : "Browser preview"} <span>·</span> Local
            organization
          </span>
          <span>
            {snapshot?.pending_bodies
              ? `${snapshot.pending_bodies} bodies pending download`
              : snapshot?.history_complete
                ? "Available history imported"
                : "History import pending"}{" "}
            <CircleHelp size={12} />
          </span>
        </footer>
      </main>
      <Suspense
        fallback={
          <div className="overlay">
            <Loader2 size={25} className="spin" />
          </div>
        }
      >
        {settings && profile && (
          <Settings
            profile={profile}
            onClose={() => setSettings(false)}
            onChange={reload}
            notify={notify}
          />
        )}
        {composer && (
          <Composer
            key={composer.id}
            initial={composer}
            identities={identities}
            onClose={() => setComposer(null)}
            onSaved={() => void reload()}
            notify={notify}
          />
        )}
      </Suspense>
      {toast && (
        <div
          className={`toast ${toast.error ? "error" : ""}`}
          role={toast.error ? "alert" : "status"}
        >
          {toast.error ? <CircleHelp size={17} /> : <CheckCheck size={17} />}
          <span>{toast.text}</span>
          {toast.undo && <button onClick={toast.undo}>Undo</button>}
          <button
            className="icon-button"
            aria-label="Dismiss notification"
            onClick={() => setToast(null)}
          >
            <X size={15} />
          </button>
        </div>
      )}
    </div>
  );
}

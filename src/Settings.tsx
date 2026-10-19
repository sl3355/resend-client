import { useEffect, useRef, useState } from "react";
import {
  ArrowLeft,
  CheckCircle2,
  Download,
  KeyRound,
  Loader2,
  Plus,
  ShieldCheck,
  Trash2,
  Upload,
  X,
} from "lucide-react";
import { command, desktop, confirmAction } from "./backend";
import { uid } from "./mail";
import type { Identity, Probe, Profile } from "./types";
import { useDialogFocus } from "./useDialogFocus";

interface Props {
  profile: Profile;
  onClose: () => void;
  onChange: () => Promise<void>;
  notify: (s: string, error?: boolean) => void;
}
export default function Settings({
  profile,
  onClose,
  onChange,
  notify,
}: Props) {
  const [tab, setTab] = useState(
    profile.connected ? "identities" : "connection",
  );
  const [identities, setIdentities] = useState<Identity[]>(profile.identities);
  const [theme, setTheme] = useState(profile.settings.theme || "system");
  const [key, setKey] = useState("");
  const [probe, setProbe] = useState<Probe | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [dirty, setDirty] = useState(false);
  const panel = useRef<HTMLElement>(null);
  useDialogFocus(panel);
  useEffect(() => {
    panel.current?.querySelector<HTMLButtonElement>("button")?.focus();
  }, []);
  async function action(fn: () => Promise<void>) {
    setBusy(true);
    setError("");
    try {
      await fn();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  async function save() {
    await command("save_settings", { identities, settings: { theme } });
    setDirty(false);
    await onChange();
    notify("Settings saved on this device.");
  }
  async function close() {
    if (dirty) {
      await action(async () => {
        await save();
        onClose();
      });
    } else onClose();
  }
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        void close();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  });
  const patch = (id: string, changes: Partial<Identity>) => {
    setIdentities((list) =>
      list.map((i) => (i.id === id ? { ...i, ...changes } : i)),
    );
    setDirty(true);
  };
  return (
    <div className="overlay settings-overlay">
      <section
        className="settings-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="settings-title"
        ref={panel}
      >
        <header className="modal-header">
          <div>
            <span className="eyebrow">MAKE YOURSELF AT HOME</span>
            <h2 id="settings-title">Settings</h2>
          </div>
          <button
            className="icon-button"
            aria-label="Close settings"
            disabled={busy}
            onClick={() => void close()}
          >
            <X size={20} />
          </button>
        </header>
        <div className="settings-layout">
          <nav aria-label="Settings sections">
            {[
              ["connection", "Connection"],
              ["identities", "Sender identities"],
              ["appearance", "Appearance"],
              ["storage", "Local data"],
            ].map(([id, label]) => (
              <button
                key={id}
                className={tab === id ? "selected" : ""}
                onClick={() => setTab(id)}
              >
                {label}
              </button>
            ))}
            <div className="settings-nav-note">
              <ShieldCheck size={19} />
              <p>
                Your mail stays
                <br />
                on your device.
              </p>
            </div>
          </nav>
          <div className="settings-content">
            {tab === "connection" && (
              <>
                <h3>Your Resend account</h3>
                <p className="muted">
                  Connect the account you already use. Your domains and DNS stay
                  managed in Resend.
                </p>
                <div className="info-card">
                  <ShieldCheck size={20} />
                  <p>
                    This personal client can access team-wide mail. Folders,
                    read state, and drafts are saved only on this device.
                  </p>
                </div>
                {!desktop && (
                  <div className="info-card demo-note">
                    You’re exploring a browser demo. Connect real mail in the
                    desktop app with <code>npm run desktop</code>.
                  </div>
                )}
                {profile.connected ? (
                  <div className="connection-status">
                    <span className="status-dot" />
                    <div>
                      <strong>
                        {desktop ? "Connected securely" : "Demo workspace"}
                      </strong>
                      <p>
                        {desktop
                          ? "Credential stored in your OS credential store."
                          : "Synthetic correspondence · no connection to Resend."}
                      </p>
                    </div>
                  </div>
                ) : (
                  <>
                    <label className="stacked-label">
                      Resend API key
                      <div className="key-input">
                        <KeyRound size={16} />
                        <input
                          type="password"
                          autoComplete="off"
                          spellCheck={false}
                          placeholder="re_••••••••••••••••"
                          value={key}
                          onChange={(e) => setKey(e.target.value)}
                          disabled={!desktop}
                        />
                      </div>
                    </label>
                    <p className="form-help">
                      The key is saved to OS secure storage only after both read
                      checks pass. No test message is sent automatically.
                    </p>
                    <button
                      className="primary-button"
                      disabled={busy || !key || !desktop}
                      onClick={() =>
                        void action(async () => {
                          const result = await command<Probe>(
                            "connect_account",
                            { credential: key },
                          );
                          setKey("");
                          setProbe(result);
                          if (result.incoming && result.outgoing) {
                            await onChange();
                            notify(
                              "Connected. Add your sender identities, then refresh mail.",
                            );
                            setTab("identities");
                          }
                        })
                      }
                    >
                      {busy ? (
                        <Loader2 className="spin" size={16} />
                      ) : (
                        <ShieldCheck size={16} />
                      )}{" "}
                      Verify & connect
                    </button>
                  </>
                )}
                {probe && (
                  <div className="probe-results">
                    <p>
                      {probe.incoming ? "✓" : "×"} Incoming history:{" "}
                      {probe.incoming ? "accessible" : probe.incoming_error}
                    </p>
                    <p>
                      {probe.outgoing ? "✓" : "×"} Sent history:{" "}
                      {probe.outgoing ? "accessible" : probe.outgoing_error}
                    </p>
                  </div>
                )}
                <p className="form-help">
                  Reading sent history verifies read access. Sending permission
                  and sender verification are confirmed when you explicitly send
                  a message.
                </p>
                {profile.connected && desktop && (
                  <button
                    className="danger-button"
                    disabled={busy}
                    onClick={async () => {
                      if (
                        await confirmAction(
                          "Disconnect and start a fresh profile? Export a backup first. Existing local data is retained in the app storage, and provider mail is unaffected.",
                        )
                      )
                        void action(async () => {
                          await command("disconnect_account");
                          await onChange();
                          onClose();
                        });
                    }}
                  >
                    Disconnect & start fresh
                  </button>
                )}
              </>
            )}
            {tab === "identities" && (
              <>
                <div className="section-heading">
                  <div>
                    <h3>A home for every address</h3>
                    <p className="muted">
                      Use exact addresses on your configured domains.
                    </p>
                  </div>
                  <button
                    className="secondary-button"
                    onClick={() => {
                      setIdentities([
                        ...identities,
                        { id: uid(), name: "", email: "", signature: "" },
                      ]);
                      setDirty(true);
                    }}
                  >
                    <Plus size={15} /> Add identity
                  </button>
                </div>
                {!identities.length && (
                  <div className="settings-empty">
                    <p>Add your first sender identity.</p>
                    <small>
                      For example, you@yourdomain.com. No DNS changes are made.
                    </small>
                  </div>
                )}
                {identities.map((i, index) => (
                  <article className="identity-card" key={i.id}>
                    <div className="identity-card-header">
                      <span className="small-avatar">{i.name?.[0] || "@"}</span>
                      <strong>{i.email || `New identity ${index + 1}`}</strong>
                      {index === 0 && <span className="badge">Default</span>}
                      <button
                        className="icon-button"
                        aria-label={`Remove identity ${index + 1}`}
                        onClick={() => {
                          setIdentities(
                            identities.filter((x) => x.id !== i.id),
                          );
                          setDirty(true);
                        }}
                      >
                        <Trash2 size={15} />
                      </button>
                    </div>
                    <div className="two-fields">
                      <label className="stacked-label">
                        Display name
                        <input
                          value={i.name}
                          onChange={(e) =>
                            patch(i.id, { name: e.target.value })
                          }
                          placeholder="Your name"
                        />
                      </label>
                      <label className="stacked-label">
                        Email address
                        <input
                          type="email"
                          value={i.email}
                          onChange={(e) =>
                            patch(i.id, { email: e.target.value })
                          }
                          placeholder="you@yourdomain.com"
                        />
                      </label>
                    </div>
                    <label className="stacked-label">
                      Signature
                      <textarea
                        rows={3}
                        value={i.signature}
                        onChange={(e) =>
                          patch(i.id, { signature: e.target.value })
                        }
                        placeholder="A little sign-off"
                      />
                    </label>
                  </article>
                ))}
                <p className="form-help">
                  Identities filter your mail locally. They are not separate
                  accounts or access boundaries.
                </p>
              </>
            )}
            {tab === "appearance" && (
              <>
                <h3>A space that feels like you</h3>
                <p className="muted">Choose how your inbox looks.</p>
                <div className="theme-options">
                  {["light", "dark", "system"].map((value) => (
                    <button
                      key={value}
                      className={`theme-option ${theme === value ? "chosen" : ""}`}
                      aria-pressed={theme === value}
                      onClick={() => {
                        setTheme(value);
                        setDirty(true);
                      }}
                    >
                      <div className={`theme-preview ${value}`}>
                        <span />
                        <div>
                          <i />
                          <i />
                          <i />
                        </div>
                      </div>
                      <span>
                        {value[0].toUpperCase() + value.slice(1)}
                        {theme === value && <CheckCircle2 size={15} />}
                      </span>
                    </button>
                  ))}
                </div>
                <h4>Keyboard shortcuts</h4>
                <dl className="shortcuts">
                  <dt>Search mail</dt>
                  <dd>⌘ / Ctrl K</dd>
                  <dt>New message</dt>
                  <dd>N</dd>
                  <dt>Next / previous message</dt>
                  <dd>J / K</dd>
                  <dt>Save and close composer</dt>
                  <dd>Esc</dd>
                </dl>
                <p className="form-help">
                  Use your operating system’s zoom and accessibility settings.
                  All primary controls also work with Tab and Enter.
                </p>
              </>
            )}
            {tab === "storage" && (
              <>
                <h3>Your mail, kept close</h3>
                <p className="muted">
                  Downloaded messages, organization, and drafts are stored
                  locally in a private SQLite profile.
                </p>
                <div className="info-card">
                  <ShieldCheck size={20} />
                  <p>
                    The database is not encrypted by this app. Your OS account
                    and disk encryption protect local files. Backups contain
                    sensitive mail and exclude credentials.
                  </p>
                </div>
                <div className="storage-action">
                  <div>
                    <h4>Export a backup</h4>
                    <p>
                      Messages, local state, drafts, Outbox, and cached
                      attachments.
                    </p>
                  </div>
                  <button
                    className="secondary-button"
                    disabled={busy}
                    onClick={() =>
                      void action(async () => {
                        if (await command<boolean>("export_profile"))
                          notify("Backup exported. Credentials were excluded.");
                      })
                    }
                  >
                    <Download size={15} /> Export
                  </button>
                </div>
                <div className="storage-action">
                  <div>
                    <h4>Restore a backup</h4>
                    <p>
                      Validates the backup and opens a separate disconnected
                      profile. Use its original Resend account to reconnect.
                      Restored sends require review.
                    </p>
                  </div>
                  <button
                    className="secondary-button"
                    disabled={busy}
                    onClick={() =>
                      void action(async () => {
                        if (await command<boolean>("restore_profile")) {
                          await onChange();
                          notify("Backup restored into a separate profile.");
                          onClose();
                        }
                      })
                    }
                  >
                    <Upload size={15} /> Restore
                  </button>
                </div>
                <p className="form-help">
                  Background refresh runs while the app is open. Quitting or
                  sleeping pauses refresh and queued sends. Cached attachments
                  remain available offline; others need a connection.
                </p>
              </>
            )}
            {error && (
              <p className="inline-error" role="alert">
                {error}
              </p>
            )}
          </div>
        </div>
        <footer className="settings-footer">
          <button
            className="text-button"
            onClick={() => void close()}
            disabled={busy}
          >
            <ArrowLeft size={15} /> Back to mail
          </button>
          <button
            className="primary-button"
            disabled={busy || !dirty}
            onClick={() => void action(save)}
          >
            {busy && <Loader2 className="spin" size={15} />} Save changes
          </button>
        </footer>
      </section>
    </div>
  );
}

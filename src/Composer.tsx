import { useEffect, useRef, useState } from "react";
import { EditorContent, useEditor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import {
  Bold,
  Italic,
  List,
  ListOrdered,
  Paperclip,
  Send,
  Trash2,
  X,
  Check,
  Loader2,
  Undo2,
  Redo2,
  Quote,
} from "lucide-react";
import { command, desktop, confirmAction } from "./backend";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useDialogFocus } from "./useDialogFocus";
import { formatBytes, sanitize } from "./mail";
import type { Attachment, Draft, Identity } from "./types";

interface Props {
  initial: Draft;
  identities: Identity[];
  onClose: () => void;
  onSaved: () => void;
  notify: (message: string, error?: boolean) => void;
}
export default function Composer({
  initial,
  identities,
  onClose,
  onSaved,
  notify,
}: Props) {
  const [draft, setDraft] = useState<Draft>(initial);
  const current = useRef(draft);
  const committed = useRef(0);
  const saveChain = useRef(Promise.resolve());
  const [saveStatus, setSaveStatus] = useState("Saving…");
  const [busy, setBusy] = useState(false);
  const [showCc, setShowCc] = useState(!!draft.cc || !!draft.bcc);
  const [emptySubject, setEmptySubject] = useState(false);
  const [error, setError] = useState("");
  const panel = useRef<HTMLElement>(null);
  useDialogFocus(panel);
  const update = (patch: Partial<Draft>) =>
    setDraft((d) => {
      const next = {
        ...d,
        ...patch,
        revision: d.revision + 1,
        updated_at: new Date().toISOString(),
      };
      current.current = next;
      setSaveStatus("Unsaved changes");
      return next;
    });
  const editor = useEditor({
    extensions: [
      StarterKit.configure({ heading: { levels: [1, 2, 3] }, link: false }),
    ],
    content: sanitize(initial.html || "<p></p>"),
    editorProps: {
      transformPastedHTML: sanitize,
      attributes: {
        "aria-label": "Message body",
        role: "textbox",
        "aria-multiline": "true",
      },
    },
    onUpdate: ({ editor }) =>
      update({ html: editor.getHTML(), text: editor.getText() }),
  });
  function flush(): Promise<void> {
    const snapshot = structuredClone(current.current);
    const task = saveChain.current
      .catch(() => {})
      .then(async () => {
        if (committed.current >= snapshot.revision) return;
        setSaveStatus("Saving…");
        try {
          await command("save_draft", { draft: snapshot });
          committed.current = snapshot.revision;
          if (current.current.revision === snapshot.revision)
            setSaveStatus("Saved on this device");
        } catch (e) {
          setSaveStatus("Save failed");
          setError(String(e));
          throw e;
        }
      });
    saveChain.current = task;
    return task;
  }
  useEffect(() => {
    current.current = draft;
    const timer = setTimeout(() => {
      void flush().catch(() => {});
    }, 650);
    return () => clearTimeout(timer);
  }, [draft]);
  useEffect(() => {
    const handler = (event: BeforeUnloadEvent) => {
      if (committed.current < current.current.revision) {
        event.preventDefault();
        void flush().catch(() => {});
      }
    };
    window.addEventListener("beforeunload", handler);
    return () => window.removeEventListener("beforeunload", handler);
  }, []);
  useEffect(() => {
    if (!desktop) return;
    let dispose: (() => void) | undefined;
    let gone = false;
    void getCurrentWindow()
      .onCloseRequested(async (e) => {
        e.preventDefault();
        try {
          await flush();
          await getCurrentWindow().destroy();
        } catch (err) {
          setError(String(err));
        }
      })
      .then((fn) => {
        if (gone) fn();
        else dispose = fn;
      });
    return () => {
      gone = true;
      dispose?.();
    };
  }, []);
  useEffect(() => {
    editor?.setEditable(!busy);
  }, [busy, editor]);
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
  async function close() {
    if (busy) return;
    setBusy(true);
    try {
      await flush();
      onSaved();
      onClose();
    } catch {
      setBusy(false);
    }
  }
  async function send(confirmed = false) {
    if (!draft.to.trim()) {
      setError("Add at least one To recipient.");
      return;
    }
    if (!draft.identity_id) {
      setError("Choose a sender identity in Settings.");
      return;
    }
    if (!draft.subject.trim() && !confirmed) {
      setEmptySubject(true);
      return;
    }
    setBusy(true);
    setError("");
    try {
      await flush();
      await command("enqueue_send", { draft: current.current });
      notify(
        desktop
          ? "Message queued. Follow its progress in Outbox."
          : "Queued in the demo. No email will be sent.",
      );
      onSaved();
      onClose();
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  }
  async function attach() {
    try {
      const attachments = await command<Attachment[]>("attach_files");
      update({ attachments: [...current.current.attachments, ...attachments] });
    } catch (e) {
      setError(String(e));
    }
  }
  async function discard() {
    if (!(await confirmAction("Discard this local draft?"))) return;
    setBusy(true);
    try {
      await saveChain.current.catch(() => {});
      await command("delete_draft", { id: draft.id });
      onSaved();
      onClose();
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  }
  const sender = identities.find((i) => i.id === draft.identity_id);
  return (
    <div className="overlay compose-overlay" role="presentation">
      <section
        ref={panel}
        className="composer"
        role="dialog"
        aria-modal="true"
        aria-labelledby="compose-title"
      >
        <header className="modal-header">
          <div>
            <span className="eyebrow">A LITTLE CORRESPONDENCE</span>
            <h2 id="compose-title">
              {draft.headers["In-Reply-To"] ? "Write a reply" : "New message"}
            </h2>
          </div>
          <button
            className="icon-button"
            aria-label="Save and close composer"
            onClick={() => void close()}
            disabled={busy}
          >
            <X size={20} />
          </button>
        </header>
        <div className="compose-fields" inert={busy}>
          <label className="field-line">
            <span>From</span>
            <select
              aria-label="From identity"
              value={draft.identity_id}
              onChange={(e) => update({ identity_id: e.target.value })}
            >
              <option value="" disabled>
                Choose an identity
              </option>
              {identities.map((i) => (
                <option key={i.id} value={i.id}>
                  {i.name ? `${i.name} <${i.email}>` : i.email}
                </option>
              ))}
            </select>
          </label>
          <div className="field-line">
            <label htmlFor="compose-to">To</label>
            <input
              id="compose-to"
              autoFocus
              placeholder="name@example.com"
              value={draft.to}
              onChange={(e) => update({ to: e.target.value })}
            />
            <button
              className="text-button"
              onClick={() => setShowCc(!showCc)}
              aria-expanded={showCc}
            >
              Cc / Bcc
            </button>
          </div>
          {showCc && (
            <>
              <label className="field-line">
                <span>Cc</span>
                <input
                  value={draft.cc}
                  onChange={(e) => update({ cc: e.target.value })}
                  placeholder="Copy recipients"
                />
              </label>
              <label className="field-line">
                <span>Bcc</span>
                <input
                  value={draft.bcc}
                  onChange={(e) => update({ bcc: e.target.value })}
                  placeholder="Hidden recipients"
                />
              </label>
            </>
          )}
          <label className="field-line subject-field">
            <span>Subject</span>
            <input
              placeholder="A good place to start"
              value={draft.subject}
              onChange={(e) => {
                update({ subject: e.target.value });
                setEmptySubject(false);
              }}
            />
          </label>
        </div>
        <div className="format-toolbar" role="toolbar" aria-label="Formatting">
          <button
            className={`icon-button ${editor?.isActive("bold") ? "active" : ""}`}
            aria-label="Bold"
            aria-pressed={editor?.isActive("bold")}
            onClick={() => editor?.chain().focus().toggleBold().run()}
          >
            <Bold size={16} />
          </button>
          <button
            className={`icon-button ${editor?.isActive("italic") ? "active" : ""}`}
            aria-label="Italic"
            aria-pressed={editor?.isActive("italic")}
            onClick={() => editor?.chain().focus().toggleItalic().run()}
          >
            <Italic size={16} />
          </button>
          <span className="divider" />
          <button
            className="icon-button"
            aria-label="Bullet list"
            onClick={() => editor?.chain().focus().toggleBulletList().run()}
          >
            <List size={17} />
          </button>
          <button
            className="icon-button"
            aria-label="Numbered list"
            onClick={() => editor?.chain().focus().toggleOrderedList().run()}
          >
            <ListOrdered size={17} />
          </button>
          <button
            className="icon-button"
            aria-label="Quote"
            onClick={() => editor?.chain().focus().toggleBlockquote().run()}
          >
            <Quote size={16} />
          </button>
          <span className="divider" />
          <button
            className="icon-button"
            aria-label="Undo typing"
            onClick={() => editor?.chain().focus().undo().run()}
          >
            <Undo2 size={16} />
          </button>
          <button
            className="icon-button"
            aria-label="Redo typing"
            onClick={() => editor?.chain().focus().redo().run()}
          >
            <Redo2 size={16} />
          </button>
        </div>
        <div className="editor-scroll" inert={busy}>
          <EditorContent editor={editor} />
          {sender?.signature && (
            <div className="signature-preview">
              <span>Signature · {sender.email}</span>
              <p>{sender.signature}</p>
            </div>
          )}
        </div>
        {draft.attachments.length > 0 && (
          <div className="compose-attachments">
            {draft.attachments.map((a) => (
              <div className="attachment-chip" key={a.id}>
                <Paperclip size={14} />
                <span>
                  {a.filename}
                  <small>{formatBytes(a.size)}</small>
                </span>
                <button
                  className="icon-button"
                  aria-label={`Remove ${a.filename}`}
                  onClick={() =>
                    update({
                      attachments: draft.attachments.filter(
                        (x) => x.id !== a.id,
                      ),
                    })
                  }
                >
                  <X size={14} />
                </button>
              </div>
            ))}
          </div>
        )}
        {error && (
          <p className="inline-error" role="alert">
            {error}
          </p>
        )}
        {emptySubject && (
          <div className="inline-confirm" role="alert">
            <span>Send this message without a subject?</span>
            <button onClick={() => void send(true)}>Send anyway</button>
            <button onClick={() => setEmptySubject(false)}>Keep editing</button>
          </div>
        )}
        <footer className="composer-footer">
          <button
            className="primary-button"
            disabled={busy}
            onClick={() => void send()}
          >
            {busy ? <Loader2 className="spin" size={16} /> : <Send size={16} />}{" "}
            {desktop ? "Send message" : "Queue demo message"}
          </button>
          <button
            className="icon-button"
            aria-label="Attach files"
            onClick={() => void attach()}
            disabled={busy}
          >
            <Paperclip size={19} />
          </button>
          <span
            className={`draft-status ${saveStatus === "Save failed" ? "error-text" : ""}`}
            aria-live="polite"
          >
            {saveStatus === "Saved on this device" && <Check size={13} />}{" "}
            {saveStatus}
          </span>
          <button
            className="icon-button"
            aria-label="Discard draft"
            onClick={() => void discard()}
            disabled={busy}
          >
            <Trash2 size={17} />
          </button>
        </footer>
      </section>
    </div>
  );
}

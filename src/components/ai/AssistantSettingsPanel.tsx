import { useEffect, useMemo, useRef, useState } from "react";
import {
  assistantActivateStyleProfile,
  assistantCancelChatGptLogin,
  assistantDeleteStyleProfile,
  assistantDisconnectSubscription,
  assistantGetSettings,
  assistantGetSubscriptionStatus,
  assistantListStyleProfiles,
  assistantPreviewStyleProfile,
  assistantProcessQueue,
  assistantStartChatGptLogin,
  assistantStartStyleLearning,
  assistantStyleLearningPreview,
  assistantUpdateSettings,
  assistantUpdateStyleProfile,
} from "@/ipc/assistant";
import type {
  AssistantSettings,
  StyleLearningPreview,
  StylePattern,
  StyleProfile,
  StyleProfileContent,
  StyleProfilePreviewText,
  SubscriptionStatus,
} from "@/ipc/types";
import { useUiStore } from "@/store/uiStore";

type LearningCount = 100 | 200;

const copyContent = (content: StyleProfileContent): StyleProfileContent =>
  JSON.parse(JSON.stringify(content)) as StyleProfileContent;

const lines = (value: string) =>
  value
    .split("\n")
    .map((item) => item.trim())
    .filter(Boolean);

const patternLines = (items: StylePattern[]) => items.map((item) => item.pattern).join("\n");
const patterns = (value: string): StylePattern[] =>
  lines(value).map((pattern) => ({ pattern, usage: "本人編集" }));

const dateLabel = (value: number | null) =>
  value == null ? "未確認" : new Date(value * 1000).toLocaleDateString("ja-JP");

const connectionLabel: Record<SubscriptionStatus["connection"], string> = {
  unavailable: "Codexなし",
  disconnected: "未接続",
  connecting: "接続中",
  connected: "接続済み",
  error: "要確認",
};

const safeAuthUrl = (value: string | undefined) => {
  if (!value) return null;
  try {
    const url = new URL(value);
    const host = url.hostname;
    return url.protocol === "https:" &&
      (host === "chatgpt.com" || host.endsWith(".chatgpt.com") ||
        host === "openai.com" || host.endsWith(".openai.com"))
      ? value
      : null;
  } catch {
    return null;
  }
};

const rateLabel = (window: SubscriptionStatus["primary"]) => {
  if (!window || window.used_percent == null) return "未確認";
  const reset = window.resets_at == null
    ? "リセット時刻 未確認"
    : `${new Date(window.resets_at * 1000).toLocaleString("ja-JP")} リセット`;
  return `${Math.round(window.used_percent)}% 使用 · ${reset}`;
};

/**
 * Automatic drafts remain opt-in. Style learning is not a toggle: it always
 * starts from an explicit 100/200-message preview and creates an editable draft.
 */
export function AssistantSettingsPanel() {
  const accountId = useUiStore((state) => state.selectedAccountId);
  const [settings, setSettings] = useState<AssistantSettings | null>(null);
  const [subscription, setSubscription] = useState<SubscriptionStatus | null>(null);
  const [profiles, setProfiles] = useState<StyleProfile[]>([]);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [editing, setEditing] = useState<StyleProfileContent | null>(null);
  const [learningCount, setLearningCount] = useState<LearningCount>(100);
  const [learningPreview, setLearningPreview] = useState<StyleLearningPreview | null>(null);
  const [learningConsent, setLearningConsent] = useState(false);
  const [textPreview, setTextPreview] = useState<StyleProfilePreviewText | null>(null);
  const [queue, setQueue] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const request = useRef(0);

  const selected = useMemo(
    () => profiles.find((profile) => profile.id === selectedId) ?? null,
    [profiles, selectedId],
  );
  const dirty = Boolean(
    selected && editing && JSON.stringify(selected.content) !== JSON.stringify(editing),
  );

  const chooseProfile = (profile: StyleProfile | null) => {
    setSelectedId(profile?.id ?? null);
    setEditing(profile ? copyContent(profile.content) : null);
    setTextPreview(null);
  };

  const load = async (preferredId?: number) => {
    if (accountId == null) return;
    const current = ++request.current;
    setError(null);
    try {
      const [nextSettings, nextProfiles, nextSubscription] = await Promise.all([
        assistantGetSettings({ account_id: accountId }),
        assistantListStyleProfiles({ account_id: accountId }),
        assistantGetSubscriptionStatus(),
      ]);
      if (request.current !== current) return;
      setSettings(nextSettings);
      setSubscription(nextSubscription);
      setProfiles(nextProfiles);
      const preferred =
        nextProfiles.find((profile) => profile.id === preferredId) ??
        nextProfiles.find((profile) => profile.status === "draft") ??
        nextProfiles.find((profile) => profile.status === "active") ??
        nextProfiles[0] ??
        null;
      chooseProfile(preferred);
    } catch (cause) {
      if (request.current === current) {
        setError(cause instanceof Error ? cause.message : "設定を読み込めませんでした");
      }
    }
  };

  useEffect(() => {
    setLearningPreview(null);
    setLearningConsent(false);
    void load();
    return () => {
      request.current += 1;
    };
  }, [accountId]);

  useEffect(() => {
    if (subscription?.connection !== "connecting") return;
    let active = true;
    const timer = window.setInterval(() => {
      void assistantGetSubscriptionStatus()
        .then((next) => {
          if (active) setSubscription(next);
        })
        .catch(() => {
          if (active) setError("ChatGPT接続状態を確認できませんでした");
        });
    }, 1200);
    return () => {
      active = false;
      window.clearInterval(timer);
    };
  }, [subscription?.connection]);

  if (accountId == null || !settings) return null;

  const updateSettings = async (patch: Partial<AssistantSettings>) => {
    setBusy("settings");
    setError(null);
    try {
      const next = await assistantUpdateSettings({
        account_id: accountId,
        auto_draft_enabled: patch.auto_draft_enabled ?? settings.auto_draft_enabled,
        subscription_reply_enabled:
          patch.subscription_reply_enabled ?? settings.subscription_reply_enabled,
        daily_generation_limit:
          patch.daily_generation_limit ?? settings.daily_generation_limit,
      });
      setSettings(next);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "設定を保存できませんでした");
    } finally {
      setBusy(null);
    }
  };

  const startSubscriptionLogin = async () => {
    setBusy("subscription-login");
    setError(null);
    try {
      const requestId = crypto.randomUUID();
      const login = await assistantStartChatGptLogin({ request_id: requestId });
      setSubscription((current) => current && ({
        ...current,
        connection: "connecting",
        login,
        message: "ブラウザでChatGPTへの接続を完了してください。",
      }));
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "ChatGPT接続を開始できませんでした");
      setSubscription(await assistantGetSubscriptionStatus().catch(() => subscription));
    } finally {
      setBusy(null);
    }
  };

  const cancelSubscriptionLogin = async () => {
    const login = subscription?.login;
    if (!login) return;
    setBusy("subscription-cancel");
    setError(null);
    try {
      await assistantCancelChatGptLogin({
        request_id: login.request_id,
        login_id: login.login_id,
      });
      setSubscription(await assistantGetSubscriptionStatus());
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "ChatGPT接続を中止できませんでした");
    } finally {
      setBusy(null);
    }
  };

  const disconnectSubscription = async () => {
    if (!window.confirm("A-mailだけでなく、このMacのCodex CLIなどもChatGPTからログアウトします。接続を解除しますか？")) return;
    setBusy("subscription-disconnect");
    setError(null);
    try {
      const next = await assistantDisconnectSubscription({ confirm_shared_logout: true });
      setSubscription(next);
      setSettings((current) => current && ({ ...current, subscription_reply_enabled: false }));
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "ChatGPT接続を解除できませんでした");
    } finally {
      setBusy(null);
    }
  };

  const processQueue = async () => {
    if (busy || !settings.auto_draft_enabled) return;
    setBusy("queue");
    setError(null);
    try {
      const result = await assistantProcessQueue({ account_id: accountId });
      setQueue(
        `処理 ${result.processed} / 待機 ${result.pending}${
          result.reason ? `: ${result.reason}` : ""
        }`,
      );
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "自動下書きの実行に失敗しました");
    } finally {
      setBusy(null);
    }
  };

  const inspectLearning = async () => {
    setBusy("learning-preview");
    setError(null);
    setLearningConsent(false);
    try {
      setLearningPreview(
        await assistantStyleLearningPreview({
          account_id: accountId,
          requested_count: learningCount,
        }),
      );
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "対象メールを確認できませんでした");
    } finally {
      setBusy(null);
    }
  };

  const startLearning = async () => {
    if (!learningPreview || !learningConsent || learningPreview.selected_count === 0) return;
    setBusy("learning-start");
    setError(null);
    try {
      const result = await assistantStartStyleLearning({
        account_id: accountId,
        requested_count: learningPreview.requested_count,
        consent_version: learningPreview.consent_version,
        preview_hash: learningPreview.preview_hash,
      });
      setLearningPreview(null);
      setLearningConsent(false);
      await load(result.profile.id);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "文体プロファイルを作成できませんでした");
    } finally {
      setBusy(null);
    }
  };

  const saveProfile = async () => {
    if (!selected || !editing) return;
    setBusy("profile-save");
    setError(null);
    try {
      const profile = await assistantUpdateStyleProfile({
        account_id: accountId,
        profile_id: selected.id,
        expected_updated_at: selected.updated_at,
        content: editing,
      });
      await load(profile.id);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "文体プロファイルを保存できませんでした");
    } finally {
      setBusy(null);
    }
  };

  const activateProfile = async () => {
    if (!selected || dirty) return;
    setBusy("profile-activate");
    setError(null);
    try {
      const profile = await assistantActivateStyleProfile({
        account_id: accountId,
        profile_id: selected.id,
      });
      await load(profile.id);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "文体プロファイルを有効にできませんでした");
    } finally {
      setBusy(null);
    }
  };

  const previewProfile = async () => {
    if (!selected || dirty) return;
    setBusy("profile-preview");
    setError(null);
    try {
      setTextPreview(
        await assistantPreviewStyleProfile({
          account_id: accountId,
          profile_id: selected.id,
          scenario_id: "inquiry",
        }),
      );
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "文体プレビューを作成できませんでした");
    } finally {
      setBusy(null);
    }
  };

  const deleteProfile = async () => {
    if (!selected) return;
    if (!window.confirm(`文体プロファイル v${selected.version} を削除しますか？`)) return;
    setBusy("profile-delete");
    setError(null);
    try {
      await assistantDeleteStyleProfile({ account_id: accountId, profile_id: selected.id });
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "文体プロファイルを削除できませんでした");
    } finally {
      setBusy(null);
    }
  };

  const setContent = (patch: Partial<StyleProfileContent>) => {
    if (editing) setEditing({ ...editing, ...patch });
  };

  const exclusionTotal = learningPreview
    ? Object.values(learningPreview.exclusions).reduce((sum, value) => sum + value, 0)
    : 0;

  return (
    <details className="ai-sec assistant-settings">
      <summary>自動下書き・自分の文体</summary>

      <section className="assistant-settings-block subscription-settings">
        <div className="subscription-heading">
          <h3>AI接続</h3>
          <span className={`subscription-badge ${subscription?.connection ?? "unavailable"}`}>
            {subscription ? connectionLabel[subscription.connection] : "確認中"}
          </span>
        </div>
        <p className="muted assistant-note">
          ChatGPT/Codexの対応サブスクリプション枠を使います。APIキーはA-mailへ登録しません。
          接続だけでは本文を送りません。設定のAI補助と本文共有も本人がONにした場合だけ生成します。
        </p>
        {subscription && (
          <dl className="subscription-facts">
            <div><dt>実行環境</dt><dd>{subscription.runtime_version ?? (subscription.runtime_available ? "検出済み" : "未検出")}</dd></div>
            <div><dt>プラン</dt><dd>{subscription.plan_type ?? "未確認"}</dd></div>
            <div><dt>短期枠</dt><dd>{rateLabel(subscription.primary)}</dd></div>
            <div><dt>長期枠</dt><dd>{rateLabel(subscription.secondary)}</dd></div>
          </dl>
        )}
        {subscription?.message && <p className="assistant-note">{subscription.message}</p>}
        {!subscription?.release_enabled && (
          <p className="cmp-status err">隔離受入前のため、このビルドでは実メールへの利用を停止しています。</p>
        )}
        <div className="subscription-actions">
          {(subscription?.connection === "disconnected" || subscription?.connection === "error") && (
            <button
              className="iconbtn"
              disabled={!subscription.release_enabled || busy != null}
              onClick={() => void startSubscriptionLogin()}
            >
              {busy === "subscription-login" ? "接続準備中…" : "ChatGPTで接続"}
            </button>
          )}
          {subscription?.connection === "connecting" && subscription.login && (
            <>
              {safeAuthUrl(subscription.login.auth_url) ? (
                <a className="iconbtn subscription-link" href={safeAuthUrl(subscription.login.auth_url)!} target="_blank" rel="noreferrer">認証画面を開く</a>
              ) : (
                <span className="muted assistant-note">架空デモのため認証画面は開きません。</span>
              )}
              <button className="iconbtn" disabled={busy != null} onClick={() => void cancelSubscriptionLogin()}>
                {busy === "subscription-cancel" ? "中止中…" : "接続を中止"}
              </button>
            </>
          )}
          {subscription?.connection === "connected" && (
            <button className="iconbtn danger" disabled={busy != null} onClick={() => void disconnectSubscription()}>
              {busy === "subscription-disconnect" ? "解除中…" : "接続を解除"}
            </button>
          )}
        </div>
        <label className="assistant-check">
          <input
            type="checkbox"
            checked={settings.subscription_reply_enabled}
            disabled={busy === "settings" || (
              !settings.subscription_reply_enabled &&
              (subscription?.connection !== "connected" || !subscription.release_enabled)
            )}
            onChange={(event) => void updateSettings({ subscription_reply_enabled: event.target.checked })}
          />
          返信下書きにこのChatGPT接続を使う（失敗時の自動切替なし）
        </label>
        <p className="assistant-safety-callout">AIにメール送信権限はありません。生成候補を本人が確認・反映した後だけ、通常の送信操作へ進めます。</p>
      </section>

      <section className="assistant-settings-block">
        <h3>自動下書き</h3>
        <label className="assistant-check">
          <input
            type="checkbox"
            checked={settings.auto_draft_enabled}
            disabled={busy === "settings"}
            onChange={(event) =>
              void updateSettings({ auto_draft_enabled: event.target.checked })
            }
          />
          A-mail起動中の新着から、未送信の候補だけを作る
        </label>
        <label className="assistant-number">
          1日の上限
          <input
            type="number"
            min="1"
            max="100"
            value={settings.daily_generation_limit}
            onChange={(event) =>
              void updateSettings({
                daily_generation_limit: Math.max(
                  1,
                  Math.min(100, Number(event.target.value) || 1),
                ),
              })
            }
          />
        </label>
        <p className="muted assistant-note">
          {settings.auto_generated_today}/{settings.daily_generation_limit}件（UTC {settings.quota_day}）
          · {queue ?? "待機理由は未確認"}
        </p>
        <button
          className="iconbtn"
          disabled={!settings.auto_draft_enabled || busy != null}
          onClick={() => void processQueue()}
        >
          {busy === "queue" ? "候補を生成中…" : "待機中の候補を生成"}
        </button>
      </section>

      <section className="assistant-settings-block">
        <h3>自分の文体</h3>
        <p className="muted assistant-note">
          自動では学習しません。本人が選んだ送信済みメールから、言い回し・文章量・段落・改行だけを抽象化します。
        </p>
        <div className="style-learning-controls">
          <select
            aria-label="文体学習に使う件数"
            value={learningCount}
            onChange={(event) => {
              setLearningCount(Number(event.target.value) as LearningCount);
              setLearningPreview(null);
              setLearningConsent(false);
            }}
          >
            <option value={100}>直近100件</option>
            <option value={200}>直近200件</option>
          </select>
          <button
            className="iconbtn"
            disabled={busy != null}
            onClick={() => void inspectLearning()}
          >
            {busy === "learning-preview" ? "確認中…" : "対象を確認"}
          </button>
        </div>

        {learningPreview && (
          <div className="style-learning-review" role="region" aria-label="文体学習の確認">
            <strong>学習前の確認</strong>
            <dl>
              <div><dt>使う本文</dt><dd>{learningPreview.selected_count}件 / 希望{learningPreview.requested_count}件</dd></div>
              <div><dt>期間</dt><dd>{dateLabel(learningPreview.source_range_start)}〜{dateLabel(learningPreview.source_range_end)}</dd></div>
              <div><dt>除外</dt><dd>{exclusionTotal}件（未取得・別送信者・引用・短文・重複）</dd></div>
              <div><dt>このMacで集計</dt><dd>{learningPreview.locally_aggregated_count}件</dd></div>
              <div><dt>外部AIへ送信</dt><dd>{learningPreview.ai_sample_count}件 / {learningPreview.ai_input_chars}文字</dd></div>
            </dl>
            <p>{learningPreview.notice}</p>
            {learningPreview.selected_count < learningPreview.requested_count && (
              <p className="cmp-status err">取得済みの本人本文が希望件数に足りません。別accountや受信文では補いません。</p>
            )}
            <label className="assistant-check">
              <input
                type="checkbox"
                checked={learningConsent}
                onChange={(event) => setLearningConsent(event.target.checked)}
              />
              対象件数と保存内容を確認しました
            </label>
            <button
              className="iconbtn"
              disabled={!learningConsent || learningPreview.selected_count === 0 || busy != null}
              onClick={() => void startLearning()}
            >
              {busy === "learning-start" ? "プロファイル作成中…" : "文体プロファイルを作成"}
            </button>
          </div>
        )}

        {profiles.length === 0 ? (
          <p className="muted assistant-note">文体プロファイルはまだありません。</p>
        ) : (
          <div className="style-profile-tabs" role="list" aria-label="文体プロファイルの版">
            {profiles.map((profile) => (
              <button
                key={profile.id}
                type="button"
                className={profile.id === selectedId ? "selected" : ""}
                onClick={() => chooseProfile(profile)}
              >
                v{profile.version} · {profile.status === "active" ? "使用中" : profile.status === "draft" ? "編集中" : "旧版"}
              </button>
            ))}
          </div>
        )}

        {selected && editing && (
          <div className="style-profile-editor">
            <p className="muted assistant-note">
              {selected.source_count}件 · {dateLabel(selected.source_range_start)}〜{dateLabel(selected.source_range_end)} · {selected.provider}/{selected.model}
            </p>
            <label>
              丁寧さ
              <select
                value={editing.honorific_level}
                onChange={(event) =>
                  setContent({ honorific_level: event.target.value as StyleProfileContent["honorific_level"] })
                }
              >
                <option value="plain">簡潔</option>
                <option value="polite">丁寧</option>
                <option value="very_polite">とても丁寧</option>
              </select>
            </label>
            <label>
              よく使う挨拶（1行1件）
              <textarea
                value={patternLines(editing.greetings)}
                onChange={(event) => setContent({ greetings: patterns(event.target.value) })}
              />
            </label>
            <label>
              よく使う結び（1行1件）
              <textarea
                value={patternLines(editing.closings)}
                onChange={(event) => setContent({ closings: patterns(event.target.value) })}
              />
            </label>
            <label>
              文体の特徴（1行1件）
              <textarea
                value={editing.tone_notes.join("\n")}
                onChange={(event) => setContent({ tone_notes: lines(event.target.value) })}
              />
            </label>
            <label className="assistant-check">
              <input
                type="checkbox"
                checked={editing.paragraphs.blank_line_between}
                onChange={(event) =>
                  setContent({
                    paragraphs: { ...editing.paragraphs, blank_line_between: event.target.checked },
                  })
                }
              />
              段落の間に空行を入れる
            </label>
            <label>
              本人による微修正
              <textarea
                value={editing.freeform_instructions}
                placeholder="例：結論を先に書き、最後は簡潔にする"
                onChange={(event) => setContent({ freeform_instructions: event.target.value })}
              />
            </label>
            <div className="style-profile-actions">
              <button className="iconbtn" disabled={!dirty || busy != null} onClick={() => void saveProfile()}>
                {busy === "profile-save" ? "保存中…" : "変更を保存"}
              </button>
              <button className="iconbtn" disabled={dirty || busy != null} onClick={() => void previewProfile()}>
                架空メールで確認
              </button>
              <button className="iconbtn" disabled={dirty || selected.status === "active" || busy != null} onClick={() => void activateProfile()}>
                {selected.status === "active" ? "使用中" : "この文体を使う"}
              </button>
              <button className="iconbtn danger" disabled={busy != null} onClick={() => void deleteProfile()}>
                削除
              </button>
            </div>
            {dirty && <p className="cmp-status err">未保存の変更があります。有効化・プレビューの前に保存してください。</p>}
            {textPreview && (
              <div className="style-profile-preview">
                <strong>{textPreview.subject}</strong>
                <pre>{textPreview.body_text}</pre>
                <p>{textPreview.notice}</p>
              </div>
            )}
          </div>
        )}
      </section>

      <p className="assistant-runtime-note">AIは下書き候補まで。送信は本人操作だけです。同期・生成・学習はA-mail起動中に限ります。</p>
      {error && <p className="cmp-status err" role="alert">{error}</p>}
    </details>
  );
}

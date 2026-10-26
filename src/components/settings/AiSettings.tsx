import { useRef, useState } from "react";
import { useAiStore } from "@/store/aiStore";
import { isTauri } from "@/ipc/client";
import {
  hasGeminiKey,
  hasOpenAiKey,
  setGeminiKey,
  setOpenAiKey,
  updateSettings,
} from "@/ipc/ai";
import type { AiProvider } from "@/ipc/types";

/**
 * AI 設定(オプトイン・送信範囲・キー登録)。
 *
 * プライバシー方針: AI 補助は既定 OFF。明示的にオンにしたときだけメール本文を
 * 選択中プロバイダーに送信する。API キーは Keychain に保存し、値はフロントへ返さない。
 *
 * M0 は骨組み。設定画面への導線(⌘,)は後続で配線する。
 */
export function AiSettings() {
  const aiEnabled = useAiStore((s) => s.aiEnabled);
  const setAiEnabled = useAiStore((s) => s.setAiEnabled);
  const hasKey = useAiStore((s) => s.hasKey);
  const setHasKey = useAiStore((s) => s.setHasKey);
  const provider = useAiStore((s) => s.provider);
  const model = useAiStore((s) => s.model);
  const setProvider = useAiStore((s) => s.setProvider);
  const projects = useAiStore((s) => s.projects);
  const projectsLoading = useAiStore((s) => s.projectsLoading);
  const projectsError = useAiStore((s) => s.projectsError);
  const addProject = useAiStore((s) => s.addProject);
  const removeProject = useAiStore((s) => s.removeProject);

  const [keyInput, setKeyInput] = useState("");
  const [saving, setSaving] = useState(false);
  const [settingsSaving, setSettingsSaving] = useState(false);
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const providerRequest = useRef(0);
  const [projectName, setProjectName] = useState("");
  const [projectPath, setProjectPath] = useState("");
  const [projectValidation, setProjectValidation] = useState<string | null>(null);
  const [projectStatus, setProjectStatus] = useState<string | null>(null);

  const onToggleAi = async (v: boolean) => {
    const previous = useAiStore.getState().aiEnabled;
    setSettingsError(null);
    setAiEnabled(v);
    if (!isTauri()) return;
    setSettingsSaving(true);
    try {
      await updateSettings({ ai_enabled: v });
    } catch (err) {
      setAiEnabled(previous);
      setSettingsError(err instanceof Error ? err.message : "AI設定を更新できませんでした。");
    } finally {
      setSettingsSaving(false);
    }
  };

  const onSaveKey = async () => {
    if (!keyInput.trim()) return;
    setSaving(true);
    try {
      if (isTauri()) {
        if (provider === "openai") await setOpenAiKey(keyInput.trim());
        else await setGeminiKey(keyInput.trim());
      }
      setHasKey(true);
      setKeyInput("");
    } finally {
      setSaving(false);
    }
  };

  const onProviderChange = async (next: AiProvider) => {
    const request = ++providerRequest.current;
    const previous = {
      provider: useAiStore.getState().provider,
      model: useAiStore.getState().model,
      hasKey: useAiStore.getState().hasKey,
    };
    const nextModel = next === "openai" ? "gpt-5.6-luna" : "gemini-flash-latest";
    setSettingsError(null);
    setSettingsSaving(true);
    setProvider(next, nextModel);
    setKeyInput("");
    if (isTauri()) {
      let backendUpdated = false;
      try {
        const settings = await updateSettings({ ai_provider: next });
        backendUpdated = true;
        const nextHasKey = next === "openai" ? await hasOpenAiKey() : await hasGeminiKey();
        if (providerRequest.current !== request) return;
        setProvider(next, next === "openai" ? settings.openai_model : settings.model);
        setHasKey(nextHasKey);
      } catch (err) {
        if (providerRequest.current !== request) return;
        if (backendUpdated) {
          try {
            await updateSettings({ ai_provider: previous.provider });
          } catch {
            // 復旧失敗も含め、画面には元の確定状態とエラーを表示する。
          }
        }
        setProvider(previous.provider, previous.model);
        setHasKey(previous.hasKey);
        setSettingsError(
          err instanceof Error ? err.message : "AIプロバイダーを変更できませんでした。",
        );
      } finally {
        if (providerRequest.current === request) setSettingsSaving(false);
      }
    } else {
      setHasKey(false);
      setSettingsSaving(false);
    }
  };

  const clearProjectFeedback = () => {
    setProjectValidation(null);
    setProjectStatus(null);
  };

  const onAddProject = async () => {
    const name = projectName.trim();
    const rootPath = projectPath.trim();
    clearProjectFeedback();
    if (!name) {
      setProjectValidation("表示名を入力してください。");
      return;
    }
    if (!rootPath) {
      setProjectValidation("プロジェクトのルートパスを入力してください。");
      return;
    }
    if (!rootPath.startsWith("/")) {
      setProjectValidation("NAS またはMac上の絶対パス（/ から始まるパス）を入力してください。");
      return;
    }
    try {
      await addProject(name, rootPath);
      setProjectName("");
      setProjectPath("");
      setProjectStatus(`「${name}」を登録しました。`);
    } catch {
      // 詳細はストアの projectsError を表示する。
    }
  };

  const onRemoveProject = async (projectId: number, name: string) => {
    clearProjectFeedback();
    try {
      await removeProject(projectId);
      setProjectStatus(`「${name}」の登録を解除しました。ファイルは削除していません。`);
    } catch {
      // 詳細はストアの projectsError を表示する。
    }
  };

  return (
    <div className="settings-pane">
      <h2>AI アシスト設定</h2>

      <label style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <input
          type="checkbox"
          checked={aiEnabled}
          disabled={settingsSaving}
          onChange={(e) => void onToggleAi(e.target.checked)}
        />
        AI 補助を有効にする(既定: オフ)
      </label>
      <p style={{ fontSize: 12, color: "var(--ink-3)" }}>
        オンにすると、作成中メールや関連する過去メールの本文が選択中の
        AI プロバイダーへ送信されます。
      </p>

      <h3>AI プロバイダー</h3>
      <select
        className="settings-select"
        value={provider}
        disabled={settingsSaving}
        onChange={(e) => void onProviderChange(e.target.value as AiProvider)}
      >
        <option value="gemini">Gemini API</option>
        <option value="openai">OpenAI API</option>
      </select>
      <p style={{ fontSize: 12, color: "var(--ink-3)" }}>
        使用モデル: {model}
      </p>
      {settingsError && <p className="cmp-status err" role="alert">{settingsError}</p>}

      <h3>{provider === "openai" ? "OpenAI" : "Gemini"} API キー</h3>
      <p style={{ fontSize: 12, color: "var(--ink-3)" }}>
        {hasKey ? "登録済み(Keychain に保存)" : "未登録"}
      </p>
      <div style={{ display: "flex", gap: 8 }}>
        <input
          type="password"
          value={keyInput}
          placeholder={provider === "openai" ? "sk-..." : "AIza..."}
          onChange={(e) => setKeyInput(e.target.value)}
          style={{ flex: 1 }}
        />
        <button className="primary" disabled={saving} onClick={() => void onSaveKey()}>
          保存
        </button>
      </div>

      <section className="project-settings" aria-labelledby="project-settings-title">
        <h3 id="project-settings-title">プロジェクト文脈</h3>
        <p className="muted">
          NAS またはMac上のプロジェクトを登録すると、AIが必要なファイルを検索して
          メールの要点や返信に利用できます。参照は読み取り専用で、ファイルの変更・削除・実行はしません。
          AI欄で選択したプロジェクトの関連ファイル抜粋は、選択中のAIプロバイダーへ送信されます。
        </p>

        {projects.length === 0 ? (
          <p className="muted">登録済みプロジェクトはありません。</p>
        ) : (
          <ul className="project-list" aria-label="登録済みプロジェクト">
            {projects.map((project) => (
              <li key={project.id}>
                <div className="project-list-copy">
                  <strong>{project.name}</strong>
                  <code title={project.root_path}>{project.root_path}</code>
                </div>
                <button
                  type="button"
                  className="link-danger"
                  disabled={projectsLoading}
                  aria-label={`${project.name} の登録を解除`}
                  onClick={() => void onRemoveProject(project.id, project.name)}
                >
                  解除
                </button>
              </li>
            ))}
          </ul>
        )}

        <div className="project-form">
          <label>
            表示名
            <input
              value={projectName}
              placeholder="例: A-mail"
              autoComplete="off"
              onChange={(event) => {
                setProjectName(event.target.value);
                clearProjectFeedback();
              }}
            />
          </label>
          <label>
            ルートパス
            <input
              value={projectPath}
              placeholder="/Volumes/NAS/projects/A-mail"
              autoComplete="off"
              spellCheck={false}
              onChange={(event) => {
                setProjectPath(event.target.value);
                clearProjectFeedback();
              }}
            />
          </label>
          <button
            type="button"
            className="primary"
            disabled={projectsLoading}
            onClick={() => void onAddProject()}
          >
            {projectsLoading ? "処理中…" : "プロジェクトを登録"}
          </button>
        </div>
        {(projectValidation || projectsError) && (
          <p className="form-error" role="alert">{projectValidation ?? projectsError}</p>
        )}
        {projectStatus && <p className="cmp-status ok" role="status">{projectStatus}</p>}
      </section>
    </div>
  );
}

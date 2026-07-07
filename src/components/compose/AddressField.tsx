interface AddressFieldProps {
  label: string;
  /** 表示中のアドレス群(M0 は pill 表示のみ、入力/オートコンプリートは M2)。 */
  addresses: string[];
}

/**
 * 宛先/Cc などのアドレス行。M0 は確定済みアドレスの pill 表示のみ。
 * オートコンプリート(送受信履歴からのアドレス帳)は M2/M3 で追加する。
 */
export function AddressField({ label, addresses }: AddressFieldProps) {
  return (
    <div className="field">
      <span className="lbl">{label}</span>
      <span className="val" style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
        {addresses.map((a) => (
          <span className="pill" key={a}>
            {a}
          </span>
        ))}
      </span>
    </div>
  );
}

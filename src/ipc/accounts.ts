import { invoke } from "./client";
import type { Account, Provider } from "./types";

// アカウント関連 Tauri コマンドの型付きラッパ。
// 実装本体は Rust 側(commands/account_cmd.rs)で M1 に用意する。

export function listAccounts(): Promise<Account[]> {
  return invoke<Account[]>("list_accounts");
}

export interface AddAccountInput {
  email: string;
  appPassword: string;
  provider: Provider;
  displayName: string;
}

export function addAccount(input: AddAccountInput): Promise<Account> {
  return invoke<Account>("add_account", {
    email: input.email,
    appPassword: input.appPassword,
    provider: input.provider,
    displayName: input.displayName,
  });
}

export function removeAccount(accountId: number): Promise<void> {
  return invoke<void>("remove_account", { accountId });
}

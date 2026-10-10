import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { SETTINGS_SAVE_DEBOUNCE_MS } from "./features/settings";
import type { AboutInfo, Settings } from "./ipc";

function mockAppIpc(handler: Parameters<typeof mockIPC>[0]) {
  mockIPC(handler, { shouldMockEvents: true });
}

/** The header's language selector; its accessible name follows the language. */
function languageSelect(): HTMLElement {
  return screen.getByRole("combobox");
}

function selectLanguage(lang: "ja" | "en") {
  fireEvent.change(languageSelect(), { target: { value: lang } });
}

const SAVED_SETTINGS: Settings = {
  language: "en",
  outputDir: { dirLabel: "CleanedFiles" },
};

const ABOUT_READY: AboutInfo = {
  version: "0.1.0",
};

/** Lets the `get_settings` answer and the renders after it through. */
async function flush() {
  await act(async () => {});
}

describe("App", () => {
  beforeAll(async () => {
    // The dialog loads the license list on demand; load it once up front
    // so tests that expand it do not time out.
    await import("./licenses");
  });

  afterEach(() => {
    cleanup();
    clearMocks();
    vi.useRealTimers();
  });

  describe("header and layout", () => {
    it("shows app title, language selector, and about button in the header", () => {
      mockAppIpc(() => new Promise(() => {}));
      render(<App initialNavLang="ja" initialSettings={null} />);

      expect(
        screen.getByRole("heading", { name: "Metadata Cleaner" }),
      ).toBeInTheDocument();
      expect(screen.getByRole("combobox", { name: "言語" })).toHaveValue("ja");
      expect(
        screen.getByRole("option", { name: "English" }),
      ).toBeInTheDocument();
      expect(
        screen.getByRole("button", { name: "このアプリについて" }),
      ).toBeInTheDocument();
    });

    it("switches language when another language is selected", () => {
      mockAppIpc(() => new Promise(() => {}));
      render(<App initialNavLang="ja" initialSettings={null} />);

      // Switch to English
      selectLanguage("en");
      expect(screen.getByText("Output folder")).toBeInTheDocument();
      expect(screen.getByText("No files selected")).toBeInTheDocument();
      expect(
        screen.getByText(
          "Visible contents, PDF body text, and file names cannot be removed",
        ),
      ).toBeInTheDocument();

      // Switch back to Japanese
      selectLanguage("ja");
      expect(screen.getByText("保存先フォルダ")).toBeInTheDocument();
      expect(screen.getByText("ファイルがありません")).toBeInTheDocument();
      expect(
        screen.getByText("写っているもの、PDF の本文、ファイル名は消せません"),
      ).toBeInTheDocument();
    });

    it("shows drop zone when file list is empty", () => {
      mockAppIpc(() => new Promise(() => {}));
      render(<App initialSettings={null} />);

      expect(
        screen.getByRole("region", {
          name: /ファイルまたはフォルダをここにドロップ|Drop files or folders here/,
        }),
      ).toBeInTheDocument();
    });

    it("shows skipped message below drop zone when files are skipped and list is empty", async () => {
      mockAppIpc((cmd) => {
        if (cmd === "add_files") {
          return {
            added: [],
            skipped: { folders: 1, unsupported: 0, duplicates: 0 },
          };
        }
        return new Promise(() => {});
      });

      render(<App initialNavLang="ja" initialSettings={null} />);

      const addBtn = screen.getByRole("button", { name: "ファイルを追加" });
      await act(async () => {
        fireEvent.click(addBtn);
      });

      expect(
        screen.getByText("（対象外 1 件：サブフォルダ）"),
      ).toBeInTheDocument();
    });

    it("shows hint text in the right aside when nothing is selected", () => {
      mockAppIpc(() => new Promise(() => {}));
      render(<App initialSettings={null} />);

      const aside = screen.getByRole("complementary", {
        name: /選んだファイルの詳細|Details of selected file/,
      });
      expect(aside).toBeInTheDocument();
      expect(
        screen.getByText(
          /写っているもの、PDF の本文、ファイル名は消せません|Visible contents, PDF body text, and file names cannot be removed/,
        ),
      ).toBeInTheDocument();
    });
  });

  describe("restoring the settings", () => {
    it("starts in the saved language and output folder", async () => {
      mockAppIpc((cmd) => (cmd === "get_settings" ? SAVED_SETTINGS : null));
      render(<App initialNavLang="ja" />);
      await flush();

      // The saved language wins over the OS language
      expect(languageSelect()).toHaveValue("en");
      expect(screen.getByText("Output folder")).toBeInTheDocument();
      expect(screen.getByText("CleanedFiles")).toBeInTheDocument();
    });

    it("follows the OS language when none was saved", async () => {
      mockAppIpc((cmd) =>
        cmd === "get_settings" ? { language: null, outputDir: null } : null,
      );
      render(<App initialNavLang="ja-JP" />);
      await flush();

      expect(languageSelect()).toHaveValue("ja");
    });

    it("shows nothing until the settings are read", () => {
      mockAppIpc(() => new Promise(() => {}));
      render(<App initialNavLang="ja" />);

      expect(
        screen.queryByRole("heading", { name: "Metadata Cleaner" }),
      ).not.toBeInTheDocument();
    });

    it("starts from the defaults when get_settings fails", async () => {
      mockAppIpc((cmd) => {
        if (cmd === "get_settings") {
          throw { code: "ReadFailed", detail: null };
        }
        return null;
      });
      render(<App initialNavLang="ja" />);
      await flush();

      expect(
        screen.getByRole("heading", { name: "Metadata Cleaner" }),
      ).toBeInTheDocument();
      expect(languageSelect()).toHaveValue("ja");
      expect(screen.getByText("未選択")).toBeInTheDocument();
    });

    it("starts from the defaults when the answer has the wrong shape", async () => {
      mockAppIpc((cmd) =>
        cmd === "get_settings" ? { invalidSettingsPayload: true } : null,
      );
      render(<App initialNavLang="ja" />);
      await flush();

      expect(
        screen.getByRole("heading", { name: "Metadata Cleaner" }),
      ).toBeInTheDocument();
      expect(languageSelect()).toHaveValue("ja");
    });
  });

  describe("saving the settings", () => {
    function mockSaves() {
      const saved: unknown[] = [];
      mockAppIpc((cmd, args) => {
        if (cmd === "save_settings") {
          saved.push(args);
          return null;
        }
        return new Promise(() => {});
      });
      return saved;
    }

    it("saves a run of changes once, after they settle", async () => {
      vi.useFakeTimers();
      const saved = mockSaves();
      render(<App initialNavLang="ja" initialSettings={null} />);

      selectLanguage("en");
      act(() => {
        vi.advanceTimersByTime(SETTINGS_SAVE_DEBOUNCE_MS - 1);
      });
      selectLanguage("ja");
      selectLanguage("en");
      act(() => {
        vi.advanceTimersByTime(SETTINGS_SAVE_DEBOUNCE_MS - 1);
      });
      expect(saved).toEqual([]);

      await act(async () => {
        vi.advanceTimersByTime(1);
      });
      expect(saved).toEqual([
        {
          input: {
            language: "en",
          },
        },
      ]);
    });

    it("does not save what the app started with", async () => {
      vi.useFakeTimers();
      const saved = mockSaves();
      render(<App initialNavLang="ja" initialSettings={SAVED_SETTINGS} />);

      await act(async () => {
        vi.advanceTimersByTime(SETTINGS_SAVE_DEBOUNCE_MS * 2);
      });
      expect(saved).toEqual([]);
    });

    it("does not save a change that was undone before the save", async () => {
      vi.useFakeTimers();
      const saved = mockSaves();
      render(
        <App
          initialNavLang="ja"
          initialSettings={{ language: "ja", outputDir: null }}
        />,
      );

      selectLanguage("en");
      selectLanguage("ja");
      await act(async () => {
        vi.advanceTimersByTime(SETTINGS_SAVE_DEBOUNCE_MS * 2);
      });
      expect(saved).toEqual([]);
    });
  });

  describe("about this app", () => {
    function openAbout() {
      fireEvent.click(
        screen.getByRole("button", { name: "このアプリについて" }),
      );
      return screen.getByRole("dialog", { name: "このアプリについて" });
    }

    it("shows the version and this app's license", async () => {
      mockAppIpc((cmd) => (cmd === "get_about" ? ABOUT_READY : null));
      render(<App initialNavLang="ja" initialSettings={null} />);
      const dialog = openAbout();

      expect(
        await within(dialog).findByText("バージョン 0.1.0"),
      ).toBeInTheDocument();
      expect(within(dialog).getByText("Metadata Cleaner")).toBeInTheDocument();
      expect(
        within(dialog).getByText(/Permission is hereby granted/),
      ).toBeInTheDocument();
    });

    it("says so when the version cannot be read", async () => {
      mockAppIpc((cmd) => (cmd === "get_about" ? { version: 123 } : null));
      render(<App initialNavLang="ja" initialSettings={null} />);
      const dialog = openAbout();

      expect(
        await within(dialog).findByText("バージョンを取得できませんでした"),
      ).toBeInTheDocument();
    });

    it("lists the third-party licenses when expanded", async () => {
      mockAppIpc((cmd) => (cmd === "get_about" ? ABOUT_READY : null));
      render(<App initialNavLang="ja" initialSettings={null} />);
      const dialog = openAbout();

      const toggle = within(dialog).getByRole("button", {
        name: "第三者ライセンスを表示",
      });
      expect(toggle).toHaveAttribute("aria-expanded", "false");
      fireEvent.click(toggle);
      expect(toggle).toHaveAttribute("aria-expanded", "true");

      expect(await within(dialog).findByText(/^tauri /)).toBeInTheDocument();
      expect(within(dialog).getAllByText(/^react /).length).toBeGreaterThan(0);
    });

    it("moves focus in, keeps the app inert, and gives focus back on Escape", async () => {
      mockAppIpc((cmd) => (cmd === "get_about" ? ABOUT_READY : null));
      const { container } = render(
        <App initialNavLang="ja" initialSettings={null} />,
      );
      const aboutButton = screen.getByRole("button", {
        name: "このアプリについて",
      });
      const dialog = openAbout();
      await within(dialog).findByText("バージョン 0.1.0");

      expect(dialog.contains(document.activeElement)).toBe(true);
      const shell = container.querySelector(".app-container");
      expect(shell).toHaveAttribute("inert");

      fireEvent.keyDown(window, { key: "Escape" });

      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      expect(shell).not.toHaveAttribute("inert");
      expect(aboutButton).toHaveFocus();
    });

    it("closes with the close button", async () => {
      mockAppIpc((cmd) => (cmd === "get_about" ? ABOUT_READY : null));
      render(<App initialNavLang="ja" initialSettings={null} />);
      const dialog = openAbout();
      await within(dialog).findByText("バージョン 0.1.0");

      const closeButtons = within(dialog).getAllByRole("button", {
        name: "閉じる",
      });
      fireEvent.click(closeButtons[closeButtons.length - 1]);

      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("closes when clicking on the overlay backdrop", async () => {
      mockAppIpc((cmd) => (cmd === "get_about" ? ABOUT_READY : null));
      const { container } = render(
        <App initialNavLang="ja" initialSettings={null} />,
      );
      openAbout();

      const overlay = container.querySelector(".modal-overlay");
      expect(overlay).toBeInTheDocument();
      if (overlay instanceof HTMLElement) {
        fireEvent.click(overlay);
      }

      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });
  });
});

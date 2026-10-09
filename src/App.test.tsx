import { render, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { App } from "./App";

afterEach(() => {
  clearMocks();
});

describe("App", () => {
  it("shows the app name", () => {
    mockIPC(() => ({ workerVersion: "0.1.0", error: null }));
    render(<App />);
    expect(
      screen.getByRole("heading", { name: "Metadata Cleaner" }),
    ).toBeInTheDocument();
  });

  it("shows the version the worker answered with", async () => {
    mockIPC((cmd) =>
      cmd === "check_worker" ? { workerVersion: "9.8.7", error: null } : null,
    );
    render(<App />);
    expect(await screen.findByTestId("worker-status")).toHaveTextContent(
      "PDF worker 9.8.7: OK",
    );
  });

  it("shows why the worker did not answer", async () => {
    mockIPC(() => ({ workerVersion: null, error: "Crashed" }));
    render(<App />);
    expect(await screen.findByTestId("worker-status")).toHaveTextContent(
      "PDF worker: Crashed",
    );
  });

  it("rejects an answer of the wrong shape", async () => {
    mockIPC(() => ({ workerVersion: 1 }));
    render(<App />);
    expect(await screen.findByTestId("worker-status")).toHaveTextContent(
      "check_worker returned an unexpected value",
    );
  });
});

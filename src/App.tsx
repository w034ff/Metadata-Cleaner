import { useEffect, useState } from "react";
import { checkWorker } from "./ipc/workerCheck";

// Until the screens (T11, T12) exist, the window shows whether the PDF
// worker starts, so that installed builds can be checked (work-plan T01).
export function App() {
  const [status, setStatus] = useState<string | null>(null);

  useEffect(() => {
    checkWorker().then(
      (check) =>
        setStatus(
          check.workerVersion !== null
            ? `PDF worker ${check.workerVersion}: OK`
            : `PDF worker: ${check.error ?? ""}`,
        ),
      (e: unknown) => setStatus(String(e)),
    );
  }, []);

  return (
    <main>
      <h1>Metadata Cleaner</h1>
      {status !== null && <p data-testid="worker-status">{status}</p>}
    </main>
  );
}

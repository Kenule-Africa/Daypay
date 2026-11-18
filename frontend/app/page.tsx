export const dynamic = "force-dynamic";

async function apiAvailable(): Promise<boolean> {
  try {
    const endpoint = new URL("/health/ready", process.env.API_BASE_URL ?? "http://127.0.0.1:8000");
    const response = await fetch(endpoint, {
      cache: "no-store",
      redirect: "error",
      signal: AbortSignal.timeout(2500),
    });
    return response.ok && (await response.json()).status === "ready";
  } catch {
    return false;
  }
}

export default async function Home() {
  const ready = await apiAvailable();
  return (
    <main>
      <header>DEYPAY</header>
      <section aria-labelledby="status-title">
        <h1 id="status-title">Service status</h1>
        <dl><dt>Payments API</dt><dd data-ready={ready}>{ready ? "Available" : "Unavailable"}</dd></dl>
      </section>
    </main>
  );
}

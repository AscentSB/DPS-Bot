// Small fetch wrapper with a timeout and readable errors.
export async function getJson(url, { headers = {}, timeoutMs = 15000 } = {}) {
  const res = await fetch(url, { headers, signal: AbortSignal.timeout(timeoutMs) });
  if (res.status === 204 || res.status === 404) return null;
  if (!res.ok) {
    const body = await res.text().catch(() => '');
    throw new Error(`HTTP ${res.status} from ${new URL(url).host}: ${body.slice(0, 200)}`);
  }
  return res.json();
}
